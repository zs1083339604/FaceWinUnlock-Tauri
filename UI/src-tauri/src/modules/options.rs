use crate::utils::custom_result::CustomResult;
use winreg::enums::*;
use winreg::RegKey;

// 注册表结构体
#[derive(serde::Deserialize)]
pub struct RegistryItem {
    pub key: String,
    pub value: String,
}

// 向注册表写入数据
#[tauri::command]
pub fn write_to_registry(items: Vec<RegistryItem>) -> Result<CustomResult, CustomResult> {
    // 向所有用户写入
    let reg_path = format!("SOFTWARE\\{}", "facewinunlock-tauri");
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (app_key, _) = hklm
        .create_subkey(&reg_path)
        .map_err(|e| CustomResult::error(Some(format!("创建子项失败 {}", e)), None))?;

    // 遍历写入每个 key-value
    for item in items {
        // 写入字符串类型的值
        app_key.set_value(&item.key, &item.value).map_err(|e| {
            CustomResult::error(Some(format!("写入 {} 失败：{}", item.key, e)), None)
        })?;
    }

    Ok(CustomResult::success(None, None))
}
