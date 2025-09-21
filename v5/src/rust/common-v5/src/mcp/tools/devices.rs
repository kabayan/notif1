//! devicesツール - デバイス管理

use std::sync::Arc;
use crate::mcp::{JsonRpcError, INTERNAL_ERROR, INVALID_PARAMS};
use crate::AppState;
use actix_web::web;
use crate::{BluetoothManager, Scanner};
use serde_json::{json, Value};
use tracing::{debug, error};

/// devices.listの実行
pub async fn list(
    _arguments: Value,
    data: web::Data<Arc<AppState>>,
) -> Result<Value, JsonRpcError> {
    debug!("MCP devices.list: listing available devices");

    let bt_manager = &data.bt_manager;
    let scanner = bt_manager.create_scanner()
        .map_err(|e| JsonRpcError {
            code: INTERNAL_ERROR,
            message: "Failed to create scanner".to_string(),
            data: Some(json!({ "error": e.to_string() })),
        })?;
    
    let devices = scanner.scan("MIX", std::time::Duration::from_secs(10)).await;

    match devices {
        Ok(device_list) => {
            let devices_json: Vec<Value> = device_list
                .into_iter()
                .map(|d| {
                    json!({
                        "name": d.name,
                        "address": d.address,
                        "connected": d.connected,
                        "signal_strength": d.signal_strength,
                        "battery_level": d.battery_level,
                    })
                })
                .collect();

            Ok(json!({
                "devices": devices_json,
                "count": devices_json.len(),
                "curl_equivalent": "curl -s \"http://localhost:18080/api/devices\"",
                "api_info": {
                    "endpoint": "/api/devices",
                    "method": "GET",
                    "description": "v2 APIのデバイス一覧取得エンドポイント"
                }
            }))
        }
        Err(e) => {
            error!("Failed to scan devices: {}", e);
            Err(JsonRpcError {
                code: INTERNAL_ERROR,
                message: "Failed to scan for devices".to_string(),
                data: Some(json!({ "error": e.to_string() })),
            })
        }
    }
}

/// devices.connectの実行
pub async fn connect(
    arguments: Value,
    data: web::Data<Arc<AppState>>,
) -> Result<Value, JsonRpcError> {
    let address = arguments
        .get("address")
        .and_then(|a| a.as_str())
        .ok_or_else(|| JsonRpcError {
            code: INVALID_PARAMS,
            message: "Missing required parameter: address".to_string(),
            data: None,
        })?;

    debug!("MCP devices.connect: connecting to {}", address);

    let bt_manager = &data.bt_manager;
    let scanner = bt_manager.create_scanner()
        .map_err(|e| JsonRpcError {
            code: INTERNAL_ERROR,
            message: "Failed to create scanner".to_string(),
            data: Some(json!({ "error": e.to_string() })),
        })?;
    
    // アドレスでデバイスを検索
    let device_opt = scanner.scan_for_device(address, std::time::Duration::from_secs(10)).await
        .map_err(|e| JsonRpcError {
            code: INTERNAL_ERROR,
            message: "Failed to scan for device".to_string(),
            data: Some(json!({ "error": e.to_string() })),
        })?;
    
    match device_opt {
        Some(device) => {
            match scanner.connect(&device).await {
                Ok(_connection) => Ok(json!({
                    "success": true,
                    "message": format!("Connected to device {}", address),
                    "curl_equivalent": format!("# Bluetooth接続はHTTP APIでは直接実行できません\n# デバイスのスキャンと管理には専用ツールが必要です"),
                    "api_info": {
                        "note": "Bluetooth接続管理はMCP専用機能です"
                    }
                })),
                Err(e) => {
                    error!("Failed to connect to device: {}", e);
                    Err(JsonRpcError {
                        code: INTERNAL_ERROR,
                        message: "Failed to connect to device".to_string(),
                        data: Some(json!({ "error": e.to_string() })),
                    })
                }
            }
        }
        None => Err(JsonRpcError {
            code: INVALID_PARAMS,
            message: format!("Device with address {} not found", address),
            data: None,
        })
    }
}

/// devices.disconnectの実行
pub async fn disconnect(
    arguments: Value,
    data: web::Data<Arc<AppState>>,
) -> Result<Value, JsonRpcError> {
    let device = arguments
        .get("device")
        .and_then(|d| d.as_u64())
        .ok_or_else(|| JsonRpcError {
            code: INVALID_PARAMS,
            message: "Missing required parameter: device".to_string(),
            data: None,
        })? as usize;

    debug!("MCP devices.disconnect: disconnecting device {}", device);

    let bt_manager = &data.bt_manager;
    let device_id = format!("device_{}", device);
    
    match bt_manager.disconnect_device(&device_id).await {
        Ok(_) => Ok(json!({
            "success": true,
            "message": format!("Disconnected device {}", device),
            "curl_equivalent": "# Bluetooth切断はHTTP APIでは直接実行できません\n# デバイスの切断管理には専用ツールが必要です",
            "api_info": {
                "note": "Bluetooth切断管理はMCP専用機能です"
            }
        })),
        Err(e) => {
            error!("Failed to disconnect device: {}", e);
            Err(JsonRpcError {
                code: INTERNAL_ERROR,
                message: "Failed to disconnect device".to_string(),
                data: Some(json!({ "error": e.to_string() })),
            })
        }
    }
}