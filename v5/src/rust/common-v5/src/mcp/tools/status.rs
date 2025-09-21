//! statusツール - デバイスステータス取得

use std::sync::Arc;
use crate::mcp::JsonRpcError;
use crate::AppState;
use actix_web::web;
use crate::BluetoothManager;
use serde_json::{json, Value};
use tracing::debug;

/// statusツールの実行
pub async fn execute(
    _arguments: Value,
    data: web::Data<Arc<AppState>>,
) -> Result<Value, JsonRpcError> {
    debug!("MCP status tool: getting device status");

    let bt_manager = &data.bt_manager;
    let devices = bt_manager.list_connected_devices().await;

    let device_list: Vec<Value> = devices
        .into_iter()
        .map(|info| {
            json!({
                "id": info.number.unwrap_or(0),
                "name": info.name,
                "address": info.address,
                "connected": info.connected,
                "signal_strength": info.signal_strength,
                "battery": info.battery_level,
            })
        })
        .collect();

    Ok(json!({
        "connected_count": device_list.len(),
        "devices": device_list,
        "curl_equivalent": "curl -s \"http://localhost:18080/status\"",
        "api_info": {
            "endpoint": "/status",
            "method": "GET",
            "description": "v1 APIのステータス取得エンドポイント"
        }
    }))
}