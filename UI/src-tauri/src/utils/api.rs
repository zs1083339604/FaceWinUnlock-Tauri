use std::{os::windows::process::CommandExt, process::Command};

use crate::{utils::custom_result::CustomResult, OpenCVResource, APP_STATE, DB_POOL, GLOBAL_TRAY, ROOT_DIR};
use opencv::{
    core::{Mat, MatTraitConst, Size},
    objdetect::{FaceDetectorYN, FaceRecognizerSF},
    videoio::{self, VideoCapture, VideoCaptureTrait, VideoCaptureTraitConst},
};
use r2d2::Pool;
use r2d2_sqlite::rusqlite;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Manager};
use tauri_plugin_log::log::{error, info, warn};
use windows::{
    core::{BSTR, HSTRING, PWSTR},
    Win32::{
        Foundation::{E_UNEXPECTED, HWND},
        Media::{
            DirectShow::ICreateDevEnum,
            MediaFoundation::{CLSID_SystemDeviceEnum, CLSID_VideoInputDeviceCategory},
        },
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, IEnumMoniker,
                StructuredStorage::IPropertyBag, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
            }, RemoteDesktop::WTSUnRegisterSessionNotification, Shutdown::LockWorkStation, Variant::{VariantClear, VARIANT}, WindowsProgramming::GetUserNameW
        },
    },
};

use super::pipe::Client;

#[derive(Debug, Clone, Serialize)]
struct ValidCameraInfo {
    camera_name: String,
    capture_index: String,
    is_valid: bool,
}

// 定义摄像头后端类型枚举
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CameraBackend {
    Any,   // CAP_ANY
    DShow, // CAP_DSHOW
    MSMF,  // CAP_MSMF
    VFW,   // CAP_VFW
}

impl From<CameraBackend> for i32 {
    fn from(backend: CameraBackend) -> Self {
        match backend {
            CameraBackend::Any => videoio::CAP_ANY,
            CameraBackend::DShow => videoio::CAP_DSHOW,
            CameraBackend::MSMF => videoio::CAP_MSMF,
            CameraBackend::VFW => videoio::CAP_VFW,
        }
    }
}

// 获取当前用户名
#[tauri::command]
pub fn get_now_username() -> Result<CustomResult, CustomResult> {
    // buffer大小，256应该够了
    let mut buffer = [0u16; 256];
    let mut size = buffer.len() as u32;
    unsafe {
        let succuess = GetUserNameW(Some(PWSTR(buffer.as_mut_ptr())), &mut size);
        if succuess.is_err() {
            return Err(CustomResult::error(
                Some(format!("获取用户名失败: {:?}", succuess.err())),
                None,
            ));
        }

        let name = String::from_utf16_lossy(&buffer[..size as usize - 1]);
        return Ok(CustomResult::success(None, Some(json!({"username": name}))));
    }
}

// 测试 WinLogon 是否加载成功
#[tauri::command]
pub fn test_win_logon(user_name: String, password: String) -> Result<CustomResult, CustomResult> {
    // 锁定屏幕
    unsafe {
        let succuess = LockWorkStation();
        if succuess.is_err() {
            return Err(CustomResult::error(
                Some(format!("锁定屏幕失败: {:?}", succuess.err())),
                None,
            ));
        }

        // 等待5秒
        std::thread::sleep(std::time::Duration::from_secs(5));
        // 解锁
        unlock(user_name, password)
            .map_err(|e| CustomResult::error(Some(format!("解锁屏幕失败: {:?}", e)), None))?;
    }
    return Ok(CustomResult::success(None, None));
}

// 初始化模型
#[tauri::command]
pub fn init_model() -> Result<CustomResult, CustomResult> {
    // 加载模型
    let mut app_state = APP_STATE
        .lock()
        .map_err(|e| CustomResult::error(Some(format!("获取app状态 {}", e)), None))?;
    if app_state.detector.is_none() {
        let resource_path = ROOT_DIR
            .join("resources")
            .join("face_detection_yunet_2023mar.onnx");

        // 这个不用检查文件是否存在，不存在opencv会报错
        let detector = FaceDetectorYN::create(
            resource_path.to_str().unwrap_or(""),
            "",
            Size::new(320, 320), // 初始尺寸，后面会动态更新
            0.9,
            0.3,
            5000,
            0,
            0,
        )
        .map_err(|e| CustomResult::error(Some(format!("初始化检测器模型失败: {:?}", e)), None))?;

        app_state.detector = Some(OpenCVResource { inner: detector });
    }

    if app_state.recognizer.is_none() {
        let resource_path = ROOT_DIR
            .join("resources")
            .join("face_recognition_sface_2021dec.onnx");
        let recognizer = FaceRecognizerSF::create(resource_path.to_str().unwrap_or(""), "", 0, 0)
            .map_err(|e| {
            CustomResult::error(Some(format!("初始化识别器模型失败: {:?}", e)), None)
        })?;

        app_state.recognizer = Some(OpenCVResource { inner: recognizer });
    }

    let db_path = ROOT_DIR.join("database.db");

    // 创建连接池
    let mut pool_guard = DB_POOL
        .lock()
        .map_err(|e| CustomResult::error(Some(format!("获取连接池锁失败 {}", e)), None))?;

    if pool_guard.as_ref().is_none() {
        // 如果当前没有SQLite 连接池，则创建一个
        let manager = r2d2_sqlite::SqliteConnectionManager::file(&db_path).with_flags(
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                | rusqlite::OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        );

        let pool = Pool::builder()
            .max_size(2) // 回调函数使用，不需要太多连接
            .build(manager)
            .map_err(|e| CustomResult::error(Some(format!("创建连接池失败 {}", e)), None))?;

        *pool_guard = Some(pool);
    }

    Ok(CustomResult::success(None, None))
}

// 获取windows所有摄像头
#[tauri::command]
pub fn get_camera() -> Result<CustomResult, CustomResult> {
    // 初始化COM
    let com_init_result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if com_init_result.is_err() {
        return Err(CustomResult::error(
            Some(String::from("初始化Com失败")),
            None,
        ));
    }

    let com_operation_result = get_windows_video_devices();
    // 卸载Com
    unsafe { CoUninitialize() };

    if let Err(e) = com_operation_result {
        return Err(CustomResult::error(
            Some(format!("获取系统摄像头失败 {}", e)),
            None,
        ));
    }

    let video_devices = com_operation_result.unwrap();
    if video_devices.is_empty() {
        return Err(CustomResult::error(
            Some(String::from("未检测到系统视频设备（摄像头）")),
            None,
        ));
    }

    // 判断摄像头可用性
    let mut valid_cameras = Vec::new();
    for (camera_name, index) in video_devices {
        match is_camera_index_valid(index) {
            Ok(is_valid) => {
                valid_cameras.push(ValidCameraInfo {
                    camera_name,
                    capture_index: index.to_string(),
                    is_valid: is_valid,
                });
            }
            _ => {}
        }
    }

    Ok(CustomResult::success(None, Some(json!(valid_cameras))))
}

// 打开摄像头
#[tauri::command]
pub fn open_camera(
    backend: Option<CameraBackend>,
    camear_index: i32,
) -> Result<CustomResult, CustomResult> {
    let mut app_state = APP_STATE
        .lock()
        .map_err(|e| CustomResult::error(Some(format!("获取app状态失败 {}", e)), None))?;

    // 如果摄像头已打开，直接返回成功
    if app_state.camera.is_some() {
        return Ok(CustomResult::success(None, None));
    }

    // 尝试的列表
    let backends_to_try = match backend {
        // 指定了：只尝试该后端
        Some(backend) => vec![backend],
        // 未指定：尝试所有常用后端
        None => vec![
            CameraBackend::DShow,
            CameraBackend::Any,
            CameraBackend::MSMF,
            CameraBackend::VFW,
        ],
    };

    // 循环尝试不同后端
    for (idx, backend_inner) in backends_to_try.iter().enumerate() {
        match try_open_camera_with_backend(*backend_inner, camear_index) {
            Ok(cam) => {
                // 成功打开
                app_state.camera = Some(OpenCVResource { inner: cam });
                let msg = if backend.is_some() {
                    format!("使用指定后端 {:?} 成功打开摄像头", backend)
                } else {
                    format!("尝试第{}个后端 {:?} 成功打开摄像头", idx + 1, backend)
                };
                info!("{}", msg);
                return Ok(CustomResult::success(None, None));
            }
            Err(e) => {
                // 处理失败情况
                if backend.is_some() {
                    // 指定了后端但失败：直接返回错误
                    return Err(CustomResult::error(
                        Some(format!("使用指定后端 {:?} 打开摄像头失败: {}", backend, e)),
                        None,
                    ));
                } else {
                    // 未指定后端：打印尝试失败日志，继续尝试下一个
                    warn!("尝试后端 {:?} 失败: {}", backend, e);
                    continue;
                }
            }
        }
    }

    // 所有后端都尝试失败
    Err(CustomResult::error(
        Some("所有摄像头后端均尝试失败，请检查设备是否连接/被占用/有权限".to_string()),
        None,
    ))
}

// 关闭摄像头
#[tauri::command]
pub fn stop_camera() -> Result<CustomResult, CustomResult> {
    let mut app_state = APP_STATE
        .lock()
        .map_err(|e| CustomResult::error(Some(format!("获取app状态失败 {}", e)), None))?;
    app_state.camera = None;
    Ok(CustomResult::success(None, None))
}

// 打开指定目录用资源管理器
#[tauri::command]
pub fn open_directory(path: String) -> Result<CustomResult, CustomResult> {
    let path = std::path::Path::new(&path);
    if !path.exists() {
        return Err(CustomResult::error(
            Some(format!("路径不存在 {}", path.display())),
            None,
        ));
    }

    std::process::Command::new("explorer")
        .arg(path)
        .status()
        .map_err(|e| {
            CustomResult::error(
                Some(format!(
                    "打开文件夹失败：{}<br>请手动打开文件夹：{:?}",
                    e,
                    path.to_str()
                )),
                None,
            )
        })?;

    Ok(CustomResult::success(None, None))
}

// 自启代码由 Google Gemini 3 生成
// 我写不了出来了，注册表不管用 哭**
const CREATE_NO_WINDOW: u32 = 0x08000000;
// 启用全用户自启动 (通过任务计划程序)
#[tauri::command]
pub fn enable_global_autostart() -> Result<CustomResult, CustomResult> {
    let binding = ROOT_DIR.join("facewinunlock-tauri.exe");
    let path = binding.to_str();
    if path.is_none() {
        return Err(CustomResult::error(
            Some(String::from("程序路径解析失败")),
            None,
        ));
    }
    let path = path.unwrap();

    // 构建 schtasks 命令
    // /Create: 创建任务
    // /TN: 任务名称
    // /TR: 运行程序路径及参数 (注意转义引号)
    // /SC ONLOGON: 登录时启动
    // /RL HIGHEST: 以最高权限运行 (绕过 UAC 弹窗的关键)
    // /F: 强制创建，如果已存在则覆盖
    let task_name = "FaceWinUnlockAutoStart";
    let task_run = format!("\"{}\" --silent", path);

    let output = Command::new("schtasks")
        .args(&[
            "/Create",
            "/TN", task_name,
            "/TR", &task_run,
            "/SC", "ONLOGON",
            "/RL", "HIGHEST",
            "/RU", "BUILTIN\\Users",  // 全用户组
            "/IT",
            "/DELAY", "0000:10",
            "/F",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| CustomResult::error(Some(format!("执行系统命令失败: {}", e)), None))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(CustomResult::error(Some(format!("创建全用户计划任务失败: {}", err_msg)), None));
    }

    // 增强版：修改任务设置（补充会话交互配置）
    let ps_command = format!(
       r#"
        $task = Get-ScheduledTask -TaskName '{}' -ErrorAction Stop;
        $task.Settings.DisallowStartIfOnBatteries = $false;
        $task.Settings.StopIfGoingOnBatteries = $false;
        $task.Settings.AllowStartOnDemand = $true;
        $task.Settings.StartWhenAvailable = $true;
        $task.Settings.MultipleInstances = 'Parallel';
        $task.Settings.AllowHardTerminate = $true;
        $task.Settings.DisallowStartOnRemoteAppSession = $false;
        $task.Settings.IdleSettings.IdleDuration = 'PT0S';
        $task.Settings.IdleSettings.WaitTimeout = 'PT0S';
        $task.Settings.IdleSettings.StopOnIdleEnd = $false;
        $task.Settings.IdleSettings.RestartOnIdle = $false;
        $task.Settings.AllowInteractive = $true;
        $task.Principal.LogonType = 'InteractiveToken';
        $task.Principal.RunLevel = 'HighestAvailable';
        $task.Settings.RestartCount = 3;
        $task.Settings.RestartInterval = 'PT1M';
        Set-ScheduledTask -InputObject $task -ErrorAction Stop;
        "#, task_name
    );

    let ps_output = Command::new("powershell")
        .args(&[
            "-ExecutionPolicy", "Bypass",
            "-NoProfile",              // 不加载 PowerShell 配置，避免干扰
            "-Command", &ps_command,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| CustomResult::error(Some(format!("PowerShell 修改任务设置失败: {}", e)), None))?;

    if !ps_output.status.success() {
        let err_msg = String::from_utf8_lossy(&ps_output.stderr);
        warn!("警告：修改任务高级设置失败，但基础任务已创建: {}", err_msg);
    }

    Ok(CustomResult::success(None, None))
}

// 禁用全用户自启动
#[tauri::command]
pub fn disable_global_autostart() -> Result<CustomResult, CustomResult> {
    let task_name = "FaceWinUnlockAutoStart";

    let output = Command::new("schtasks")
        .args(&["/Delete", "/TN", task_name, "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| CustomResult::error(Some(format!("执行系统命令失败: {}", e)), None))?;

    if output.status.success() {
        Ok(CustomResult::success(None, None))
    } else {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        // 如果任务本身不存在，删除会报错，这里可以根据需要判断是否视为成功
        Err(CustomResult::error(Some(format!("删除计划任务失败: {}", err_msg)), None))
    }
}

// 检查是否已开启全用户自启动
#[tauri::command]
pub fn check_global_autostart() -> Result<CustomResult, CustomResult> {
    let task_name = "FaceWinUnlockAutoStart";

    // /Query 检查任务是否存在
    let output = Command::new("schtasks")
        .args(&["/Query", "/TN", task_name])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| CustomResult::error(Some(format!("查询系统命令失败: {}", e)), None))?;

    // 如果状态码为 0，说明任务存在
    let is_enabled = output.status.success();

    Ok(CustomResult::success(
        None,
        Some(json!({"enable": is_enabled})),
    ))
}

// 关闭软件
#[tauri::command]
pub fn close_app(app_handle: AppHandle) -> Result<CustomResult, CustomResult> {
    let window = app_handle.get_webview_window("main").unwrap();
    let hwnd = window.hwnd().unwrap();
    unsafe {
        // 注销 WTS 通知
        let _ = WTSUnRegisterSessionNotification(HWND(hwnd.0));
    }
    
    // 关闭系统托盘
    let mut guard = GLOBAL_TRAY.lock().map_err(|e| CustomResult::error(Some(format!("锁定托盘全局变量失败: {}", e)), None))?;
    if let Some(tray_any) = guard.as_mut() {
        tray_any.set_visible(false)
            .map_err(|e| CustomResult::error(Some(format!("隐藏托盘图标失败: {}", e)), None))?;
    }

    app_handle.exit(0);

    Ok(CustomResult::success(None, None))
}
// 使用指定后端尝试打开摄像头并验证读取帧
fn try_open_camera_with_backend(
    backend: CameraBackend,
    camear_index: i32,
) -> Result<VideoCapture, Box<dyn std::error::Error>> {
    let mut cam = VideoCapture::new(camear_index, backend.into())?;

    if !cam.is_opened()? {
        return Err(format!("后端 {:?} 打开摄像头后状态为未激活", backend).into());
    }

    // 激活摄像头
    let mut frame = Mat::default();
    let read_result = cam.read(&mut frame);

    match read_result {
        Ok(_) => {
            if frame.empty() {
                return Err(format!("后端 {:?} 读取到空帧", backend).into());
            }
        }
        Err(e) => {
            return Err(format!("后端 {:?} 读取帧失败: {}", backend, e).into());
        }
    }

    Ok(cam)
}
// 获取windows所有摄像头
fn get_windows_video_devices() -> windows::core::Result<Vec<(String, u32)>> {
    // 存放所有摄像头设备信息
    let mut devices = Vec::new();

    unsafe {
        // 创建ICreateDevEnum，用于获取摄像头设备，参考自微软官方文档
        // https://learn.microsoft.com/zh-cn/windows/win32/directshow/using-the-system-device-enumerator
        let dev_enum: ICreateDevEnum = CoCreateInstance(
            &CLSID_SystemDeviceEnum, // 系统设备枚举器的CLSID
            None,                    // 无聚合对象，传NULL
            CLSCTX_INPROC_SERVER,    // 进程内组件上下文
        )
        .map_err(|e| {
            error!("创建ICreateDevEnum失败");
            e
        })?;

        // 获取视频输入设备
        let mut enum_moniker: Option<IEnumMoniker> = None;
        dev_enum
            .CreateClassEnumerator(&CLSID_VideoInputDeviceCategory, &mut enum_moniker, 0)
            .map_err(|e| {
                error!("获取视频设备列表失败");
                e
            })?;

        // 若没有视频设备，直接返回空列表
        let Some(enum_moniker) = enum_moniker else {
            return Ok(vec![]);
        };

        let mut i = 0;
        loop {
            let mut moniker = [None];
            let mut fetched = 0;
            let result = enum_moniker.Next(&mut moniker, Some(&mut fetched));
            let moniker = moniker[0].clone();

            if result.is_err() || fetched == 0 || moniker.is_none() {
                break;
            }
            let moniker = moniker.unwrap();

            // 获取属性袋
            let prop_bag: Result<IPropertyBag, windows::core::Error> =
                moniker.BindToStorage(None, None);
            if prop_bag.is_err() {
                continue;
            }
            let prop_bag = prop_bag.unwrap();

            // 从属性中读取摄像头名字
            let name_bstr = BSTR::from("FriendlyName");
            let mut variant = VARIANT::from(BSTR::default());
            let read_result = prop_bag.Read(&name_bstr, &mut variant, None);

            // 获取设备名称
            let camera_name = if read_result.is_err() {
                format!("未知的摄像头 {}", i)
            } else {
                let bstr = variant.Anonymous.Anonymous.Anonymous.bstrVal.clone();
                if bstr.is_empty() {
                    format!("未知的摄像头 {}", i)
                } else {
                    bstr.to_string()
                }
            };

            // 清理VARIANT，释放内部资源
            VariantClear(&mut variant).ok();

            devices.push((camera_name, i));
            i += 1;
        }
    };

    Ok(devices)
}

// 验证摄像头有效性
fn is_camera_index_valid(index: u32) -> opencv::Result<bool> {
    let mut capture = VideoCapture::new(index as i32, opencv::videoio::CAP_ANY)?;
    let is_valid = capture.is_opened()?;

    // 立即释放资源，避免占用摄像头
    if is_valid {
        capture.release()?;
    }

    Ok(is_valid)
}

// 解锁屏幕
pub fn unlock(user_name: String, password: String) -> windows::core::Result<()> {
    let client = Client::new(HSTRING::from(r"\\.\pipe\MansonWindowsUnlockRustServer"));
    if client.is_err() {
        return Err(windows::core::Error::new(E_UNEXPECTED, "管道不存在"));
    }
    let client = client.unwrap();
    if let Err(e) = crate::utils::pipe::write(client.handle, format!("{}::FaceWinUnlock::{}", user_name, password)) {
        println!("向客户端写入数据失败: {:?}", e);
    }

    Ok(())
}

// 获取活体检测状态
#[tauri::command]
pub fn get_liveness_status() -> Result<CustomResult, CustomResult> {
    let model_path = ROOT_DIR.join("resources").join("detect.onnx");
    let model_exists = model_path.exists();

    Ok(CustomResult::success(None, Some(json!({
        "model_exists": model_exists,
        "model_path": model_path.to_str().unwrap_or("").to_string(),
        "enabled": model_exists
    }))))
}

// 使用 bcrypt 对密码进行哈希处理
fn hash_password(password: &str) -> Result<String, Box<dyn std::error::Error>> {
    const DEFAULT_COST: u32 = 12;
    let hash = bcrypt::hash(password, DEFAULT_COST)?;
    Ok(hash)
}

// 验证密码
fn verify_password(password: &str, hash: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let result = bcrypt::verify(password, hash)?;
    Ok(result)
}

// 对密码进行哈希处理（返回哈希值）
#[tauri::command]
pub fn hash_password_cmd(password: String) -> Result<CustomResult, CustomResult> {
    let hash = hash_password(&password)
        .map_err(|e| CustomResult::error(Some(format!("密码哈希失败: {}", e)), None))?;

    Ok(CustomResult::success(None, Some(json!({
        "hash": hash
    }))))
}

// 验证应用登录密码
#[tauri::command]
pub fn verify_app_password(password: String) -> Result<CustomResult, CustomResult> {
    // 从本地存储获取保存的哈希值
    let stored_hash = crate::utils::api::get_stored_password_hash();

    if stored_hash.is_empty() {
        return Err(CustomResult::error(Some("未设置登录密码".to_string()), None));
    }

    let is_valid = verify_password(&password, &stored_hash)
        .map_err(|e| CustomResult::error(Some(format!("密码验证失败: {}", e)), None))?;

    if is_valid {
        Ok(CustomResult::success(Some("密码验证成功".to_string()), None))
    } else {
        Err(CustomResult::error(Some("密码错误".to_string()), None))
    }
}

// 从本地存储获取保存的密码哈希
fn get_stored_password_hash() -> String {
    // 优先从 SQLite 数据库获取
    let pool_guard = DB_POOL.lock().unwrap();
    if let Some(ref pool) = *pool_guard {
        if let Ok(conn) = pool.get() {
            if let Ok(mut stmt) = conn.prepare("SELECT val FROM options WHERE key = ?") {
                if let Ok(mut rows) = stmt.query(["app_password_hash"]) {
                    if let Ok(Some(row)) = rows.next() {
                        return row.get::<_, String>(0).unwrap_or_default();
                    }
                }
            }
        }
    }

    // 回退到 localStorage（前端存储）
    // 注意：这里无法直接访问 JavaScript 的 localStorage
    // 所以主要依赖数据库存储

    String::new()
}