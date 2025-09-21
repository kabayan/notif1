//! デバイス状態リソース

use std::sync::Arc;
use crate::mcp::JsonRpcError;
use crate::AppState;
use actix_web::web;
use crate::BluetoothManager;
use serde_json::{json, Value};

/// デバイス状態を読み取る
pub async fn read(data: web::Data<Arc<AppState>>) -> Result<Value, JsonRpcError> {
    let bt_manager = &data.bt_manager;
    let devices = bt_manager.list_connected_devices().await;

    let device_states: Vec<Value> = devices
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
        "devices": device_states
    }))
}