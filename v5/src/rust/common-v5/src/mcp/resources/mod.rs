//! MCPリソース実装

use std::sync::Arc;
pub mod device_state;
pub mod connection_status;

use serde_json::{json, Value};

/// リソースリストを返す
pub async fn list() -> Result<Value, crate::mcp::JsonRpcError> {
    Ok(json!({
        "resources": [
            {
                "uri": "notif://device_state",
                "name": "Device State",
                "description": "Current state of all connected Bluetooth devices",
                "mimeType": "application/json"
            },
            {
                "uri": "notif://connection_status",
                "name": "Connection Status",
                "description": "Overall connection status and statistics",
                "mimeType": "application/json"
            }
        ]
    }))
}