// 引入日志宏和日志库
#[macro_use] extern crate log;
extern crate simplelog;
use simplelog::*;
use windows::Win32::System::Registry::{RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ, REG_VALUE_TYPE};
use std::fs::File;

// 引入必要的系统类型和Win32 API绑定
use std::ffi::{c_void, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicI32, Ordering};

// Windows基础类型和COM接口
use windows::Win32::Foundation::{CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_INVALIDARG, HINSTANCE, S_FALSE, S_OK};
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
use windows::Win32::UI::Shell::ICredentialProvider;
use windows_core::{implement, Ref, BOOL, GUID, PCWSTR};
use windows::core::{Interface, HRESULT};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};

// 导入凭据提供程序和凭据的实现模块
pub mod CSampleProvider;
pub mod CSampleCredential;
pub mod CPipeListener;
pub mod Pipe;
use CSampleProvider::SampleProvider;
// 全局引用计数器，用于管理DLL的生命周期
// 当引用计数为0时，系统可以安全卸载DLL
static G_REF_COUNT: AtomicI32 = AtomicI32::new(0);

/// 增加DLL的引用计数
pub fn dll_add_ref() {
    let new_count = G_REF_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
    info!("DLL引用计数增加，当前计数: {}", new_count);
}

/// 减少DLL的引用计数
pub fn dll_release() {
    let new_count = G_REF_COUNT.fetch_sub(1, Ordering::SeqCst) - 1;
    info!("DLL引用计数减少，当前计数: {}", new_count);
}

/// 读取注册表数据
pub fn read_facewinunlock_registry(key_name: &str) -> windows::core::Result<String> {
    let reg_path = "SOFTWARE\\facewinunlock-tauri";
    // 打开HKLM下的注册表项
    let mut hkey: HKEY = HKEY::default();

    let os_str = OsStr::new(reg_path);
    let reg_path_ptr: Vec<u16> = os_str.encode_wide().chain(std::iter::once(0)).collect();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR::from_raw(reg_path_ptr.as_ptr()), // 子路径
            None, // 保留参数
            KEY_READ, // 只读
            &mut hkey, // 输出打开的注册表句柄
        )
    };

    if status.is_err() {
        return Err(windows_core::Error::new(HRESULT(0), format!("打开注册表失败: {}", status.0)));
    }

    // 查询值的长度
    let mut value_type = REG_VALUE_TYPE::default();
    let mut value_len = 0u32;

    let os_str = OsStr::new(key_name);
    let key_name_ptr: Vec<u16> = os_str.encode_wide().chain(std::iter::once(0)).collect();
    let status = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR::from_raw(key_name_ptr.as_ptr()),
            None,
            Some(&mut value_type),
            None,
            Some(&mut value_len),
        )
    };

    if status.is_err() {
        // 关闭注册表
        unsafe { let _ = RegCloseKey(hkey); };
        return Err(windows_core::Error::new(HRESULT(0), format!("查询注册表长度失败: {}", status.0)));
    }

    if value_type != REG_SZ {
        // 关闭注册表
        unsafe { let _ = RegCloseKey(hkey); };
        return Err(windows_core::Error::new(HRESULT(0), "值类型不是 REG_SZ"));
    }

    // 读取值内容
    let mut buffer = vec![0u16; (value_len / 2) as usize];
    let status = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR::from_raw(key_name_ptr.as_ptr()),
            None,
            None,
            Some(buffer.as_mut_ptr() as *mut u8), // 转换为 *mut u8
            Some(&mut value_len),
        )
    };

    if status.is_err() {
        // 关闭注册表
        unsafe { let _ = RegCloseKey(hkey); };
        return Err(windows_core::Error::new(HRESULT(0), format!("读取注册表值失败: {}", status.0)));
    }

    unsafe { let _ = RegCloseKey(hkey); };

    // 将 UTF-16 数组转换回 Rust String
    let value = String::from_utf16(&buffer)?.trim_end_matches('\0').to_string();
    Ok(value)
}

// 定义凭据提供程序的GUID，用于系统识别
// 8a7b9c6d-4e5f-89a0-8b7c-6d5e4f3e2d1c
pub const CLSID_SampleProvider: GUID = GUID::from_u128(0x8a7b9c6d_4e5f_89a0_8b7c_6d5e4f3e2d1c);

// 共享的凭据信息
pub struct SharedCredentials {
    pub username: String,
    pub password: String,
    pub domain: String,
    pub is_ready: bool,
}

/// 类工厂实现，用于创建凭据提供程序实例
/// COM规范要求通过类工厂来实例化组件
#[implement(IClassFactory)]
struct SampleClassFactory;

impl IClassFactory_Impl for SampleClassFactory_Impl {
    /// 创建组件实例
    /// punkouter: 聚合对象的外部IUnknown接口，通常为null
    /// riid: 要获取的接口ID
    /// ppv_object: 输出参数，接收创建的接口实例
    fn CreateInstance(
        &self,
        punkouter: Ref<'_, windows::core::IUnknown>,
        riid: *const GUID,
        ppv_object: *mut *mut std::ffi::c_void,
    ) -> windows::core::Result<()> {
        info!("SampleClassFactory::CreateInstance 被调用 - 开始创建凭据提供程序实例");
        
        // 不支持聚合，若提供了外部对象则返回错误
        if punkouter.is_some() {
            error!("不支持聚合，返回CLASS_E_NOAGGREGATION");
            return Err(CLASS_E_NOAGGREGATION.into());
        }

        unsafe {
            // 检查输出指针是否有效
            if ppv_object.is_null() {
                error!("输出指针为空，返回E_INVALIDARG");
                return Err(E_INVALIDARG.into());
            }
            
            // 实例化凭据提供程序并转换为ICredentialProvider接口
            let provider: ICredentialProvider = SampleProvider::new().into();
            // 查询请求的接口并返回
            let result = provider.query(riid, ppv_object);
            if result.is_err() {
                error!("接口查询失败: {:?}", result.message());
                Err(E_INVALIDARG.into())
            } else {
                info!("凭据提供程序实例创建成功");
                Ok(())
            }
        }
    }
    
    /// 锁定或解锁DLL，用于控制DLL卸载
    /// flock: true表示锁定（增加引用计数），false表示解锁（减少引用计数）
    fn LockServer(&self, flock: BOOL) -> windows::core::Result<()> {
        if flock.as_bool() {
            info!("LockServer: 锁定DLL");
            dll_add_ref();
        } else {
            info!("LockServer: 解锁DLL");
            dll_release();
        }
        Ok(())
    }
}

/// DLL导出函数，用于获取类工厂
/// rclsid: 要创建的组件的CLSID
/// riid: 要获取的接口ID（通常是IClassFactory）
/// ppv: 输出参数，接收类工厂接口
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    info!("DllGetClassObject 被调用 - 尝试获取类工厂");
    
    // 检查输入参数有效性
    if rclsid.is_null() || riid.is_null() || ppv.is_null() {
        error!("输入参数为空，返回E_INVALIDARG");
        return E_INVALIDARG;
    }

    // 检查请求的CLSID是否为我们的凭据提供程序
    if unsafe { *rclsid } == CLSID_SampleProvider {
        info!("请求的CLSID匹配，创建类工厂实例");
        let factory: IClassFactory = SampleClassFactory.into();
        // 查询请求的接口
        unsafe {
            let result = factory.query(riid, ppv);
            if result.is_err() {
                error!("类工厂接口查询失败: {:?}", result.message());
                E_INVALIDARG
            } else {
                info!("类工厂接口查询成功");
                S_OK
            }
        }
    } else {
        error!("不支持的CLSID，返回CLASS_E_CLASSNOTAVAILABLE");
        CLASS_E_CLASSNOTAVAILABLE
    }
}

/// DLL导出函数，用于判断DLL是否可以卸载
/// 当引用计数为0时可以卸载
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    let count = G_REF_COUNT.load(Ordering::SeqCst);
    info!("DllCanUnloadNow 被调用 - 当前引用计数: {}", count);
    
    if count == 0 {
        info!("引用计数为0，可以卸载DLL");
        S_OK
    } else {
        info!("引用计数不为0，不能卸载DLL");
        S_FALSE
    }
}

/// DLL入口点函数，处理DLL加载和卸载事件
/// hinst_dll: DLL实例句柄
/// dw_reason: 调用原因（加载、卸载等）
/// reserved: 保留参数
#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllMain(
    _hinst_dll: HINSTANCE,
    dw_reason: u32,
    _reserved: *mut c_void,
) -> BOOL {
    match dw_reason {
        DLL_PROCESS_ATTACH => {
            // 读取注册表设置
            let result = read_facewinunlock_registry("DLL_LOG_PATH");
            let mut log_path = String::from("C:");

            if let Ok(log_path_reg) = result.clone() {
                log_path = if log_path_reg.starts_with("\\\\?\\") {
                    log_path_reg["\\\\?\\".len()..].to_string()
                } else {
                    log_path_reg
                };
            }

            // 初始化日志系统
            if let Ok(file) = File::create(log_path + "\\facewinunlock.log") {
                // 日志时间太麻烦，不搞了，没有日期影响不大
                let mut config = ConfigBuilder::new();
                let _ = config.set_time_offset_to_local();
                config.add_filter_ignore_str("tract_onnx");  // 过滤 tract_onnx 库的日志
                config.add_filter_ignore_str("tract_core");  // 过滤 tract_core 库的日志
                config.add_filter_ignore_str("tract");       // 过滤 tract 库的日志
                let config = config.build();
                match CombinedLogger::init(
                    vec![
                        WriteLogger::new(
                            LevelFilter::Info,
                            config,
                            file
                        ),
                    ]
                ) {
                    Ok(_) => info!("日志系统初始化成功"),
                    _ => {},
                }
            }
            
            info!("DllMain: 基础框架初始化完成");

            if let Err(e) = result {
                warn!("从注册表加载配置失败：{}", e);
            }
        }
        // 可以添加其他事件的处理（如DLL_PROCESS_DETACH）
        _ => info!("DllMain: 处理事件，原因代码: {}", dw_reason),
    }
    BOOL::from(true)
}