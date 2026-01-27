use serde_json::{json, Value};

#[derive(Debug, serde::Serialize)]
pub struct CustomResult {
    pub code: i32,
    pub msg: String,
    pub data: Value,
}

impl CustomResult {
    pub fn new(code: i32, msg: String, data: Value) -> Self {
        Self { code, msg, data }
    }

    pub fn success(msg: Option<String>, data: Option<Value>) -> Self {
        Self::new(
            200,
            msg.unwrap_or("Success".to_string()),
            data.unwrap_or(json!(null)),
        )
    }

    pub fn error(msg: Option<String>, data: Option<Value>) -> Self {
        Self::new(
            500,
            msg.unwrap_or("error".to_string()),
            data.unwrap_or(json!(null)),
        )
    }

    pub fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}
