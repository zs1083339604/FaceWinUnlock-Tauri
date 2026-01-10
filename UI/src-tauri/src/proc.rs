use opencv::{objdetect::FaceRecognizerSF_DisType, prelude::FaceRecognizerSFTraitConst};
use serde::Deserialize;
use std::{sync::atomic::Ordering, thread::sleep, time::Duration};
use tauri_plugin_log::log::{error, info, warn};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::DefSubclassProc,
        WindowsAndMessaging::{
            KillTimer, SetTimer, WM_TIMER, WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK,
            WTS_SESSION_UNLOCK,
        },
    },
};

use crate::{
    modules::faces::{get_feature, load_face_data, read_mat_from_camera},
    utils::api::{open_camera, stop_camera, unlock},
    APP_STATE, CAMERA_INDEX, DB_POOL, IS_LOCKED, ROOT_DIR, TIMER_ID_LOCK_CHECK,
};

const MAX_SUCCESS: usize = 3;
const MAX_FAIL: usize = 3;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")] // 适配 JSON 中的驼峰命名
pub struct FaceExtraData {
    /// 面容别名
    pub alias: String,
    /// 置信度阈值
    pub threshold: f32,
    /// 是否在列表页显示图片缩略图
    pub view: bool,
    /// 人脸检测置信度阈值
    pub face_detection_threshold: f32,
}

// windows回调
pub unsafe extern "system" fn wnd_proc_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_WTSSESSION_CHANGE {
        let event_type = wparam.0 as u32;
        let _session_id = lparam.0 as u32;

        match event_type {
            WTS_SESSION_LOCK => {
                // 屏幕锁屏，关闭摄像头，因为不确定用户是否开启了摄像头
                if let Err(e) = stop_camera() {
                    error!("关闭摄像头失败: {}", e.to_string());
                } else {
                    // 摄像头处于关闭状态，可以进行面容识别
                    // 获取数据库设置
                    if let Ok(pool_guard) = DB_POOL.lock() {
                        if let Some(pool) = pool_guard.as_ref() {
                            // 获取连接
                            let conn = pool.get();
                            if conn.is_err() {
                                error!("从连接池获取连接失败: {:?}", conn.err());
                            } else {
                                let conn = conn.unwrap();

                                if let Ok(count) = conn.query_row(
                                    "SELECT COUNT(id) as count FROM faces;",
                                    [],
                                    |row| row.get::<&str, i32>("count"),
                                ) {
                                    if count > 0 {
                                        // 有人脸才进行识别
                                        let result: Result<String, _> = conn.query_row(
                                            "SELECT val FROM options WHERE key = 'is_initialized';",
                                            [],
                                            |row| row.get::<&str, String>("val"),
                                        );
                                        let is_initialized = match result {
                                            Ok(val) => val,
                                            Err(
                                                r2d2_sqlite::rusqlite::Error::QueryReturnedNoRows,
                                            ) => String::from("false"),
                                            Err(e) => {
                                                error!("从数据库获取设置失败: {:?}", e);
                                                String::new()
                                            }
                                        };

                                        // 只有初始化完成才启动
                                        if is_initialized == "true" {
                                            let result: Result<String, _> = conn.query_row(
                                                "SELECT val FROM options WHERE key = 'faceRecogDelay';",
                                                [],
                                                |row| row.get::<&str, String>("val"),
                                            );

                                            let time = match result {
                                                Ok(val) => val,
                                                Err(r2d2_sqlite::rusqlite::Error::QueryReturnedNoRows) => {
                                                    String::from("10.0")
                                                }
                                                Err(e) => {
                                                    error!("从数据库获取设置失败: {:?}", e);
                                                    String::new()
                                                }
                                            };

                                            if !time.is_empty() {
                                                let time_ms: f32 = match time.parse::<f32>() {
                                                    Ok(seconds) => seconds * 1000.0,
                                                    Err(e) => {
                                                        error!(
                                                            "秒数字符串转换失败: {}，使用默认值 10000 毫秒",
                                                            e
                                                        );
                                                        10.0 * 1000.0
                                                    }
                                                };

                                                // 读取摄像头索引
                                                let result: Result<String, _> = conn.query_row(
                                                    "SELECT val FROM options WHERE key = 'camera';",
                                                    [],
                                                    |row| row.get::<&str, String>("val"),
                                                );

                                                let camera_index = match result {
                                                    Ok(val) => val,
                                                    Err(r2d2_sqlite::rusqlite::Error::QueryReturnedNoRows) => {
                                                        String::from("0")
                                                    }
                                                    Err(e) => {
                                                        error!("从数据库获取设置失败: {:?}", e);
                                                        String::new()
                                                    }
                                                };

                                                CAMERA_INDEX.store(
                                                    camera_index.parse().unwrap_or(0),
                                                    Ordering::SeqCst,
                                                );

                                                IS_LOCKED.store(true, Ordering::SeqCst);
                                                // 设置一个定时器
                                                // 当时间到达时，系统会发送 WM_TIMER 消息
                                                unsafe {
                                                    SetTimer(
                                                        Some(hwnd),
                                                        TIMER_ID_LOCK_CHECK,
                                                        time_ms as u32,
                                                        None,
                                                    )
                                                };
                                                info!("计时器已设置 {}", time_ms);
                                            } else {
                                                error!("未获取到延迟秒，停止启动面容识别");
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            error!("数据库连接池不存在");
                        }
                    } else {
                        error!("从全局变量获取连接池失败");
                    }
                }

                // println!("[会话{}] 屏幕已锁屏", session_id);
            }
            WTS_SESSION_UNLOCK => {
                // println!("[会话{}] 屏幕已解锁", session_id);
                // 解锁取消计时器
                IS_LOCKED.store(false, Ordering::SeqCst);
                unsafe {
                    let _ = KillTimer(Some(hwnd), TIMER_ID_LOCK_CHECK);
                };
            }
            _ => {}
        }
    } else if msg == WM_TIMER {
        if wparam.0 == TIMER_ID_LOCK_CHECK {
            // 关闭定时器，防止重复触发
            unsafe {
                let _ = KillTimer(Some(hwnd), TIMER_ID_LOCK_CHECK);
            };

            // 二次检查状态
            if IS_LOCKED.load(Ordering::SeqCst) {
                // 先打开摄像头
                let result = open_camera(None, CAMERA_INDEX.load(Ordering::SeqCst));
                if let Err(e) = result {
                    error!("打开摄像头失败 {}", e.msg);
                } else {
                    // 摄像头成功打开
                    if let Err(e) = run() {
                        error!("运行面容解锁失败: {:?}", e);
                    };

                    if let Err(e) = stop_camera() {
                        error!("停止摄像头失败: {}", e.msg);
                    };
                }
            }
        }
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

fn run() -> Result<bool, String> {
    // 从全局变量获取连接并查询
    if let Ok(pool_guard) = DB_POOL.lock() {
        if let Some(pool) = pool_guard.as_ref() {
            // 获取连接
            let conn = pool
                .get()
                .map_err(|e| format!("从连接池获取连接失败：{:?}", e))?;
            // 获取面容数据
            let mut faces = conn
                .prepare("SELECT * FROM faces;")
                .map_err(|e| format!("准备查询面容数据失败：{:?}", e))?;
            let rows = faces
                .query_map([], |row| {
                    // 读取基础字段
                    let id = row.get::<&str, i32>("id")?;
                    let user_name = row.get::<&str, String>("user_name")?;
                    let user_pwd = row.get::<&str, String>("user_pwd")?;
                    let account_type = row.get::<&str, String>("account_type")?;
                    let face_token = row.get::<&str, String>("face_token")?;
                    let json_data_str = row.get::<&str, String>("json_data")?;
                    let create_time = row.get::<&str, String>("createTime")?;

                    // 解析 JSON 字符串为结构体
                    let json_data: FaceExtraData = serde_json::from_str(&json_data_str)
                        .map_err(|_e| r2d2_sqlite::rusqlite::Error::ExecuteReturnedResults)?;

                    // 返回
                    Ok((
                        id,
                        user_name,
                        user_pwd,
                        account_type,
                        face_token,
                        json_data,
                        create_time,
                    ))
                })
                .map_err(|e| format!("查询面容数据失败：{:?}", e))?;

            for row in rows {
                let (
                    id,
                    user_name,
                    user_pwd,
                    account_type,
                    mut face_token,
                    json_data,
                    _create_time,
                ) = row.map_err(|e| format!("获取1条面容数据失败：{:?}", e))?;
                // 加载数据
                face_token.push_str(".face");
                let path = ROOT_DIR.join("faces").join(face_token);
                // 解析面容数据
                let face = load_face_data(&path);
                if face.is_err() {
                    error!("加载面容数据失败：{:?}", path);
                    continue;
                }

                let face = face.unwrap();
                // 参考面容转换失败，跳过当前用户
                let dst_feature = face.to_mat();
                if dst_feature.is_err() {
                    error!("{}, 转换参考面容数据失败：{:?}", json_data.alias, path);
                    continue;
                }
                let dst_feature = dst_feature.unwrap();

                let mut success_count = 0;
                let mut fail_count = 0;

                loop {
                    // 读取一帧，摄像头的操作一旦失败，必须退出函数
                    let frame =
                        read_mat_from_camera().map_err(|e| format!("摄像头读取失败: {}", e))?;
                    // 提取特征点
                    let cur_feature = match get_feature(&frame, json_data.face_detection_threshold)
                    {
                        Ok(feature) => feature,
                        Err(e) => {
                            let err_msg = format!("特征提取失败: {}", e);
                            if err_msg.contains("未检测到人脸") {
                                // 未检测到人脸不动
                                sleep(Duration::from_millis(200));
                                continue;
                            } else {
                                // 其他错误退出整个函数
                                return Err(err_msg);
                            }
                        }
                    };

                    let score = {
                        // 必须实时获取，否则会死锁
                        let app_state = APP_STATE
                            .lock()
                            .map_err(|e| format!("获取app状态失败 {}", e))?;

                        let Some(recognizer) = app_state.recognizer.as_ref() else {
                            return Err(String::from("人脸识别模型未初始化"));
                        };

                        recognizer
                            .inner
                            .match_(
                                &dst_feature,
                                &cur_feature,
                                FaceRecognizerSF_DisType::FR_COSINE.into(),
                            )
                            .map_err(|e| format!("特征匹配失败: {}", e))?
                    };

                    if score * 100.0 >= json_data.threshold.into() {
                        // 匹配成功，次数+1
                        success_count += 1;
                        if success_count >= MAX_SUCCESS {
                            // 大于3次，算面容匹配成功
                            let user_name = if account_type == "local" {
                                format!(".\\{}", user_name)
                            } else {
                                user_name
                            };

                            if let Err(e) = unlock(user_name, user_pwd) {
                                return Err(format!("调用解锁函数失败：{}", e));
                            } else {
                                if let Err(e) = insert_unlock_log(&conn, id, true) {
                                    warn!("插入解锁日志失败：{}", e);
                                };
                                return Ok(true);
                            }
                        }
                    } else {
                        success_count = 0;
                        fail_count += 1;
                        if fail_count >= MAX_FAIL {
                            break;
                        }
                    }

                    sleep(Duration::from_millis(50));
                }
            }
            // 发个假的用户名密码，通知用户解锁失败
            if let Err(e) = unlock(String::from("null"), String::from("null")) {
                return Err(format!("调用解锁函数失败：{}", e));
            }
            if let Err(e) = insert_unlock_log(&conn, -1, false) {
                warn!("插入解锁日志失败：{}", e);
            };
            return Ok(false);
        } else {
            return Err(String::from("连接池不存在"));
        }
    } else {
        return Err(String::from("从全局变量获取连接池失败"));
    }
}

// 插入解锁日志到数据库
// 为了统一，这里其实应该前端添加数据，可以实现rust只读，前端读写，并实现响应式数据的同步更新
// 但是需要包装一个全局变量，存储app，然后向前端发送通知，这里我懒得做了，所以直接后端插入数据了，前端不更新
fn insert_unlock_log(
    conn: &r2d2_sqlite::rusqlite::Connection,
    face_id: i32,
    is_unlock: bool,
) -> Result<(), String> {
    let mut insert_stmt = conn
        .prepare("INSERT INTO unlock_log (face_id, is_unlock) VALUES (?1, ?2)")
        .map_err(|e| format!("准备插入解锁日志语句失败：{:?}", e))?;

    // 插入数据
    insert_stmt
        .execute(r2d2_sqlite::rusqlite::params![
            face_id,
            if is_unlock { 1 } else { 0 }
        ])
        .map_err(|e| format!("插入解锁日志失败：{:?}", e))?;
    Ok(())
}
