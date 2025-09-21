//! ステータス表示プロンプト

use std::sync::Arc;
use crate::mcp::JsonRpcError;
use crate::AppState;
use actix_web::web;
use serde_json::{json, Value};

/// ステータス表示プロンプトを取得
pub async fn get(
    arguments: Value,
    _data: web::Data<Arc<AppState>>,
) -> Result<Value, JsonRpcError> {
    let format = arguments
        .get("format")
        .and_then(|f| f.as_str())
        .unwrap_or("simple");

    let messages = if format == "detailed" {
        vec![
            json!({
                "role": "user",
                "content": {
                    "type": "text",
                    "text": "Display detailed device status including battery, signal strength, and capabilities"
                }
            })
        ]
    } else {
        vec![
            json!({
                "role": "user",
                "content": {
                    "type": "text",
                    "text": "Display simple device status with connection count"
                }
            })
        ]
    };

    Ok(json!({
        "description": "Status display prompt",
        "messages": messages
    }))
}