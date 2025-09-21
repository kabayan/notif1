//! プログレスバー表示プロンプト

use std::sync::Arc;
use crate::mcp::{JsonRpcError, INVALID_PARAMS};
use crate::AppState;
use actix_web::web;
use serde_json::{json, Value};

/// プログレスバー表示プロンプトを取得
pub async fn get(
    arguments: Value,
    _data: web::Data<Arc<AppState>>,
) -> Result<Value, JsonRpcError> {
    let percent = arguments
        .get("percent")
        .and_then(|p| p.as_u64())
        .ok_or_else(|| JsonRpcError {
            code: INVALID_PARAMS,
            message: "Missing required parameter: percent".to_string(),
            data: None,
        })? as u8;

    let label = arguments
        .get("label")
        .and_then(|l| l.as_str())
        .unwrap_or("Progress");

    let messages = vec![
        json!({
            "role": "user",
            "content": {
                "type": "text",
                "text": format!("Display a progress bar at {}% with label: {}", percent, label)
            }
        })
    ];

    Ok(json!({
        "description": "Progress bar display prompt",
        "messages": messages
    }))
}