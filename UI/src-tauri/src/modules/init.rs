use crate::modules::options::{write_to_registry, RegistryItem};
use crate::{
    utils::{
        api::{check_global_autostart, disable_global_autostart},
        custom_result::CustomResult,
    },
    ROOT_DIR,
};
use opencv::videoio::{self, VideoCaptureTraitConst};
use serde_json::json;
use std::fs;
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};
use winreg::enums::*;
use winreg::RegKey;

// 检查是否具有管理员权限
#[tauri::command]
pub fn check_admin_privileges() -> Result<CustomResult, CustomResult> {
    unsafe {
        let mut token: HANDLE = HANDLE::default();
        let success = OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token);
        if let Ok(_) = success {
            let mut elevation = TOKEN_ELEVATION::default();
            let mut size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;

            let result = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                size,
                &mut size,
            );

            let _ = CloseHandle(token);
            if result.is_ok() && elevation.TokenIsElevated != 0 {
                return Ok(CustomResult::success(None, None));
            }
        } else {
            return Err(CustomResult::error(
                Some(format!("打开与进程关联的访问令牌失败：{:?}", success.err())),
                None,
            ));
        }
    }
    return Err(CustomResult::error(
        Some(String::from("无管理员权限，请右键 ‘以管理员身份’ 运行软件")),
        None,
    ));
}

// 检查摄像头是否可用
#[tauri::command]
pub fn check_camera_status() -> Result<CustomResult, CustomResult> {
    let mut available_cameras = Vec::new();

    // 0 通常表示默认摄像头
    // 1 通常表示外置摄像头
    // 循环判断
    for i in 0..2 {
        if let Ok(cam) = videoio::VideoCapture::new(i, videoio::CAP_ANY) {
            if videoio::VideoCapture::is_opened(&cam).unwrap_or(false) {
                available_cameras.push(format!("摄像头索引 {}", i));
            }
        }
    }

    if available_cameras.is_empty() {
        Err(CustomResult::error(
            Some(String::from("未找到可用摄像头或权限被拒绝")),
            None,
        ))
    } else {
        Ok(CustomResult::success(None, None))
    }
}

// 复制 DLL 并写入注册表
#[tauri::command]
pub fn deploy_core_components() -> Result<CustomResult, CustomResult> {
    let dll_name = "FaceWinUnlock-Tauri.dll";
    let target_path = format!("C:\\Windows\\System32\\{}", dll_name);

    // 获取 resources 中的 DLL 路径
    let resource_path = ROOT_DIR.join("resources").join(dll_name);

    // 检查资源文件是否存在
    if !resource_path.exists() {
        return Err(CustomResult::error(
            Some(format!("资源文件不存在: {:?}", resource_path.to_str())),
            None,
        ));
    }

    // 复制文件到 System32
    fs::copy(&resource_path, &target_path).map_err(|e| {
        CustomResult::error(
            Some(format!(
                "DLL 复制失败: {} 请确认是否以管理员身份运行，或文件是否被占用",
                e
            )),
            None,
        )
    })?;

    // 写入注册表
    let clsid = "{8a7b9c6d-4e5f-89a0-8b7c-6d5e4f3e2d1c}";

    // 使用 KEY_ALL_ACCESS 确保拥有写入权限
    let hk_lm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hk_cr = RegKey::predef(HKEY_CLASSES_ROOT);

    // 注册 Credential Provider
    let cp_path = format!(
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Authentication\\Credential Providers\\{}",
        clsid
    );
    let (cp_key, _) = hk_lm
        .create_subkey(cp_path)
        .map_err(|e| CustomResult::error(Some(format!("无法创建注册表项(CP): {}", e)), None))?;
    cp_key
        .set_value("", &"FaceWinUnlock-Tauri")
        .map_err(|e| CustomResult::error(Some(format!("无法设置注册表项(CP): {}", e)), None))?;

    // 注册 CLSID
    let clsid_path = format!("CLSID\\{}", clsid);
    let (clsid_key, _) = hk_cr
        .create_subkey(&clsid_path)
        .map_err(|e| CustomResult::error(Some(format!("无法创建注册表项(CLSID): {}", e)), None))?;
    clsid_key
        .set_value("", &"FaceWinUnlock-Tauri")
        .map_err(|e| CustomResult::error(Some(format!("无法设置注册表项(CLSID): {}", e)), None))?;

    let (inproc_key, _) = hk_cr
        .create_subkey(format!("{}\\InprocServer32", clsid_path))
        .map_err(|e| {
            CustomResult::error(
                Some(format!("无法创建注册表项(InprocServer32): {}", e)),
                None,
            )
        })?;

    inproc_key.set_value("", &target_path).map_err(|e| {
        CustomResult::error(
            Some(format!("无法设置注册表项(InprocServer32): {}", e)),
            None,
        )
    })?;
    inproc_key
        .set_value("ThreadingModel", &"Apartment")
        .map_err(|e| {
            CustomResult::error(
                Some(format!("无法设置注册表项(ThreadingModel): {}", e)),
                None,
            )
        })?;

    // 创建dll日志路径
    let path = ROOT_DIR.join("logs");
    if !path.exists() {
        fs::create_dir_all(&path)
            .map_err(|e| CustomResult::error(Some(format!("创建 logs 文件夹失败: {}", e)), None))?;
    }

    let path_str = path.to_str();
    if path_str.is_none() {
        return Err(CustomResult::error(
            Some(String::from("获取软件日志目录失败")),
            None,
        ));
    }
    // 写入dll日志路径
    write_to_registry(vec![
        RegistryItem {
            key: String::from("DLL_LOG_PATH"),
            value: path_str.unwrap().to_string(),
        },
        RegistryItem {
            key: String::from("EXE_PATH"),
            value: ROOT_DIR.join("facewinunlock-tauri.exe").to_str().unwrap_or("").to_string(),
        }
    ])?;

    Ok(CustomResult::success(None, None))
}

// 卸载dll
#[tauri::command]
pub fn uninstall_init() -> Result<CustomResult, CustomResult> {
    // 删除注册表
    const MAIN_REG_PATH: &str = "SOFTWARE\\facewinunlock-tauri";
    const CLSID: &str = "{8a7b9c6d-4e5f-89a0-8b7c-6d5e4f3e2d1c}";

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);

    // 删除 Credential Providers 下的 CLSID 项（递归删除）
    let cp_path = format!(
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Authentication\\Credential Providers\\{}",
        CLSID
    );
    hklm.delete_subkey_all(&cp_path)
        .map_err(|e| CustomResult::error(Some(format!("删除注册表项(CP)失败: {}", e)), None))?;

    // 删除 CLSID 根项（递归删除）
    let clsid_path = format!("CLSID\\{}", CLSID);
    hkcr.delete_subkey_all(&clsid_path)
        .map_err(|e| CustomResult::error(Some(format!("删除注册表项(CLSID)失败: {}", e)), None))?;

    // 删除程序DLL设置的注册表
    hklm.delete_subkey_all(MAIN_REG_PATH)
        .map_err(|e| CustomResult::error(Some(format!("删除注册表项(DLL)失败: {}", e)), None))?;

    // 清除自启动
    let result = check_global_autostart()?;
    if *result.data.get("enable").unwrap() == json!(true) {
        disable_global_autostart()?;
    }

    // 删除dll文件
    let dll_name = "FaceWinUnlock-Tauri.dll";
    let target_path = format!("C:\\Windows\\System32\\{}", dll_name);
    fs::remove_file(target_path)
        .map_err(|e| CustomResult::error(Some(format!("删除DLL失败: {}", e)), None))?;

    Ok(CustomResult::success(None, None))
}
