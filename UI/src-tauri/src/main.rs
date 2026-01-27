// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 设置 WebView2 数据文件夹的环境变量，避免，System 账户运行时的权限问题
    let app_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let webview_data_dir = format!("{}\\facewinunlock-tauri\\EBWebView", app_data);
    
    // 检测开发目录存在
    let _ = std::fs::create_dir_all(&webview_data_dir);
    
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", webview_data_dir);

    facewinunlock_tauri_lib::run()
}
