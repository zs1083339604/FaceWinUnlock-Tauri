use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle, sleep},
    time::{Duration, SystemTime, UNIX_EPOCH},
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
    Pipe::{read, Client, Server},
    SharedCredentials,
    read_facewinunlock_registry
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
// 是否可以发送run
static IS_SEND_RUN: AtomicBool = AtomicBool::new(false);
// 记录上一次发送管道消息的时间戳（毫秒）
static mut LAST_SEND_TIME: u128 = 0;
// 检查是否可以发送（避免短时间重复发送）
fn can_send() -> bool {
    unsafe {
        // 获取当前时间戳（毫秒）
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        // 设置 1 秒防抖延迟
        let delay: u128 = 1000;

        // 如果距离上次发送超过最小间隔，更新时间并允许发送
        if now - LAST_SEND_TIME >= delay {
            LAST_SEND_TIME = now;
            true
        } else {
            false
        }
    }
}
unsafe extern "system" fn hook_fn(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        if can_send() {
            IS_SEND_RUN.store(true, Ordering::SeqCst);
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn keyboard_hook_fn(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        if can_send() {
            IS_SEND_RUN.store(true, Ordering::SeqCst);
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
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
        }

        // 注册键盘钩子
        let hook_id = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_fn), None, 0) };
        if hook_id.is_err() {
            error!("设置键盘钩子失败！错误码: {:?}", hook_id.err());
        } else {
            unsafe { KEYBOARD_HOOK_ID = hook_id.unwrap() };
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

                                    info!("成功解析用户信息: {}", user_name);

                                    // 触发登录逻辑
                                    is_unlocked_clone.store(true, Ordering::SeqCst);
                                    let _ = events_wrapper.0.CredentialsChanged(advise_context);
                                } else {
                                    warn!("收到未知格式的数据: {}", content);
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

        let running_client = running.clone();
        let client_thread = thread::spawn(move || {
            info!("CPipeListener::start - 进入管道Client线程");
            let mut client = None;
            while running_client.load(Ordering::SeqCst) {
                if let Ok(client_i) =
                    Client::new(HSTRING::from(r"\\.\pipe\MansonWindowsUnlockRustClient"))
                {
                    info!("Client管道连接成功.");
                    client = Some(client_i);
                    break;
                } else {
                    // 连接失败，尝试启动 UI 进程
                    if let Ok(exe_path) = read_facewinunlock_registry("EXE_PATH") {
                        info!("管道未就绪，尝试启动 UI 进程: {}", exe_path);
                        let mut si = STARTUPINFOW::default();
                        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
                        si.dwFlags = STARTF_USESHOWWINDOW;
                        si.wShowWindow = SW_HIDE.0 as u16; // 静默启动
                        
                        let mut pi = PROCESS_INFORMATION::default();
                        
                        // 命令行参数需要包含 --silent
                        let mut command_line = format!("\"{}\" --silent", exe_path)
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect::<Vec<u16>>();
                        
                        unsafe {
                            let success = CreateProcessW(
                                None,
                                Some(PWSTR(command_line.as_mut_ptr())),
                                None,
                                None,
                                false,
                                CREATE_NO_WINDOW | NORMAL_PRIORITY_CLASS,
                                None,
                                None,
                                &si,
                                &mut pi,
                            );
                            
                            if success.is_ok() {
                                info!("UI 进程启动成功");
                                // 关闭句柄，避免泄漏
                                let _ = windows::Win32::Foundation::CloseHandle(pi.hProcess);
                                let _ = windows::Win32::Foundation::CloseHandle(pi.hThread);
                            } else {
                                error!("UI 进程启动失败: {:?}", success.err());
                            }
                        }
                    }
                }
                sleep(Duration::from_millis(1000)); // 启动后等待久一点再试
            }

            if let Some(client) = client {
                // 检查是否启用了开机面容识别
                let boot_face_recog_enabled = match read_facewinunlock_registry("BOOT_FACE_RECOG") {
                    Ok(val) => val == "1",
                    Err(_) => false,
                };
                
                if boot_face_recog_enabled {
                    info!("开机面容识别已启用，立即触发面容识别");
                    // 等待一小段时间确保 UI 端管道已就绪
                    sleep(Duration::from_millis(500));
                    if let Err(e) = crate::Pipe::write(client.handle, String::from("run")) {
                        error!("开机面容识别触发失败: {:?}", e);
                    }
                }
                
                while running_client.load(Ordering::SeqCst) {
                    if IS_SEND_RUN.load(Ordering::SeqCst) {
                        IS_SEND_RUN.store(false, Ordering::SeqCst);
                        if let Err(e) = crate::Pipe::write(client.handle, String::from("run")) {
                            println!("向客户端写入数据失败: {:?}", e);
                        }
                    }
                    sleep(Duration::from_millis(10));
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
        unsafe { UnhookWindowsHookEx(MOUSE_HOOK_ID).unwrap() };
        unsafe { UnhookWindowsHookEx(KEYBOARD_HOOK_ID).unwrap() };
    }
}
