//! 接続状態リソース

use std::sync::Arc;
use crate::mcp::JsonRpcError;
use crate::AppState;
use actix_web::web;
use crate::BluetoothManager;
use serde_json::{json, Value};

/// 接続状態を読み取る
pub async fn read(data: web::Data<Arc<AppState>>) -> Result<Value, JsonRpcError> {
    let bt_manager = &data.bt_manager;
    let devices = bt_manager.list_connected_devices().await;
    
    // デバイス統計を取得
    let stats = bt_manager.get_statistics().await;

    Ok(json!({
        "connected": !devices.is_empty(),
        "connection_count": devices.len(),
        "uptime_seconds": stats.uptime_seconds,
        "total_commands_sent": stats.total_commands_sent,
        "total_errors": stats.total_errors,
    }))
}