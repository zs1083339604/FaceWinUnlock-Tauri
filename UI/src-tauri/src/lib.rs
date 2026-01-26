use std::{
    env,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicI32}, Arc, Mutex
    },
};
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use tauri::{tray::TrayIcon, Manager, Wry};
use windows::Win32::{
    Foundation::HWND,
    System::RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION},
    UI::Shell::SetWindowSubclass,
};
pub mod modules;
pub mod proc;
pub mod utils;
pub mod liveness;
use modules::faces::{
    check_face_from_camera, check_face_from_img, save_face_registration, verify_face,
};
use modules::init::{
    check_admin_privileges, check_camera_status, deploy_core_components, uninstall_init,
};
use modules::options::write_to_registry;
use opencv::{
    core::Ptr,
    objdetect::{FaceDetectorYN, FaceRecognizerSF},
    videoio::VideoCapture,
};
use proc::wnd_proc_subclass;
use tauri_plugin_log::{Target, TargetKind};
use utils::api::{
    get_now_username, init_model, open_camera, open_directory, stop_camera, test_win_logon,
    close_app, get_liveness_status, get_camera, enable_global_autostart, disable_global_autostart,
    check_global_autostart, verify_app_password, hash_password_cmd,
};
mod tray;
use tray::create_system_tray;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

pub struct OpenCVResource<T> {
    pub inner: T,
}
unsafe impl<T> Send for OpenCVResource<T> {}
unsafe impl<T> Sync for OpenCVResource<T> {}
// 持久存储模型
pub struct AppState {
    pub detector: Option<OpenCVResource<Ptr<FaceDetectorYN>>>,
    pub recognizer: Option<OpenCVResource<Ptr<FaceRecognizerSF>>>,
    pub camera: Option<OpenCVResource<VideoCapture>>,
}

// 是否退出线程
static IS_BREAK_THREAD: AtomicBool = AtomicBool::new(true);
// 是否正在运行面容识别？
static IS_RUN: AtomicBool = AtomicBool::new(false);
// 多长时间进行重试？
static RETRY_DELAY: AtomicI32 = AtomicI32::new(10000);

// 定义全局只读连接池，用来在解锁中对数据库读操作
lazy_static::lazy_static! {
    // 系统托盘
    static ref GLOBAL_TRAY: Mutex<Option<Arc<TrayIcon<Wry>>>> = Mutex::new(None);
    static ref TRAY_IS_READY: Mutex<bool> = Mutex::new(false);
    static ref DB_POOL: Mutex<Option<Pool<SqliteConnectionManager>>> = Mutex::new(None);
    // 不在使用状态管理，因为proc获取不到
    static ref APP_STATE: Mutex<AppState> = Mutex::new(AppState {
        detector: None,
        recognizer: None,
        camera: None,
    });

    // 全局只读软件根目录
    pub static ref ROOT_DIR: &'static Path = {
        let exe_path = match env::current_exe() {
            Ok(path) => path,
            // 失败时回退到当前工作目录
            Err(_) => env::current_dir().unwrap(),
        };
        let root_dir: PathBuf = match exe_path.parent() {
            Some(parent) => parent.to_path_buf(),
            None => {
                let current_dir = env::current_dir().unwrap();
                current_dir
            }
        };
        Box::leak(Box::new(root_dir)).as_path()
    };
}

// 计时器，确定何时调用面容识别代码
static IS_LOCKED: AtomicBool = AtomicBool::new(false);
const TIMER_ID_LOCK_CHECK: usize = 1001;

// 全局摄像头索引
static CAMERA_INDEX: AtomicI32 = AtomicI32::new(0);
// 面容不匹配时，当前的尝试次数
static MATCH_FAIL_COUNT: AtomicI32 = AtomicI32::new(0);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 获取软件安装目录，用于将日志放到软件安装目录下
    let log_path = ROOT_DIR.join("logs");
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                let main = app
                    .get_webview_window("main")
                    .expect("no main window");
                if !main.is_visible().unwrap() {
                    main.show().unwrap();
                }
                main.set_focus().unwrap();
            }))
            .plugin(tauri_plugin_fs::init())
            // 对话框
            .plugin(tauri_plugin_dialog::init())
            // 注册状态管理
            // .manage(AppState {
            //     detector: RwLock::new(None),
            //     recognizer: RwLock::new(None),
            //     camera: RwLock::new(None),
            // })
            // 文件系统插件
            .plugin(tauri_plugin_opener::init())
            .plugin(tauri_plugin_sql::Builder::default().build())
            // 注册日志插件
            .plugin(
                tauri_plugin_log::Builder::new()
                    .targets([
                        Target::new(TargetKind::Stdout),
                        Target::new(TargetKind::Webview),
                        Target::new(TargetKind::Folder {
                            path: log_path,
                            file_name: Some("app".to_string()),
                        }),
                    ])
                    .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                    .filter(|attrs| {
                        // 过滤掉 onnxruntime 和 tracing 的详细日志
                        let target = attrs.target();
                        !target.starts_with("onnxruntime") && !target.starts_with("tracing")
                    })
                    .build(),
            )
            .setup(|app| {
                let _ = create_system_tray(app.app_handle());
                let window = app.get_webview_window("main").unwrap();
                #[cfg(debug_assertions)] // 仅在调试(debug)版本中包含此代码
                {
                    window.open_devtools();
                    window.close_devtools();
                }

                #[cfg(windows)]
                {
                    let window = app.get_webview_window("main").unwrap();
                    let hwnd = window.hwnd().unwrap();
                    unsafe {
                        // 注册 WTS 通知
                        let _ =
                            WTSRegisterSessionNotification(HWND(hwnd.0), NOTIFY_FOR_THIS_SESSION);

                        // 注入子类化回调来捕获 WM_WTSSESSION_CHANGE
                        // on_window_event 收不到这个消息
                        let _ = SetWindowSubclass(HWND(hwnd.0), Some(wnd_proc_subclass), 0, 0);
                    }
                }

                let args: Vec<String> = env::args().collect();
                let is_silent = args.iter().any(|arg| arg == "-s" || arg == "--silent" || arg == "--s");
                if !is_silent {
                    // 只有不是静默启动时才显示
                    window.show().unwrap();
                }

                // 初始化活体检测器（后台线程）- ONNX Runtime 模型
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let mut detector = liveness::LIVENESS_DETECTOR.lock().unwrap();
                    if let Err(e) = detector.initialize() {
                        tauri_plugin_log::log::error!("活体检测器初始化失败: {:?}", e);
                    } else {
                        tauri_plugin_log::log::info!("活体检测器初始化成功");
                    }
                });

                Ok(())
            })
            .invoke_handler(tauri::generate_handler![
                // init 初始化模块
                check_admin_privileges,
                check_camera_status,
                deploy_core_components,
                uninstall_init,
                // 面容模块
                check_face_from_img,
                check_face_from_camera,
                verify_face,
                save_face_registration,
                // 配置模块
                write_to_registry,
                // 通用api
                get_now_username,
                test_win_logon,
                init_model,
                open_camera,
                stop_camera,
                get_camera,
                open_directory,
                enable_global_autostart,
                disable_global_autostart,
                check_global_autostart,
                close_app,
                get_liveness_status,
                // 认证模块
                verify_app_password,
                hash_password_cmd,
            ])
            .on_window_event(|window, event| {
                if window.label() == "main" {
                    match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            let _ = window.hide();
                        }
                        _ => {}
                    }
                }
            })
    }
    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}