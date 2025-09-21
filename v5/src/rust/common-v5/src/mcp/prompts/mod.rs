//! MCPプロンプト実装

use std::sync::Arc;
pub mod status_display;
pub mod progress_bar;

use serde_json::{json, Value};

/// プロンプトリストを返す
pub async fn list() -> Result<Value, crate::mcp::JsonRpcError> {
    Ok(json!({
        "prompts": [
            {
                "name": "status_display",
                "description": "Display device status on Bluetooth display",
                "arguments": [
                    {
                        "name": "format",
                        "description": "Display format (simple/detailed)",
                        "required": false
                    }
                ]
            },
            {
                "name": "progress_bar",
                "description": "Display a progress bar on Bluetooth display",
                "arguments": [
                    {
                        "name": "percent",
                        "description": "Progress percentage (0-100)",
                        "required": true
                    },
                    {
                        "name": "label",
                        "description": "Progress label text",
                        "required": false
                    }
                ]
            }
        ]
    }))
}