use tauri_plugin_log::log::{error};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::DefSubclassProc,
        WindowsAndMessaging::{
            WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK,
            WTS_SESSION_UNLOCK,
        },
    },
};

use crate::
    utils::api::stop_camera
;
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
                }
            }
            WTS_SESSION_UNLOCK => {

            }
            _ => {}
        }
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}
