use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};
use windows::Win32::{Foundation::HWND, System::RemoteDesktop::WTSUnRegisterSessionNotification};

pub fn create_tray_menu<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Menu<R>, Box<dyn std::error::Error>> {
    let show_window = MenuItem::with_id(app, "show-window", "显示窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::new(app)?;
    menu.append_items(&[&show_window, &quit])?;

    Ok(menu)
}

pub fn create_system_tray<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Arc<TrayIcon<R>>, Box<dyn std::error::Error>> {
    let menu = create_tray_menu(app)?;

    let tray = Arc::new(
        TrayIconBuilder::new()
            .icon(app.default_window_icon().unwrap().clone())
            .menu(&menu)
            .show_menu_on_left_click(false)
            .tooltip("facewinunlock-tauri")
            .build(app)?,
    );
    let tray_handle_for_close = tray.clone();
    let window = app.get_webview_window("main").unwrap();

    tray.on_menu_event(move |app, event| match event.id.as_ref() {
        "show-window" => {
            let _ = window.show();
            let _ = window.set_focus();
        }
        "quit" => {
            let hwnd = window.hwnd().unwrap();
            unsafe {
                // 注销 WTS 通知
                let _ = WTSUnRegisterSessionNotification(HWND(hwnd.0));
            }
            let _ = tray_handle_for_close.set_visible(false);
            app.exit(0);
        }
        _ => {
            let _ = window.emit("menu-event", format!("unknow id {:?}", event.id().as_ref()));
        }
    });

    tray.on_tray_icon_event(|tray, event| match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } => {
            let app = tray.app_handle();
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        _ => {}
    });
    Ok(tray)
}
