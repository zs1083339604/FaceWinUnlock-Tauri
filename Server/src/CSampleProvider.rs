// 引入必要的Win32 API和同步原语
use windows::Win32::{Foundation::{HANDLE, STATUS_SUCCESS}, Security::Authentication::Identity::{LsaConnectUntrusted, LsaDeregisterLogonProcess, LsaLookupAuthenticationPackage, LSA_STRING}, UI::Shell::*};
use std::sync::{atomic::Ordering, Arc, Mutex};
use crate::{dll_add_ref, dll_release, read_facewinunlock_registry, CPipeListener::CPipeListener, CSampleCredential::SampleCredential, SharedCredentials};
use windows_core::{implement, BOOL, PSTR, PWSTR};

/// 凭据提供程序主类，负责管理凭据和与系统交互
#[implement(ICredentialProvider)]
pub struct SampleProvider {
    // 内部状态（使用互斥锁保证线程安全）
    inner: Mutex<ProviderInner>,
}

/// 凭据提供程序的内部状态
struct ProviderInner {
    usage_scenario: CREDENTIAL_PROVIDER_USAGE_SCENARIO, // 使用场景（登录、解锁等）
    events: Option<ICredentialProviderEvents>, // 系统事件接口
    advise_context: usize, // 通知上下文ID
    listener: Option<Arc<Mutex<CPipeListener>>>, // 管道监听器实例
    pub shared_creds: Arc<Mutex<SharedCredentials>>, // 共享的凭据列表
    pub auth_package_id: u32, // 认证包ID
    pub credential: Option<ICredentialProviderCredential>,
}

impl SampleProvider {
    /// 创建新的凭据提供程序实例
    pub fn new() -> Self {
        info!("SampleProvider::new - 创建凭据提供程序实例");
        dll_add_ref(); // 增加DLL引用计数
        // 创建共享的凭据列表实例
        let shared = Arc::new(Mutex::new(SharedCredentials {
            username: String::new(),
            password: String::new(),
            domain: String::from("."),
            is_ready: false,
        }));

        // 获取认证包ID
        let auth_id = retrieve_negotiate_auth_package().unwrap_or(0);

        Self {
            inner: Mutex::new(ProviderInner {
                usage_scenario: CPUS_LOGON, // 默认场景为登录
                events: None,
                advise_context: 0,
                listener: None,
                shared_creds: shared,
                auth_package_id: auth_id,
                credential: None
            }),
        }
    }
}

/// 实现Drop trait，在对象销毁时减少引用计数
impl Drop for SampleProvider {
    fn drop(&mut self) {
        // 准备安全停止监听线程
        let mut inner = self.inner.lock().unwrap();
        if let Some(listener) = inner.listener.take() {
            let mut listener = listener.lock().unwrap();
            listener.stop_and_join();
        }

        inner.listener = None;

        info!("SampleProvider::drop - 销毁凭据提供程序实例");
        dll_release(); // 减少DLL引用计数
    }
}

/// 实现ICredentialProvider接口，这是凭据提供程序的核心接口
impl ICredentialProvider_Impl for SampleProvider_Impl {
    /// 设置凭据提供程序的使用场景
    /// cpus: 使用场景（登录、解锁、切换用户等）
    /// _dwflags: 附加标志
    fn SetUsageScenario(&self, cpus: CREDENTIAL_PROVIDER_USAGE_SCENARIO, _dwflags: u32) -> windows_core::Result<()> {
        info!("SampleProvider::SetUsageScenario - 设置使用场景: {:?}", cpus);
        let mut inner = self.inner.lock().unwrap();
        inner.usage_scenario = cpus; // 保存使用场景
        Ok(())
    }

    /// 设置序列化的凭据信息（用于预填充凭据，这里空实现）
    /// _pcpcs: 序列化的凭据数据
    fn SetSerialization(&self, _pcpcs: *const CREDENTIAL_PROVIDER_CREDENTIAL_SERIALIZATION) -> windows_core::Result<()> {
        info!("SampleProvider::SetSerialization - 空实现");
        Ok(())
    }

    /// 注册系统事件通知
    /// pcpe: 系统提供的事件接口
    /// upadvisecontext: 通知上下文ID
    fn Advise(&self, pcpe: windows_core::Ref<ICredentialProviderEvents>, upadvisecontext: usize) -> windows_core::Result<()> {
        info!("SampleProvider::Advise - 注册事件通知，上下文ID: {}", upadvisecontext);
        let mut inner = self.inner.lock().unwrap();
        inner.events = pcpe.clone(); // 保存事件接口
        inner.advise_context = upadvisecontext; // 保存上下文ID

        // 启动管道监听，传入系统事件接口
        if let Some(events) = &inner.events {
            inner.listener = Some(CPipeListener::start(events.clone(), upadvisecontext, inner.shared_creds.clone()));
        }

        Ok(())
    }

    /// 取消事件通知
    fn UnAdvise(&self) -> windows_core::Result<()> {
        info!("SampleProvider::UnAdvise - 取消事件通知");
        let mut inner = self.inner.lock().unwrap();
        inner.events = None; // 清除事件接口
        inner.advise_context = 0; // 重置上下文ID
        Ok(())
    }

    /// 获取字段描述符的数量
    fn GetFieldDescriptorCount(&self) -> windows_core::Result<u32> {
        let count = 2; // 我们定义了2个字段：图标和文本
        info!("SampleProvider::GetFieldDescriptorCount - 字段数量: {}", count);
        Ok(count)
    }

    /// 获取指定索引的字段描述符
    /// dwindex: 字段索引
    fn GetFieldDescriptorAt(&self, dwindex: u32) -> windows_core::Result<*mut CREDENTIAL_PROVIDER_FIELD_DESCRIPTOR> {
        info!("SampleProvider::GetFieldDescriptorAt - 获取字段 {} 的描述符", dwindex);
        unsafe {
            // 分配字段描述符的内存（使用CoTaskMemAlloc，系统会负责释放）
            let size = std::mem::size_of::<CREDENTIAL_PROVIDER_FIELD_DESCRIPTOR>();
            let ptr = windows::Win32::System::Com::CoTaskMemAlloc(size) as *mut CREDENTIAL_PROVIDER_FIELD_DESCRIPTOR;
            if ptr.is_null() {
                error!("SampleProvider::GetFieldDescriptorAt - 内存分配失败");
                return Err(windows::Win32::Foundation::E_OUTOFMEMORY.into());
            }
    
            // 根据索引设置字段类型和标签
            let (ft, label) = match dwindex {
                0 => (CPFT_TILE_IMAGE, "框架图标"),  // 字段0: 图标
                1 => (CPFT_LARGE_TEXT, "WinLogon基础框架加载成功！"),  // 字段1: 文本
                _ => {
                    error!("SampleProvider::GetFieldDescriptorAt - 无效的字段索引: {}", dwindex);
                    return Err(windows::Win32::Foundation::E_INVALIDARG.into());
                }
            };
    
            // 转换标签为UTF-16并分配内存
            let label_u16: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
            let label_ptr = windows::Win32::System::Com::CoTaskMemAlloc(label_u16.len() * 2) as *mut u16;
            if label_ptr.is_null() {
                error!("SampleProvider::GetFieldDescriptorAt - 标签内存分配失败");
                windows::Win32::System::Com::CoTaskMemFree(Some(ptr as *mut _)); // 释放之前分配的内存
                return Err(windows::Win32::Foundation::E_OUTOFMEMORY.into());
            }
            std::ptr::copy_nonoverlapping(label_u16.as_ptr(), label_ptr, label_u16.len());
    
            // 设置字段描述符的属性
            (*ptr).dwFieldID = dwindex;
            (*ptr).cpft = ft;
            (*ptr).pszLabel = PWSTR(label_ptr);
    
            Ok(ptr)
        }
    }

    /// 获取凭据的数量和默认凭据
    /// pdwcount: 输出参数，凭据数量
    /// pdwdefault: 输出参数，默认选中的凭据索引
    /// pbautologonwithdefault: 输出参数，是否使用默认凭据自动登录
    fn GetCredentialCount(
        &self, 
        pdwcount: *mut u32, 
        pdwdefault: *mut u32, 
        pbautologonwithdefault: *mut BOOL
    ) -> windows_core::Result<()> {
        info!("SampleProvider::GetCredentialCount - 获取凭据数量");
        let inner = self.inner.lock().unwrap();
        let mut show_tile = true;
        if let Ok(result) = read_facewinunlock_registry("SHOW_TILE") {
            if result.as_str() == "0" {
                show_tile = false;
            }
        } else {
            warn!("注册表配置读取失败!");
        }

        info!( "是否显示图标: {}", show_tile);

        unsafe {
            // 如果管道已经收到了数据，告诉系统我们要自动登录
            if let Some(l) = &inner.listener {
                let listener = l.lock().unwrap();
                if listener.is_unlocked.load(Ordering::SeqCst) {
                    listener.is_unlocked.store(false, Ordering::SeqCst);
                    *pdwcount = 1;
                    *pdwdefault = 0;
                    *pbautologonwithdefault = BOOL::from(true); // 触发自动登录
                } else {
                    *pdwcount = if show_tile { 1 } else { 0 };
                }
            }
        }
        info!("SampleProvider::GetCredentialCount - 凭据数量: 1，默认索引: 0");
        Ok(())
    }

    /// 获取指定索引的凭据
    /// dwindex: 凭据索引
    fn GetCredentialAt(&self, dwindex: u32) -> windows_core::Result<ICredentialProviderCredential> {
        info!("SampleProvider::GetCredentialAt - 获取凭据，索引: {}", dwindex);
        if dwindex == 0 {
            let mut inner = self.inner.lock().unwrap();
            if let Some(old_cred) = inner.credential.take() {
                info!("SampleProvider::GetCredentialAt - 销毁已存在的旧凭据实例");
                drop(old_cred);
                dll_release();
            }
            // 创建凭据实例并转换为接口返回，并传递收到的用户名和密码
            info!("SampleProvider::GetCredentialAt - 创建新的凭据实例");
            let cred = SampleCredential::new(inner.shared_creds.clone(), inner.auth_package_id);
            let cred_interface: ICredentialProviderCredential = cred.into();
            inner.credential = Some(cred_interface.clone());
            Ok(cred_interface)
        } else {
            error!("SampleProvider::GetCredentialAt - 无效的凭据索引: {}", dwindex);
            Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_INVALIDARG))
        }
    }
}

// 获取Negotiate AuthPackage ID
pub fn retrieve_negotiate_auth_package() -> windows_core::Result<u32> {
    info!("正在获取 AuthPackage ID...");
    let mut lsa_handle = HANDLE::default();
    
    // 建立与 LSA 的非信任连接
    let status = unsafe { LsaConnectUntrusted(&mut lsa_handle) };
    if status != STATUS_SUCCESS {
        error!("LsaConnectUntrusted 失败: {:?}", status);
        return Err(status.into());
    }

    // 准备包名称字符串 "Negotiate"
    let package_name_str = "Negotiate";
    let name_bytes = package_name_str.as_bytes();

    let package_name = LSA_STRING {
        Buffer: PSTR(name_bytes.as_ptr() as *mut u8),
        Length: name_bytes.len() as u16,
        MaximumLength: (name_bytes.len() + 1) as u16,
    };

    // 查找 ID
    let mut package_id = 0;
    let status = unsafe { LsaLookupAuthenticationPackage(lsa_handle, &package_name, &mut package_id) };
    
    // 关闭连接
    let _ = unsafe { LsaDeregisterLogonProcess(lsa_handle) };

    if status == STATUS_SUCCESS {
        info!("成功获取 AuthPackage ID: {}", package_id);
        Ok(package_id)
    } else {
        error!("获取 AuthPackage ID 失败: {:?}", status);
        Err(status.into())
    }
}