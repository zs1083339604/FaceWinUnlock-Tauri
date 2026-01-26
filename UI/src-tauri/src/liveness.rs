use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri_plugin_log::log::{error, info, warn};
use onnxruntime::{environment::Environment, GraphOptimizationLevel, LoggingLevel};
use onnxruntime::ndarray::Array;

use crate::ROOT_DIR;

/// 活体检测器（基于 ONNX Runtime，输入尺寸 112x112）
/// 模型输出是假体置信度：score > 0.5 为假体，score <= 0.5 为真人
/// 真人置信度 = 1.0 - score
pub struct LivenessDetector {
    model_path: PathBuf,
    initialized: bool,
    input_size: usize, 
}

impl LivenessDetector {
    /// 创建新的活体检测器
    pub fn new() -> Self {
        Self {
            model_path: PathBuf::new(),
            initialized: false,
            input_size: 112,
        }
    }

    /// 初始化活体检测器
    pub fn initialize(&mut self) -> Result<()> {
        // 从软件安装目录的 resources 子目录加载模型
        self.model_path = ROOT_DIR.join("resources").join("detect.onnx");
        info!("活体检测模型路径: {:?}", self.model_path);

        if !self.model_path.exists() {
            warn!("活体检测模型不存在: {:?}", self.model_path);
            return Err(anyhow!("模型不存在: {:?}", self.model_path));
        }

        self.initialized = true;
        info!("活体检测器已就绪，模型路径: {:?}", self.model_path);

        Ok(())
    }

    /// 使用 ONNX Runtime 模型检测图片是否为活体
    /// img_data: 图片数据（JPEG/PNG等格式）
    /// 返回: (是否活体, 真人置信度)
    pub fn detect(&self, img_data: &[u8]) -> Result<(bool, f32)> {
        if !self.initialized {
            return Err(anyhow!("活体检测器未初始化"));
        }

        // 创建环境（每次检测时创建新的）
        let environment = Environment::builder()
            .with_name("face-liveness")
            .with_log_level(LoggingLevel::Warning)
            .build()
            .map_err(|e| anyhow!("创建 ONNX Runtime 环境失败: {:?}", e))?;

        // 创建会话并加载模型
        let mut session = environment
            .new_session_builder()
            .map_err(|e| anyhow!("创建会话构建器失败: {:?}", e))?
            .with_optimization_level(GraphOptimizationLevel::All)
            .map_err(|e| anyhow!("设置优化级别失败: {:?}", e))?
            .with_model_from_file(&self.model_path)
            .map_err(|e| anyhow!("加载模型失败: {:?}", e))?;

        // 获取输入形状
        let input_shape: Vec<usize> = session.inputs[0]
            .dimensions()
            .map(|d| d.unwrap_or(1) as usize)
            .collect();

        info!("ONNX 模型输入形状: {:?}", input_shape);

        let batch = input_shape.get(0).copied().unwrap_or(1);
        let channels = input_shape.get(1).copied().unwrap_or(3);
        let height = input_shape.get(2).copied().unwrap_or(self.input_size);
        let width = input_shape.get(3).copied().unwrap_or(self.input_size);

        // 解码图片
        let img = image::load_from_memory(img_data)
            .with_context(|| "无法解码图片")?;

        // 转换为RGB并调整大小到 112x112
        let img = img.to_rgb8();
        let img = image::imageops::resize(&img, width as u32, height as u32, image::imageops::FilterType::Lanczos3);

        let (img_height, img_width) = img.dimensions();

        // 创建输入数据
        let total_elements = batch * channels * height * width;
        let mut input_values: Vec<f32> = Vec::with_capacity(total_elements);

        // 中心裁剪并填充数据
        let src_x = (img_width as i32 - width as i32) / 2;
        let src_y = (img_height as i32 - height as i32) / 2;

        // 准备输入数据 (NCHW格式)
        for _b in 0..batch {
            for c in 0..channels {
                for y in 0..height {
                    for x in 0..width {
                        let src_x = (src_x + x as i32).max(0).min(img_width as i32 - 1) as u32;
                        let src_y = (src_y + y as i32).max(0).min(img_height as i32 - 1) as u32;
                        let pixel = img.get_pixel(src_x, src_y);

                        // 归一化到 [0, 1] 并减去均值
                        let value = match c {
                            0 => (pixel[0] as f32 / 255.0 - 0.5) * 2.0,  // R
                            1 => (pixel[1] as f32 / 255.0 - 0.5) * 2.0,  // G
                            _ => (pixel[2] as f32 / 255.0 - 0.5) * 2.0,  // B
                        };
                        input_values.push(value);
                    }
                }
            }
        }

        // 创建 ndarray 数组
        let input_array = Array::from_shape_vec(
            vec![batch, channels, height, width],
            input_values
        ).map_err(|e| anyhow!("创建输入数组失败: {:?}", e))?;

        let input_tensor_values = vec![input_array.into()];

        // 运行推理
        let outputs = session.run(input_tensor_values)
            .map_err(|e| anyhow!("模型推理失败: {:?}", e))?;

        // 获取输出
        let output = outputs.into_iter().next()
            .ok_or_else(|| anyhow!("模型无输出"))?;

        let view = output.view();
        let output_data = view.as_slice()
            .ok_or_else(|| anyhow!("转换输出失败"))?;

        // 解析结果 - 模型输出 2 个值: [假体概率, 真人概率]
        if output_data.len() >= 2 {
            let fake_prob: f32 = output_data[0];
            let real_prob: f32 = output_data[1];

            info!("活体检测概率 - 假体: {:.4}, 真人: {:.4}", fake_prob, real_prob);

            // 判定: 真人概率 > 0.5 且真人概率 > 假体概率 时为活体
            let is_live = real_prob > fake_prob && real_prob > 0.5;
            // 直接使用真人概率作为置信度
            let confidence = real_prob.clamp(0.0, 1.0);

            info!("活体检测结果 - 是否活体: {}, 真人置信度: {:.2}%",
                  is_live, confidence * 100.0);

            Ok((is_live, confidence))
        } else if output_data.len() == 1 {
            // 兼容单值输出（logit 格式）
            let liveness_score: f32 = output_data[0];
            info!("活体检测原始分数: {:.4}", liveness_score);
            // score > 0.5 为假体，<= 0.5 为真人
            let is_live = liveness_score <= 0.5;
            let confidence = if is_live {
                (1.0 - liveness_score).clamp(0.0, 1.0)
            } else {
                liveness_score.clamp(0.0, 1.0)
            };
            info!("活体检测结果 - 是否活体: {}, 真人置信度: {:.2}%",
                  is_live, confidence * 100.0);
            Ok((is_live, confidence))
        } else {
            Err(anyhow!("模型输出维度不支持: {}", output_data.len()))
        }
    }

    /// 检查是否已初始化
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

/// 全局活体检测器实例
lazy_static::lazy_static! {
    pub static ref LIVENESS_DETECTOR: Arc<Mutex<LivenessDetector>> = {
        Arc::new(Mutex::new(LivenessDetector::new()))
    };
}

/// 对图片数据进行活体检测
/// 返回: (是否活体, 真人置信度)
pub fn check_liveness(img_data: &[u8]) -> (bool, f32) {
    let detector = LIVENESS_DETECTOR.lock().unwrap();

    if !detector.is_initialized() {
        warn!("活体检测器未初始化，假设活体");
        return (true, 1.0);
    }

    match detector.detect(img_data) {
        Ok((is_live, confidence)) => (is_live, confidence),
        Err(e) => {
            error!("活体检测失败: {:?}", e);
            (true, 1.0) // 默认返回活体
        }
    }
}
