//! drawツール - 領域描画（完全実装）

use std::sync::Arc;
use crate::mcp::{JsonRpcError, INTERNAL_ERROR, INVALID_PARAMS};
use crate::AppState;
use actix_web::web;
use crate::{BluetoothManager, protocol::{Command, RGB, Size}, api::models::parse_color_name};
use serde_json::{json, Value};
use tracing::{debug, error, info};

/// drawツールの実行
pub async fn execute(
    arguments: Value,
    data: web::Data<Arc<AppState>>,
) -> Result<Value, JsonRpcError> {
    // パラメータ解析
    let regions = arguments
        .get("regions")
        .and_then(|r| r.as_array())
        .ok_or_else(|| JsonRpcError {
            code: INVALID_PARAMS,
            message: "Missing required parameter: regions".to_string(),
            data: None,
        })?;

    let device = arguments
        .get("device")
        .and_then(|d| d.as_u64())
        .unwrap_or(1) as usize;

    let overwrite = arguments
        .get("overwrite")
        .and_then(|o| o.as_bool())
        .unwrap_or(false);

    info!(
        "MCP draw tool called: device={}, overwrite={}, regions_count={}",
        device,
        overwrite,
        regions.len()
    );

    let bt_manager = &data.bt_manager;

    // overwrite=false（デフォルト）の場合は画面クリア
    // overwrite=trueの場合は既存表示を保持
    if !overwrite {
        let clear_cmd = Command::Clear { 
            color: RGB::black() 
        };
        if let Err(e) = bt_manager
            .send_command_by_number(device, clear_cmd)
            .await
        {
            error!("Failed to clear screen: {}", e);
        }
    }

    // 全領域を処理する完全実装
    let mut commands = Vec::new();
    let mut regions_processed = 0;
    
    for region in regions {
        // テキストを取得
        let text = region
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        
        if text.is_empty() {
            continue;
        }
        
        // 座標パラメータを解析（coords: "row1,col1,row2,col2" v2 API互換）
        let coords = region
            .get("coords")
            .and_then(|c| c.as_str())
            .unwrap_or("0,0,31,31");
        
        let coord_parts: Vec<&str> = coords.split(',').collect();
        let row1 = coord_parts.get(0)
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let col1 = coord_parts.get(1)
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let row2 = coord_parts.get(2)
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(31);
        let col2 = coord_parts.get(3)
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(31);
        
        // v2 API互換: row,col座標をx,y座標に変換
        let x = col1.max(0).min(31) as u8;
        let y = row1.max(0).min(31) as u8;
        let width = (col2 - col1 + 1).max(1).min(32) as u8;
        let height = (row2 - row1 + 1).max(1).min(32) as u8;
        
        // 背景色パラメータ（bg）
        let bg_color = region
            .get("bg")
            .and_then(|b| b.as_str())
            .map(|color| parse_color_name(color));
        
        // テキスト色パラメータ（tc）
        let text_color = region
            .get("tc")
            .and_then(|t| t.as_str())
            .map(|color| parse_color_name(color))
            .unwrap_or_else(|| RGB::white());
        
        // フォントサイズパラメータ（fs）
        let font_size = region
            .get("fs")
            .and_then(|f| f.as_u64())
            .unwrap_or(2);
        
        let size = match font_size {
            1 => Size::Small,
            2 => Size::Medium,
            3 => Size::Large,
            4 => Size::XLarge,
            _ => Size::Medium,
        };
        
        info!(
            "MCP draw: Processing region #{}: text='{}', area=({},{},{},{}), bg={:?}, tc={:?}, fs={}",
            regions_processed + 1, text, row1, col1, row2, col2, bg_color, text_color, font_size
        );
        
        // 背景色が指定されている場合は指定領域に矩形を描画
        if let Some(bg) = bg_color {
            // v2 API互換: 指定領域のみに背景色を適用
            commands.push(Command::Rect { 
                x,
                y,
                width,
                height,
                color: bg,
                fill: true
            });
        }
        
        // テキストコマンドを追加
        commands.push(Command::Text {
            x,
            y,
            size,
            color: text_color,
            text: text.to_string(),
        });
        
        regions_processed += 1;
    }
    
    if commands.is_empty() {
        return Ok(json!({
            "success": true,
            "message": "No valid regions to draw"
        }));
    }
    
    info!("MCP draw tool: Sending {} commands for {} regions", commands.len(), regions_processed);
    
    // バッチコマンドとして送信
    let batch_command = Command::Batch { commands };
    
    match bt_manager.send_command_by_number(device, batch_command).await {
        Ok(_) => {
            info!("MCP draw command executed successfully");
            
            // 等価なcurlコマンドを生成（v2 API形式）
            let mut curl_params = Vec::new();
            curl_params.push(format!("--data-urlencode \"device={}\"", device));
            if overwrite {
                curl_params.push("--data-urlencode \"overwrite=true\"".to_string());
            }
            
            // 処理した領域のパラメータを追加
            for (idx, region) in regions.iter().enumerate() {
                let region_num = idx + 1;
                
                // 座標パラメータ
                if let Some(coords) = region.get("coords").and_then(|c| c.as_str()) {
                    curl_params.push(format!("--data-urlencode \"r{}={}\"", region_num, coords));
                }
                
                // 背景色
                if let Some(bg) = region.get("bg").and_then(|b| b.as_str()) {
                    curl_params.push(format!("--data-urlencode \"bg{}={}\"", region_num, bg));
                }
                
                // テキスト
                if let Some(text) = region.get("text").and_then(|t| t.as_str()) {
                    curl_params.push(format!("--data-urlencode \"t{}={}\"", region_num, text));
                }
                
                // テキスト色
                if let Some(tc) = region.get("tc").and_then(|t| t.as_str()) {
                    curl_params.push(format!("--data-urlencode \"tc{}={}\"", region_num, tc));
                }
                
                // フォントサイズ
                if let Some(fs) = region.get("fs").and_then(|f| f.as_u64()) {
                    curl_params.push(format!("--data-urlencode \"fs{}={}\"", region_num, fs));
                }
            }
            
            let curl_command = format!(
                "curl -G \"http://localhost:18080/api/draw\" \\\n  {}",
                curl_params.join(" \\\n  ")
            );
            
            Ok(json!({
                "success": true,
                "message": format!("Drew {} regions", regions_processed),
                "curl_equivalent": curl_command,
                "api_info": {
                    "endpoint": "/api/draw",
                    "method": "GET",
                    "device": device,
                    "overwrite": overwrite,
                    "regions_count": regions_processed
                }
            }))
        },
        Err(e) => {
            error!("Failed to draw regions: {}", e);
            Err(JsonRpcError {
                code: INTERNAL_ERROR,
                message: "Failed to draw regions".to_string(),
                data: Some(json!({ "error": e.to_string() })),
            })
        }
    }
}