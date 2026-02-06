use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle, sleep},
    time::Duration,
};
use windows::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::ICredentialProviderEvents,
        WindowsAndMessaging::{
            CallNextHookEx, HHOOK, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL,
            WH_MOUSE_LL,
        },
    },
    System::Threading::{
        CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW, CREATE_NO_WINDOW, NORMAL_PRIORITY_CLASS,
        STARTF_USESHOWWINDOW
    },
};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows_core::{HSTRING, PWSTR};

use crate::{
    read_facewinunlock_registry, Pipe::{read, Client, Server}, SharedCredentials
};

// 包装 COM 接口，使其可以跨线程传输
#[derive(Clone)]
struct SendableEvents(pub ICredentialProviderEvents);
// 声明这是安全的
unsafe impl Send for SendableEvents {}
unsafe impl Sync for SendableEvents {}

pub struct CPipeListener {
    pub is_unlocked: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
    pub server_thread: Option<JoinHandle<()>>,
    pub client_thread: Option<JoinHandle<()>>,
}

// 2026.01.14
// 在普通界面注册的hook无法获取到鼠标和键盘信息，锁屏界面是在winlogon中运行的
// 今天突然想到，此dll是会被winlogon直接加载的，我在dll中直接注册钩子是否能获取到鼠标和键盘事件？
// 10点左右尝试写了一个Demo，居然成功了！！！❤ヾ(≧▽≦*)o
static mut MOUSE_HOOK_ID: HHOOK = HHOOK(std::ptr::null_mut());
static mut KEYBOARD_HOOK_ID: HHOOK = HHOOK(std::ptr::null_mut());
// 鼠标键盘hook标志
static IS_MOUSE_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
static IS_KEYBOARD_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

// 是否可以发送run
static IS_SEND_RUN: AtomicBool = AtomicBool::new(false);
unsafe extern "system" fn hook_fn(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        if !IS_SEND_RUN.load(Ordering::SeqCst) {
            IS_SEND_RUN.store(true, Ordering::SeqCst);
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn keyboard_hook_fn(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        if !IS_SEND_RUN.load(Ordering::SeqCst) {
            IS_SEND_RUN.store(true, Ordering::SeqCst);
        }
    }

    unsafe { CallNextHookEx(Some(KEYBOARD_HOOK_ID), code, wparam, lparam) }
}

impl CPipeListener {
    pub fn stop_and_join(&mut self) {
        // 通知线程停止运行
        self.running.store(false, Ordering::SeqCst);

        // 连接自己
        if let Err(e) = Client::new(HSTRING::from(r"\\.\pipe\MansonWindowsUnlockRustServer")) {
            warn!("安全关闭线程失败: {}", e);
        }

        // 取出并等待 server 线程
        if let Some(thread) = self.server_thread.take() {
            if let Err(e) = thread.join() {
                warn!("线程Server终止失败: {:?}", e)
            }
        }

        // 取出并等待 client 线程
        if let Some(thread) = self.client_thread.take() {
            if let Err(e) = thread.join() {
                warn!("线程Client终止失败: {:?}", e)
            }
        }
    }

    pub fn start(
        provider_events: ICredentialProviderEvents,
        advise_context: usize,
        shared_creds_clone: Arc<Mutex<SharedCredentials>>,
    ) -> Arc<Mutex<Self>> {
        info!("CPipeListener::start - 启动管道监听");

        // 注册鼠标钩子
        let hook_id = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(hook_fn), None, 0) };
        if hook_id.is_err() {
            error!("设置鼠标钩子失败！错误码: {:?}", hook_id.err());
        } else {
            unsafe { MOUSE_HOOK_ID = hook_id.unwrap() };
            IS_MOUSE_HOOK_INSTALLED.store(true, Ordering::SeqCst);
        }

        // 注册键盘钩子
        let hook_id = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_fn), None, 0) };
        if hook_id.is_err() {
            error!("设置键盘钩子失败！错误码: {:?}", hook_id.err());
        } else {
            unsafe { KEYBOARD_HOOK_ID = hook_id.unwrap() };
            IS_KEYBOARD_HOOK_INSTALLED.store(true, Ordering::SeqCst);
        }

        let running = Arc::new(AtomicBool::new(true));
        let is_unlocked = Arc::new(AtomicBool::new(false));
        let running_clone = running.clone();
        let is_unlocked_clone = is_unlocked.clone();
        let sendable_events = SendableEvents(provider_events);

        let server_thread = thread::spawn(move || {
            info!("CPipeListener::start - 进入管道Server线程");
            let events_wrapper = sendable_events;
            let mut server = Server::new(HSTRING::from(r"\\.\pipe\MansonWindowsUnlockRustServer"));
            unsafe {
                while running_clone.load(Ordering::SeqCst) {
                    let f_connected = server.connect();
                    // 连接失败后退出循环
                    if f_connected.is_err() {
                        error!("管道连接失败：{:?}", f_connected.err());
                    } else {
                        // 如果运行状态为false，则退出循环
                        if !running_clone.load(Ordering::SeqCst) {
                            break;
                        }
                        match read(server.handle) {
                            Ok(content) => {
                                let parts: Vec<&str> = content.split("::FaceWinUnlock::").collect();

                                if parts.len() == 2 {
                                    let user_name = parts[0];
                                    let password = parts[1];

                                    let mut creds = shared_creds_clone.lock().unwrap();
                                    creds.username = user_name.to_string();
                                    creds.password = password.to_string();
                                    creds.is_ready = true;

                                    info!("收到用户名 {}", user_name);

                                    // 触发登录逻辑
                                    is_unlocked_clone.store(true, Ordering::SeqCst);
                                    let _ = events_wrapper.0.CredentialsChanged(advise_context);
                                }
                            }
                            Err(_e) => {
                                // 先不记了
                                // error!("读取管道数据失败：{:?}", e);
                            }
                        }
                        let _ = server.disconnect();
                    }
                }
            }

            info!("管道Server 线程已彻底退出");
        });

        let mut connect_client = false;
        if let Ok(result) = read_facewinunlock_registry("CONNECT_TO_PIPE") {
            if result.as_str() == "1" {
                connect_client = true;
            }
        } else {
            warn!("注册表配置读取失败!");
        }
        let running_client = running.clone();
        let client_thread = thread::spawn(move || {
            info!("CPipeListener::start - 进入管道Client线程");

            if connect_client {
                while running_client.load(Ordering::SeqCst) {
                    if let Ok(client_i) =
                        Client::new(HSTRING::from(r"\\.\pipe\MansonWindowsUnlockRustUnlock"))
                    {
                        loop {
                            // 运行状态为false，则退出循环
                            if !running_client.load(Ordering::SeqCst) {
                                break;
                            }

                            // 如果有数据，则发送
                            if IS_SEND_RUN.load(Ordering::SeqCst) {
                                if let Err(e) = crate::Pipe::write(client_i.handle, String::from("run")) {
                                    error!("向管道写入数据失败：{:?}", e);
                                }
                                IS_SEND_RUN.store(false, Ordering::SeqCst);
                                
                                // 发送完成
                                break;
                            }

                            // 休眠
                            sleep(Duration::from_millis(500));
                        }
                        
                    }
                    sleep(Duration::from_millis(500));
                }
            }

            info!("管道Client 线程已彻底退出");
        });

        let listener = Arc::new(Mutex::new(Self {
            is_unlocked: is_unlocked.clone(),
            running: running.clone(),
            server_thread: Some(server_thread),
            client_thread: Some(client_thread),
        }));

        listener
    }
}

impl Drop for CPipeListener {
    fn drop(&mut self) {
        info!("销毁一个 CPipeListener");
        // 卸载鼠标钩子
        if IS_MOUSE_HOOK_INSTALLED.load(Ordering::SeqCst) {
            unsafe { let _ = UnhookWindowsHookEx(MOUSE_HOOK_ID); };
            unsafe { MOUSE_HOOK_ID = HHOOK(std::ptr::null_mut()) };
            IS_MOUSE_HOOK_INSTALLED.store(false, Ordering::SeqCst);
        }
        if IS_KEYBOARD_HOOK_INSTALLED.load(Ordering::SeqCst) {
            unsafe { let _ = UnhookWindowsHookEx(KEYBOARD_HOOK_ID); };
            unsafe { KEYBOARD_HOOK_ID = HHOOK(std::ptr::null_mut()) };
            IS_KEYBOARD_HOOK_INSTALLED.store(false, Ordering::SeqCst);
        }
    }
}
