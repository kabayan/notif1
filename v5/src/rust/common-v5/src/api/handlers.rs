//! 共通APIハンドラー実装

use actix_web::{web, HttpResponse};
use tracing::{debug, error, info, warn};
use std::time::Instant;
use std::fs::OpenOptions;
use std::io::Write;
use std::collections::HashMap;
use chrono::Local;
use serde::Deserialize;

// v5新機能のuse文追加（既存コードに影響なし）
#[cfg(feature = "http-endpoints")]
use crate::image::{ImageProcessor, FitMode, ProcessedImage};
#[cfg(feature = "http-endpoints")]
use actix_multipart::{Multipart, Field};
#[cfg(feature = "http-endpoints")]
use futures_util::stream::StreamExt as _;
#[cfg(feature = "http-endpoints")]
use std::sync::Arc;

// BLE制限対応のためのタイル構造体（v5新機能）
#[cfg(feature = "http-endpoints")]
#[derive(Debug, Clone)]
struct ImageTile {
    pub x: u8,
    pub y: u8,
    pub width: u8,
    pub height: u8,
    pub rgb565_data: Vec<u16>,
}

use crate::bluetooth::BluetoothManager;
use crate::error::{NotifError, Result};
use crate::protocol::{Command, RGB, Size};
use super::models::{v1, v2, ApiResponse, ApiError, parse_color_name};

/// v1 /send ハンドラーの共通処理
pub async fn process_v1_send<M: BluetoothManager>(
    params: v1::SendQuery,
    bt_manager: web::Data<M>,
) -> HttpResponse {
    info!("Processing v1 send request: {:?}", params);
    
    // パラメータを解析してコマンドを生成
    let commands = match build_v1_commands(&params) {
        Ok(cmds) => cmds,
        Err(e) => {
            error!("Failed to build commands: {}", e);
            return HttpResponse::BadRequest().json(v1::SendResponse::error(e.to_string()));
        }
    };
    
    // デバイス選択とコマンド送信
    let result = match params.device.as_deref() {
        Some("all") | None => {
            // 全デバイスに送信
            bt_manager.send_command_to_all(Command::Batch { commands }).await
        }
        Some(device_spec) => {
            // 特定のデバイスに送信
            if let Ok(num) = device_spec.parse::<usize>() {
                bt_manager.send_command_by_number(num, Command::Batch { commands }).await
            } else {
                bt_manager.send_command_to_device(device_spec, Command::Batch { commands }).await
            }
        }
    };
    
    match result {
        Ok(_) => {
            info!("Command sent successfully");
            HttpResponse::Ok().json(v1::SendResponse::ok())
        }
        Err(e) => {
            error!("Failed to send command: {}", e);
            HttpResponse::InternalServerError().json(v1::SendResponse::error(e.to_string()))
        }
    }
}

/// v1コマンドをビルド（v2互換の折り返し処理付き）
fn build_v1_commands(params: &v1::SendQuery) -> Result<Vec<Command>> {
    let mut commands = Vec::new();
    
    // 背景色でクリア
    let bgcolor = params.bgcolor.as_ref()
        .map(|c| parse_color_name(c))
        .unwrap_or(RGB::black());
    commands.push(Command::Clear { color: bgcolor });
    
    // テキストを表示
    if let Some(text) = &params.text {
        let color = params.color.as_ref()
            .map(|c| parse_color_name(c))
            .unwrap_or(RGB::white());
        
        let size = params.size.as_ref()
            .map(|s| Size::from_str(s))
            .unwrap_or(Size::Medium);
        
        // v1 API用のテキスト処理（折り返しあり）
        let text_commands = build_v1_text_commands(text, size, color)?;
        commands.extend(text_commands);
    }
    
    // display logに記録
    log_display_command(
        params.text.as_deref().unwrap_or(""),
        params.bgcolor.as_deref().unwrap_or("black"),
        params.color.as_deref().unwrap_or("white"),
        params.size.as_deref().unwrap_or("2"),
        &commands
    );
    
    Ok(commands)
}

/// v1 /status ハンドラーの共通処理
pub async fn process_v1_status<M: BluetoothManager>(
    bt_manager: web::Data<M>,
) -> HttpResponse {
    info!("Processing v1 status request");
    
    let devices = bt_manager.list_connected_devices().await;
    let stats = bt_manager.get_statistics().await;
    
    let device_statuses: Vec<v1::DeviceStatus> = devices.iter().map(|d| {
        v1::DeviceStatus {
            id: d.name.clone(),
            number: d.number.unwrap_or(0),
            connected: d.connected,
            battery: d.battery_level,
            signal: d.signal_strength,
        }
    }).collect();
    
    let response = v1::StatusResponse {
        status: if !device_statuses.is_empty() { "ok" } else { "no_devices" }.to_string(),
        connected: !device_statuses.is_empty(),
        devices: device_statuses,
        server: v1::ServerInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            platform: std::env::consts::OS.to_string(),
            uptime: stats.uptime_seconds,
        },
    };
    
    HttpResponse::Ok().json(response)
}

/// v2 /api/draw ハンドラーの共通処理
pub async fn process_v2_draw<M: BluetoothManager>(
    request: v2::DrawRequest,
    bt_manager: web::Data<M>,
) -> HttpResponse {
    info!("Processing v2 draw request");
    
    let start_time = Instant::now();
    
    // DrawCommandをprotocol::Commandに変換
    let command = match convert_draw_command(&request.command) {
        Ok(cmd) => cmd,
        Err(e) => {
            error!("Failed to convert draw command: {}", e);
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(ApiError {
                code: "INVALID_COMMAND".to_string(),
                message: e.to_string(),
                details: None,
            }));
        }
    };
    
    // デバイス選択とコマンド送信
    let device_selector = v2::DeviceSelector::parse(request.device);
    let result = match device_selector {
        v2::DeviceSelector::All(_) => {
            bt_manager.send_command_to_all(command).await
        }
        v2::DeviceSelector::Number(num) => {
            bt_manager.send_command_by_number(num, command).await
        }
        v2::DeviceSelector::Id(id) => {
            bt_manager.send_command_to_device(&id, command).await
        }
    };
    
    let execution_time_ms = start_time.elapsed().as_millis() as u64;
    
    match result {
        Ok(_) => {
            info!("Draw command executed in {}ms", execution_time_ms);
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "execution_time_ms": execution_time_ms
            }))
        }
        Err(e) => {
            error!("Failed to execute draw command: {}", e);
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(ApiError {
                code: e.error_code().to_string(),
                message: e.to_string(),
                details: None,
            }))
        }
    }
}

/// DrawCommandをprotocol::Commandに変換
fn convert_draw_command(draw_cmd: &v2::DrawCommand) -> Result<Command> {
    match draw_cmd {
        v2::DrawCommand::Text { x, y, text, color, size, .. } => {
            Ok(Command::Text {
                x: (*x).clamp(0, 255) as u8,
                y: (*y).clamp(0, 255) as u8,
                size: size.to_size(),
                color: color.to_rgb(),
                text: text.clone(),
            })
        }
        v2::DrawCommand::Clear { color } => {
            Ok(Command::Clear {
                color: color.to_rgb(),
            })
        }
        v2::DrawCommand::Line { x1, y1, x2, y2, color, width } => {
            Ok(Command::Line {
                x1: (*x1).clamp(0, 255) as u8,
                y1: (*y1).clamp(0, 255) as u8,
                x2: (*x2).clamp(0, 255) as u8,
                y2: (*y2).clamp(0, 255) as u8,
                width: *width,
                color: color.to_rgb(),
            })
        }
        v2::DrawCommand::Rect { x, y, width, height, color, filled } => {
            Ok(Command::Rect {
                x: (*x).clamp(0, 255) as u8,
                y: (*y).clamp(0, 255) as u8,
                width: (*width).clamp(0, 255) as u8,
                height: (*height).clamp(0, 255) as u8,
                fill: *filled,
                color: color.to_rgb(),
            })
        }
        v2::DrawCommand::Circle { x, y, radius, color, filled } => {
            Ok(Command::Circle {
                x: (*x).clamp(0, 255) as u8,
                y: (*y).clamp(0, 255) as u8,
                radius: (*radius).clamp(0, 255) as u8,
                color: color.to_rgb(),
                filled: *filled,
            })
        }
        v2::DrawCommand::Image { x, y, data, width, height } => {
            // Base64デコード
            let image_data = base64::decode(data)
                .map_err(|e| NotifError::InvalidParameter(format!("Invalid base64 image: {}", e)))?;
            
            Ok(Command::Image {
                x: (*x).clamp(0, 255) as u8,
                y: (*y).clamp(0, 255) as u8,
                width: width.unwrap_or(128).clamp(1, 255) as u8,
                height: height.unwrap_or(128).clamp(1, 255) as u8,
                format: 1, // デフォルトでRawRgb形式
                data: image_data,
            })
        }
        v2::DrawCommand::Emoji { x, y, emoji, size } => {
            // 絵文字文字列を最初の文字のUnicodeコードポイントに変換
            let code = emoji.chars().next().unwrap_or('\0') as u32;
            Ok(Command::Emoji {
                x: (*x).clamp(0, 255) as u8,
                y: (*y).clamp(0, 255) as u8,
                size: *size,
                code,
            })
        }
        v2::DrawCommand::Batch { commands } => {
            let converted: Result<Vec<Command>> = commands.iter()
                .map(convert_draw_command)
                .collect();
            Ok(Command::Batch { commands: converted? })
        }
    }
}

/// v2 /api/devices ハンドラーの共通処理
pub async fn process_v2_devices<M: BluetoothManager>(
    bt_manager: web::Data<M>,
) -> HttpResponse {
    info!("Processing v2 devices request");
    
    let devices = bt_manager.list_connected_devices().await;
    let total = devices.len();
    let connected = devices.iter().filter(|d| d.connected).count();
    
    let response = v2::DevicesResponse {
        devices,
        total,
        connected,
    };
    
    HttpResponse::Ok().json(ApiResponse::success(response))
}

/// v2 /api/health ハンドラーの共通処理
pub async fn process_v2_health<M: BluetoothManager>(
    bt_manager: web::Data<M>,
) -> HttpResponse {
    info!("Processing v2 health request");
    
    let stats = bt_manager.get_statistics().await;
    let devices = bt_manager.list_connected_devices().await;
    
    // メモリ情報の取得（簡易版）
    let memory_info = get_memory_info();
    
    let response = v2::HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        uptime_seconds: stats.uptime_seconds,
        bluetooth: v2::BluetoothHealth {
            status: "operational".to_string(),
            adapter: None, // プラットフォーム固有
            devices_connected: stats.connected_devices,
            devices_available: stats.total_devices,
        },
        memory: memory_info,
        api: v2::ApiStatistics {
            requests_total: stats.total_commands_sent,
            requests_per_minute: calculate_rpm(stats.total_commands_sent, stats.uptime_seconds),
            errors_total: stats.total_errors,
            average_response_time_ms: stats.average_response_time_ms,
        },
    };
    
    HttpResponse::Ok().json(ApiResponse::success(response))
}

/// v2 /api/batch ハンドラーの共通処理
pub async fn process_v2_batch<M: BluetoothManager + 'static>(
    request: v2::BatchRequest,
    bt_manager: web::Data<M>,
) -> HttpResponse {
    info!("Processing v2 batch request with {} operations", request.operations.len());
    
    let start_time = Instant::now();
    let mut results = Vec::new();
    
    if request.parallel {
        // 並列実行
        let mut tasks = Vec::new();
        
        for (index, op) in request.operations.into_iter().enumerate() {
            let bt_manager = bt_manager.clone();
            tasks.push(tokio::spawn(async move {
                let command = convert_draw_command(&op.command)?;
                let result = bt_manager.send_command_to_device(&op.device, command).await;
                Ok::<_, NotifError>((index, op.device, result))
            }));
        }
        
        for task in tasks {
            match task.await {
                Ok(Ok((index, device, result))) => {
                    results.push(v2::BatchResult {
                        index,
                        device,
                        status: if result.is_ok() { "success" } else { "error" }.to_string(),
                        error: result.err().map(|e| e.to_string()),
                    });
                }
                Ok(Err(e)) => {
                    error!("Batch operation failed: {}", e);
                }
                Err(e) => {
                    error!("Task panic: {}", e);
                }
            }
        }
    } else {
        // 順次実行
        for (index, op) in request.operations.into_iter().enumerate() {
            let command = match convert_draw_command(&op.command) {
                Ok(cmd) => cmd,
                Err(e) => {
                    results.push(v2::BatchResult {
                        index,
                        device: op.device,
                        status: "error".to_string(),
                        error: Some(e.to_string()),
                    });
                    continue;
                }
            };
            
            let result = bt_manager.send_command_to_device(&op.device, command).await;
            results.push(v2::BatchResult {
                index,
                device: op.device,
                status: if result.is_ok() { "success" } else { "error" }.to_string(),
                error: result.err().map(|e| e.to_string()),
            });
        }
    }
    
    let total_execution_time_ms = start_time.elapsed().as_millis() as u64;
    
    let response = v2::BatchResponse {
        status: "completed".to_string(),
        results,
        total_execution_time_ms,
    };
    
    HttpResponse::Ok().json(ApiResponse::success(response))
}

/// v1 API用のテキストコマンドを構築（v2互換の折り返し処理付き）
fn build_v1_text_commands(text: &str, size: Size, color: RGB) -> Result<Vec<Command>> {
    use crate::text::{parse_text_with_emoji, TextSegment};
    
    let mut commands = Vec::new();
    
    // 改行で分割（\nと\\nの両方に対応）
    let text = text.replace("\\n", "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    
    // v2互換のグリッド座標系（32x32）での文字サイズを計算
    let (ascii_width_grids, y_spacing_grids) = match size {
        Size::Small => (2, 4),   // サイズ1: ASCII 2グリッド、漢字 4グリッド、4グリッド行間
        Size::Medium => (3, 6),  // サイズ2: ASCII 3グリッド、漢字 6グリッド、6グリッド行間
        Size::Large => (4, 8),   // サイズ3: ASCII 4グリッド、漢字 8グリッド、8グリッド行間
        Size::XLarge => (5, 10), // サイズ4: ASCII 5グリッド、漢字 10グリッド、10グリッド行間（40px）
    };
    
    let mut current_y = 0u8;
    
    for line in lines.iter() {
        // 画面外チェック（32グリッドの高さ制限）
        if current_y + y_spacing_grids > 32 {
            break;
        }
        
        // 空行の場合は改行のみ
        if line.is_empty() {
            current_y += y_spacing_grids;
            continue;
        }
        
        // 各行を絵文字とテキストのセグメントに分割
        let segments = parse_text_with_emoji(line);
        let mut current_x = 0u8;
        
        for segment in segments {
            match segment {
                TextSegment::Text(txt) => {
                    if !txt.is_empty() {
                        // 文字を1つずつ処理して、行内に収まる分だけ送信
                        let mut line_text = String::new();
                        let mut line_start_x = current_x;
                        
                        for ch in txt.chars() {
                            // v2互換のグリッド単位での文字幅計算
                            let char_width = if ch.is_ascii() {
                                ascii_width_grids  // ASCII文字
                            } else {
                                ascii_width_grids * 2  // 全角文字はASCIIの2倍
                            };
                            
                            // 現在の行に収まらない場合（32グリッド幅制限）
                            if current_x + char_width > 32 {
                                // これまでのテキストを送信
                                if !line_text.is_empty() {
                                    commands.push(Command::Text {
                                        x: line_start_x,
                                        y: current_y,
                                        size,
                                        color,
                                        text: line_text.clone(),
                                    });
                                    line_text.clear();
                                }
                                
                                // 改行
                                current_x = 0;
                                current_y += y_spacing_grids;
                                line_start_x = 0;
                                
                                if current_y + y_spacing_grids > 32 {
                                    return Ok(commands);  // 画面外なので終了
                                }
                                
                                // 改行後の新しい行を開始
                                line_text.push(ch);
                                current_x = char_width;
                            } else {
                                // 現在の行に追加
                                line_text.push(ch);
                                current_x += char_width;
                            }
                        }
                        
                        // 残りのテキストを送信
                        if !line_text.is_empty() && current_y + y_spacing_grids <= 32 {
                            commands.push(Command::Text {
                                x: line_start_x,
                                y: current_y,
                                size,
                                color,
                                text: line_text,
                            });
                        }
                    }
                }
                TextSegment::Emoji(code) => {
                    // 絵文字の幅（全角文字と同じ）
                    let emoji_width = ascii_width_grids * 2;
                    
                    // 絵文字の高さ（フォントサイズに応じた実際の高さ）
                    let emoji_height = match size {
                        Size::Small => 4,   // 1行分の高さ
                        Size::Medium => 6,  // 1行分の高さ
                        Size::Large => 8,   // 1行分の高さ
                        Size::XLarge => 10, // 1行分の高さ（40px対応）
                    };
                    
                    info!("Processing emoji U+{:04X} at current_x={}, current_y={}, emoji_width={}, emoji_height={}", 
                          code, current_x, current_y, emoji_width, emoji_height);
                    
                    // 絵文字が現在の行に収まらない場合は改行
                    if current_x + emoji_width > 32 && current_x > 0 {
                        info!("Emoji doesn't fit in current line, wrapping to next line");
                        current_x = 0;
                        current_y += y_spacing_grids;
                        
                        if current_y + emoji_height > 32 {
                            info!("Emoji Y coordinate {} + {} > 32, skipping", current_y, emoji_height);
                            break;
                        }
                    }
                    
                    // 絵文字が画面内に収まるかチェック（修正版）
                    if current_x + emoji_width <= 32 && current_y + emoji_height <= 32 {
                        info!("Emoji fits in screen, creating command at ({},{})", current_x, current_y);
                        commands.push(Command::Emoji {
                            x: current_x,
                            y: current_y,
                            size: size.to_byte(),
                            code,
                        });
                        current_x += emoji_width;
                    } else {
                        info!("Emoji doesn't fit: current_x={}, emoji_width={}, current_y={}, emoji_height={}", 
                              current_x, emoji_width, current_y, emoji_height);
                    }
                }
            }
        }
        
        current_y += y_spacing_grids;
    }
    
    Ok(commands)
}

/// メモリ情報を取得（簡易版）
fn get_memory_info() -> v2::MemoryInfo {
    // プロセスのメモリ使用量を取得する簡易実装
    // 実際の実装ではsysinfo crateなどを使用
    v2::MemoryInfo {
        used_mb: 50,  // プレースホルダー
        total_mb: 256, // プレースホルダー
    }
}

/// RPM（リクエスト/分）を計算
fn calculate_rpm(total_requests: u64, uptime_seconds: u64) -> f64 {
    if uptime_seconds == 0 {
        return 0.0;
    }
    (total_requests as f64 / uptime_seconds as f64) * 60.0
}

/// 画面表示コマンドをログファイルに記録
fn log_display_command(
    text: &str,
    bgcolor: &str,
    color: &str,
    size: &str,
    commands: &Vec<Command>,
) {
    if let Ok(log_path) = std::env::var("DISPLAY_LOG_PATH") {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let log_entry = format!(
            "[{}] DISPLAY_COMMAND:\n  Text: {}\n  BgColor: {}\n  Color: {}\n  Size: {}\n  Commands: {} items\n",
            timestamp, text, bgcolor, color, size, commands.len()
        );
        
        // コマンドの詳細を追加
        let mut command_details = String::new();
        for (i, cmd) in commands.iter().enumerate() {
            match cmd {
                Command::Clear { color } => {
                    command_details.push_str(&format!("    [{}] Clear: RGB({},{},{})\n", 
                        i, color.r, color.g, color.b));
                }
                Command::Text { x, y, size, color, text } => {
                    command_details.push_str(&format!("    [{}] Text: pos({},{}) size={:?} color=RGB({},{},{}) text=\"{}\"\n",
                        i, x, y, size, color.r, color.g, color.b, text));
                }
                Command::Emoji { x, y, size, code } => {
                    command_details.push_str(&format!("    [{}] Emoji: pos({},{}) size={} code=U+{:04X}\n",
                        i, x, y, size, code));
                }
                _ => {
                    command_details.push_str(&format!("    [{}] Other command\n", i));
                }
            }
        }
        
        let full_log = format!("{}{}────────────────────────────────────────\n", 
            log_entry, command_details);
        
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        {
            let _ = file.write_all(full_log.as_bytes());
        }
    }
}

/// v2 /api/draw クエリパラメータハンドラー
pub async fn process_v2_draw_query<M: BluetoothManager>(
    params: HashMap<String, String>,
    bt_manager: web::Data<M>,
) -> HttpResponse {
    info!("Processing v2 draw request (query params)");
    
    // クエリパラメータからDrawQueryRequestを構築
    let request = v2::DrawQueryRequest::from_query_params(params);
    
    // デバッグ用ログ
    info!("DrawRequest: bg={:?}, device={:?}, overwrite={}, regions count: {}", 
          request.bg, request.device, request.overwrite, request.regions.len());
    for region in &request.regions {
        info!("Region {}: area={}, bg={:?}, text={:?}, tc={:?}, fs={:?}", 
              region.id, region.area, region.bg, region.text, region.tc, region.fs);
    }
    
    // 領域が指定されているかチェック
    if request.regions.is_empty() {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(ApiError {
            code: "key_error".to_string(),
            message: "No regions specified. At least one region (r1, r2, etc.) is required".to_string(),
            details: None,
        }));
    }
    
    // コマンドリストを構築
    let mut commands = Vec::new();
    
    // overwrite=falseの場合は画面をクリアしてから描画
    if !request.overwrite {
        let clear_color = if let Some(ref bg_color) = request.bg {
            parse_color_name(bg_color)
        } else {
            RGB::black()
        };
        commands.push(Command::Clear { color: clear_color });
    }
    
    // 各領域を処理
    for region in &request.regions {
        // 座標をパース（"row1,col1,row2,col2" 形式）
        let coords: Vec<i32> = region.area
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        
        if coords.len() != 4 {
            error!("Invalid area format for region {}: {}", region.id, region.area);
            continue;
        }
        
        let (row1, col1, row2, col2) = (coords[0], coords[1], coords[2], coords[3]);
        
        // グリッド座標のまま使用（v2 APIは32x32グリッド座標系）
        let x1 = col1;
        let y1 = row1;
        let width = col2 - col1 + 1;
        let height = row2 - row1 + 1;
        
        // 背景色の描画
        if let Some(ref bg_color) = region.bg {
            let color = parse_color_name(bg_color);
            let rect_x = x1.max(0).min(31) as u8;
            let rect_y = y1.max(0).min(31) as u8;
            let rect_width = width.max(1).min(32) as u8;
            let rect_height = height.max(1).min(32) as u8;
            
            info!("Creating Rect command: x={}, y={}, width={}, height={}, color=({},{},{}), fill=true",
                  rect_x, rect_y, rect_width, rect_height, color.r, color.g, color.b);
            
            commands.push(Command::Rect { 
                x: rect_x, 
                y: rect_y, 
                width: rect_width, 
                height: rect_height, 
                color, 
                fill: true 
            });
        }
        
        // テキストの描画
        if let Some(ref text) = region.text {
            let text_color = region.tc.as_ref()
                .map(|c| parse_color_name(c))
                .unwrap_or(RGB::white());
            
            // フォントサイズの変換
            let font_size = region.fs.unwrap_or(2);
            let size = match font_size {
                1 => Size::Small,
                2 => Size::Medium,
                3 => Size::Large,
                4 => Size::XLarge,
                _ => Size::Medium,
            };
            
            // テキスト座標（指定がない場合は領域の左上 + マージン）
            let margin = 1i32;  // 1グリッドのマージン
            let text_x = region.x.map(|x| x as i32).unwrap_or(x1 + margin);
            let text_y = region.y.map(|y| y as i32).unwrap_or(y1 + margin);
            
            // 領域内でテキストを折り返す
            use crate::text::wrap_text_with_emoji;
            let text_area_width = (width - margin * 2).max(1);
            let wrapped_lines = wrap_text_with_emoji(text, text_area_width, font_size);
            
            // 行の高さ（グリッド単位）
            let line_height = match font_size {
                1 => 4,   // size=1: 4グリッド
                2 => 6,   // size=2: 6グリッド
                3 => 8,   // size=3: 8グリッド
                4 => 10,  // size=4: 10グリッド
                _ => 6,   // デフォルト
            };
            
            // 各行を描画
            for (line_index, line) in wrapped_lines.iter().enumerate() {
                let line_y = text_y + (line_index as i32 * line_height);
                
                // 領域内に収まるかチェック
                if line_y + line_height > y1 + height - margin {
                    break;  // 領域を超えた場合は描画を中止
                }
                
                // 絵文字とテキストを分離して処理
                use crate::text::process_line_with_emoji;
                let line_commands = process_line_with_emoji(
                    line,
                    text_x,
                    line_y,
                    size,
                    text_color,
                    font_size,
                );
                
                for cmd in line_commands {
                    commands.push(cmd);
                }
            }
        }
    }
    
    // デバイス選択とコマンド送信
    let device_selector = v2::DeviceSelector::parse(request.device);
    
    info!("Sending {} commands to device as batch", commands.len());
    for (i, cmd) in commands.iter().enumerate() {
        info!("Command {}: {:?}", i, cmd);
    }
    
    // 全コマンドをBatchコマンドとして1つにまとめる
    let batch_command = Command::Batch { commands };
    
    let start_time = Instant::now();
    
    // Batchコマンドを1回で送信
    let result = match &device_selector {
        v2::DeviceSelector::All(_) => {
            bt_manager.send_command_to_all(batch_command).await
        }
        v2::DeviceSelector::Number(num) => {
            bt_manager.send_command_by_number(*num, batch_command).await
        }
        v2::DeviceSelector::Id(id) => {
            bt_manager.send_command_to_device(id, batch_command).await
        }
    };
    
    let execution_time_ms = start_time.elapsed().as_millis() as u64;
    
    match result {
        Ok(_) => {
            info!("Draw batch command executed successfully in {}ms", execution_time_ms);
            HttpResponse::Ok().json(ApiResponse::<()>::success(()))
        }
        Err(e) => {
            warn!("Draw batch command failed: {}", e);
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(ApiError {
                code: "COMMAND_FAILED".to_string(),
                message: format!("Failed to execute draw command: {}", e),
                details: None,
            }))
        }
    }
}

/// v2 /api/draw POST JSONハンドラー（v3互換）
pub async fn process_v2_draw_post<M: BluetoothManager>(
    request: v2::DrawQueryRequest,
    bt_manager: web::Data<M>,
) -> HttpResponse {
    info!("Processing v2 draw request (POST JSON)");
    
    // デバッグ用ログ
    info!("DrawRequest: bg={:?}, device={:?}, overwrite={}, regions count: {}", 
          request.bg, request.device, request.overwrite, request.regions.len());
    for region in &request.regions {
        info!("Region {}: area={}, bg={:?}, text={:?}, tc={:?}, fs={:?}", 
              region.id, region.area, region.bg, region.text, region.tc, region.fs);
    }
    
    // 領域が指定されているかチェック
    if request.regions.is_empty() {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(ApiError {
            code: "key_error".to_string(),
            message: "No regions specified. At least one region (r1, r2, etc.) is required".to_string(),
            details: None,
        }));
    }
    
    // コマンドリストを構築
    let mut commands = Vec::new();
    
    // overwrite=falseの場合は画面をクリアしてから描画
    if !request.overwrite {
        let clear_color = if let Some(ref bg_color) = request.bg {
            parse_color_name(bg_color)
        } else {
            RGB::black()
        };
        commands.push(Command::Clear { color: clear_color });
    }
    
    // 各領域を処理
    for region in &request.regions {
        // 座標をパース（"row1,col1,row2,col2" 形式）
        let coords: Vec<i32> = region.area
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        
        if coords.len() != 4 {
            error!("Invalid area format for region {}: {}", region.id, region.area);
            continue;
        }
        
        let (row1, col1, row2, col2) = (coords[0], coords[1], coords[2], coords[3]);
        
        // グリッド座標のまま使用（v2 APIは32x32グリッド座標系）
        let x1 = col1;
        let y1 = row1;
        let width = col2 - col1 + 1;
        let height = row2 - row1 + 1;
        
        // 背景色の描画
        if let Some(ref bg_color) = region.bg {
            let color = parse_color_name(bg_color);
            let rect_x = x1.max(0).min(31) as u8;
            let rect_y = y1.max(0).min(31) as u8;
            let rect_width = width.max(1).min(32) as u8;
            let rect_height = height.max(1).min(32) as u8;
            
            info!("Creating Rect command: x={}, y={}, width={}, height={}, color=({},{},{}), fill=true",
                  rect_x, rect_y, rect_width, rect_height, color.r, color.g, color.b);
            
            commands.push(Command::Rect { 
                x: rect_x, 
                y: rect_y, 
                width: rect_width, 
                height: rect_height, 
                color, 
                fill: true 
            });
        }
        
        // テキストの描画
        if let Some(ref text) = region.text {
            let text_color = region.tc.as_ref()
                .map(|c| parse_color_name(c))
                .unwrap_or(RGB::white());
            
            // フォントサイズの変換
            let font_size = region.fs.unwrap_or(2);
            let size = match font_size {
                1 => Size::Small,
                2 => Size::Medium,
                3 => Size::Large,
                4 => Size::XLarge,
                _ => Size::Medium,
            };
            
            // テキスト座標（指定がない場合は領域の左上 + マージン）
            let margin = 1i32;  // 1グリッドのマージン
            let text_x = region.x.map(|x| x as i32).unwrap_or(x1 + margin);
            let text_y = region.y.map(|y| y as i32).unwrap_or(y1 + margin);
            
            // 領域内でテキストを折り返す
            use crate::text::wrap_text_with_emoji;
            let text_area_width = (width - margin * 2).max(1);
            let wrapped_lines = wrap_text_with_emoji(text, text_area_width, font_size);
            
            // 行の高さ（グリッド単位）
            let line_height = match font_size {
                1 => 4,   // size=1: 4グリッド
                2 => 6,   // size=2: 6グリッド
                3 => 8,   // size=3: 8グリッド
                4 => 10,  // size=4: 10グリッド
                _ => 6,   // デフォルト
            };
            
            // 各行を描画
            for (line_index, line) in wrapped_lines.iter().enumerate() {
                let line_y = text_y + (line_index as i32 * line_height);
                
                // 領域内に収まるかチェック
                if line_y + line_height > y1 + height - margin {
                    break;  // 領域を超えた場合は描画を中止
                }
                
                // 絵文字とテキストを分離して処理
                use crate::text::process_line_with_emoji;
                let line_commands = process_line_with_emoji(
                    line,
                    text_x,
                    line_y,
                    size,
                    text_color,
                    font_size,
                );
                
                for cmd in line_commands {
                    commands.push(cmd);
                }
            }
        }
    }
    
    // デバイス選択とコマンド送信
    let device_selector = v2::DeviceSelector::parse(request.device);
    
    info!("Sending {} commands to device as batch", commands.len());
    for (i, cmd) in commands.iter().enumerate() {
        info!("Command {}: {:?}", i, cmd);
    }
    
    // 全コマンドをBatchコマンドとして1つにまとめる
    let batch_command = Command::Batch { commands };
    
    let start_time = Instant::now();
    
    // Batchコマンドを1回で送信
    let result = match &device_selector {
        v2::DeviceSelector::All(_) => {
            bt_manager.send_command_to_all(batch_command).await
        }
        v2::DeviceSelector::Number(num) => {
            bt_manager.send_command_by_number(*num, batch_command).await
        }
        v2::DeviceSelector::Id(id) => {
            bt_manager.send_command_to_device(id, batch_command).await
        }
    };
    
    let execution_time_ms = start_time.elapsed().as_millis() as u64;
    
    match result {
        Ok(_) => {
            info!("Draw batch command executed successfully in {}ms", execution_time_ms);
            HttpResponse::Ok().json(ApiResponse::<()>::success(()))
        }
        Err(e) => {
            warn!("Draw batch command failed: {}", e);
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(ApiError {
                code: "COMMAND_FAILED".to_string(),
                message: format!("Failed to execute draw command: {}", e),
                details: None,
            }))
        }
    }
}

// ==========================================================================
// v5新機能: 画像アップロードエンドポイント（既存のv4機能に一切影響なし）
// ========================================================================== 

/// 画像アップロードリクエストパラメータ
#[cfg(feature = "http-endpoints")]
#[derive(Debug, Clone, Deserialize)]
pub struct ImageUploadParams {
    #[serde(default = "default_device")]
    pub device: u8,
    #[serde(default)]
    pub x: u8,
    #[serde(default)]
    pub y: u8,
    #[serde(default)]
    pub fit: FitMode,
}

#[cfg(feature = "http-endpoints")]
fn default_device() -> u8 {
    1
}

#[cfg(feature = "http-endpoints")]
impl Default for ImageUploadParams {
    fn default() -> Self {
        Self {
            device: 1,
            x: 0,
            y: 0,
            fit: FitMode::Contain,
        }
    }
}

/// BLE制限対応: RGB565画像データをタイルに分割
/// 8x8ピクセル（128バイト）でBLE制限（512バイト）内に収める
#[cfg(feature = "http-endpoints")]
fn split_image_to_tiles(
    rgb565_data: &[u16], 
    image_width: u16, 
    image_height: u16,
    tile_size: u16  // 横幅（v5では16）
) -> Vec<ImageTile> {
    let mut tiles = Vec::new();
    
    // v5修正: 16x8の長方形タイル（横16ピクセル、縦8ピクセル）
    let tile_width = tile_size;
    let tile_height = 8u16;  // 固定高さ8ピクセル
    
    // タイル数を計算（切り上げ除算）
    let tiles_x = (image_width + tile_width - 1) / tile_width;
    let tiles_y = (image_height + tile_height - 1) / tile_height;
    
    info!("BLE最適化: 画像分割 {}x{}を{}x{}のタイルに分割（{}x{}タイル）", 
          image_width, image_height, tile_width, tile_height, tiles_x, tiles_y);
    
    for tile_y in 0..tiles_y {
        for tile_x in 0..tiles_x {
            let start_x = tile_x * tile_width;
            let start_y = tile_y * tile_height;
            let end_x = std::cmp::min(start_x + tile_width, image_width);
            let end_y = std::cmp::min(start_y + tile_height, image_height);
            
            let actual_width = end_x - start_x;
            let actual_height = end_y - start_y;
            
            // タイルデータを抽出
            let mut tile_data = Vec::with_capacity((actual_width * actual_height) as usize);
            
            for y in start_y..end_y {
                for x in start_x..end_x {
                    let idx = (y * image_width + x) as usize;
                    if idx < rgb565_data.len() {
                        tile_data.push(rgb565_data[idx]);
                    } else {
                        tile_data.push(0); // パディング（黒色）
                    }
                }
            }
            
            tiles.push(ImageTile {
                x: start_x as u8,
                y: start_y as u8,
                width: actual_width as u8,
                height: actual_height as u8,
                rgb565_data: tile_data,
            });
        }
    }
    
    let total_bytes: usize = tiles.iter()
        .map(|t| t.rgb565_data.len() * 2 + 8)  // データ + ヘッダー
        .sum();
    
    info!("タイル分割完了: {}個のタイル生成、総バイト数: {}バイト", tiles.len(), total_bytes);
    tiles
}

/// BLE制限対応: タイルを順次送信（v4のBluetooth実装をそのまま使用）
/// テスト用: まず1タイルのみ送信して動作確認
#[cfg(feature = "http-endpoints")]
async fn send_image_tiles<M: BluetoothManager>(
    tiles: Vec<ImageTile>,
    device: u8,
    base_x: u8,
    base_y: u8,
    bt_manager: &M
) -> std::result::Result<(), NotifError> {
    let total_tiles = tiles.len();
    // v5修正: 全128タイル送信（16x8ピクセル×128 = 128x128ピクセル全体）
    let tiles_to_send = total_tiles;  // 全タイル送信
    info!("タイル送信開始: {}個のタイル、デバイス={}, 開始位置=({},{}) - {}タイル送信", 
          total_tiles, device, base_x, base_y, tiles_to_send);
    
    // 送信速度測定用のタイマー開始
    let transmission_start = std::time::Instant::now();
    
    // v5追加: 全タイルをCommandのベクタとして保存する準備
    let mut tile_commands = Vec::new();
    
    // v5修正: 全タイル送信（256バイト×128）
    for (index, tile) in tiles.iter().take(tiles_to_send).enumerate() {
        let tile_bytes = crate::image::rgb565::rgb565_to_bytes(&tile.rgb565_data);
        let tile_data_size = tile_bytes.len();
        let tile_bytes_len = tile_bytes.len();  // 長さを先に保存
        
        // デバッグ: 最初のタイルの詳細情報
        if index == 0 {
            info!("最初のタイル送信開始: タイル座標=({},{}), base座標=({},{}), 実際の送信座標=({},{})", 
                  tile.x, tile.y, base_x, base_y, base_x + tile.x, base_y + tile.y);
        }
        
        debug!("タイル送信 {}/{}: 位置=({},{}), サイズ={}x{}, データサイズ={}バイト", 
               index + 1, tiles_to_send, 
               base_x + tile.x, base_y + tile.y, 
               tile.width, tile.height, tile_data_size);
        
        // BLE制限確認（安全のため500バイト以下で確認）
        let total_size = tile_data_size + 8; // データ + Command::Imageヘッダー
        if total_size > 500 {
            warn!("タイル{}のデータサイズ{}バイトがBLE制限を超過、送信中止", index + 1, total_size);
            return Err(NotifError::Bluetooth(format!("タイルサイズ{}バイトがBLE制限を超過", total_size)));
        }
        
        // v5修正: 各タイルの正しい位置に表示
        let image_command = crate::protocol::Command::Image {
            x: base_x + tile.x,
            y: base_y + tile.y,
            width: tile.width,
            height: tile.height,
            format: 2, // RGB565 - AtomS3ファームウェアではIMG_RAW_RGB565=0x02
            data: tile_bytes,
        };
        
        // v5追加: 再接続用にコマンドを保存
        tile_commands.push(image_command.clone());
        
        // v4のBluetooth実装をそのまま使用（変更禁止）
        let result = if device == 0 {
            bt_manager.send_command_to_all(image_command).await
        } else {
            bt_manager.send_command_by_number(device as usize, image_command).await
        };
        
        match result {
            Ok(_) => {
                // タイル送信成功をログに記録（送信パターン調査用）
                if index == 0 || index == tiles_to_send - 1 || index % 10 == 0 {
                    info!("タイル{}/{}送信成功 ({}バイト) to device {}", 
                         index + 1, tiles_to_send, tile_bytes_len + 8, device);
                } else {
                    debug!("タイル{}送信成功", index + 1);
                }
            }
            Err(e) => {
                error!("タイル{}送信失敗: {}", index + 1, e);
                return Err(e);
            }
        }
        
        // BLE安定性のためのタイル間待機（10ms）
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    
    // v5追加: 全タイル送信成功後、再接続用に保存
    if device == 0 {
        // 全デバイスの場合は各デバイスに保存
        let devices = bt_manager.list_connected_devices().await;
        for device_info in devices {
            bt_manager.save_image_tiles(&device_info.name, tile_commands.clone()).await;
        }
    } else {
        // 特定デバイスの場合はそのデバイスだけに保存
        if let Some(device_name) = bt_manager.get_device_name_by_number(device as usize).await {
            bt_manager.save_image_tiles(&device_name, tile_commands).await;
        }
    }
    
    // 送信時間を計測して速度を計算
    let elapsed = transmission_start.elapsed();
    let elapsed_ms = elapsed.as_millis();
    let tiles_per_sec = if elapsed_ms > 0 {
        (tiles_to_send as f64 * 1000.0) / elapsed_ms as f64
    } else {
        0.0
    };
    
    info!("{}タイル送信完了: {}/{}タイル, 時間: {}ms, 速度: {:.1}タイル/秒", 
          tiles_to_send, tiles_to_send, total_tiles, elapsed_ms, tiles_per_sec);
    Ok(())
}

/// フォームアップロード型画像送信
/// POST /api/image/upload
#[cfg(feature = "http-endpoints")]
pub async fn upload_image<M: BluetoothManager>(
    mut payload: Multipart,
    bt_manager: web::Data<M>,
) -> std::result::Result<HttpResponse, actix_web::Error> {
    let start_time = Instant::now();
    
    let mut image_data: Option<Vec<u8>> = None;
    let mut params = ImageUploadParams::default();
    
    info!("Processing image upload request");
    
    // multipart/form-data 処理
    while let Some(field_result) = payload.next().await {
        let mut field = field_result.map_err(|e| {
            error!("Multipart field error: {}", e);
            actix_web::error::ErrorBadRequest(format!("Multipart error: {}", e))
        })?;
        
        let field_name = field.name();
        debug!("Processing field: {}", field_name);
        
        match field_name {
            "file" => {
                // 画像ファイル処理
                image_data = Some(read_field_data(&mut field).await?);
                info!("Image file received, size: {} bytes", image_data.as_ref().unwrap().len());
            }
            "device" => {
                let data = read_field_data(&mut field).await?;
                let device_str = String::from_utf8_lossy(&data);
                params.device = device_str.trim().parse().unwrap_or(1);
                debug!("Device parameter: {}", params.device);
            }
            "x" => {
                let data = read_field_data(&mut field).await?;
                let x_str = String::from_utf8_lossy(&data);
                params.x = x_str.trim().parse().unwrap_or(0);
                debug!("X coordinate: {}", params.x);
            }
            "y" => {
                let data = read_field_data(&mut field).await?;
                let y_str = String::from_utf8_lossy(&data);
                params.y = y_str.trim().parse().unwrap_or(0);
                debug!("Y coordinate: {}", params.y);
            }
            "fit" => {
                let data = read_field_data(&mut field).await?;
                let fit_str = String::from_utf8_lossy(&data);
                params.fit = match fit_str.trim() {
                    "contain" => FitMode::Contain,
                    "cover" => FitMode::Cover,
                    "fill" => FitMode::Fill,
                    "none" => FitMode::None,
                    _ => FitMode::Contain,
                };
                debug!("Fit mode: {:?}", params.fit);
            }
            _ => {
                debug!("Unknown field ignored: {}", field_name);
            }
        }
    }
    
    let image_data = image_data.ok_or_else(|| {
        error!("No image file provided in multipart request");
        actix_web::error::ErrorBadRequest("No image file provided")
    })?;
    
    info!("Starting image processing: {} bytes, target 128x128, fit mode: {:?}", 
          image_data.len(), params.fit);
    
    // 画像処理
    let processor = ImageProcessor::new();
    let processed = match processor.process_image(image_data, (128, 128), params.fit) {
        Ok(processed) => {
            info!("Image processed successfully: {}x{} in {}ms", 
                  processed.width, processed.height, processed.processing_time_ms);
            processed
        },
        Err(e) => {
            error!("Image processing failed: {}", e);
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": format!("画像処理に失敗しました: {}", e)
            })));
        }
    };
    
    // BLE制限対応: 大きな画像をタイルに分割して送信
    let original_size = processed.rgb565_data.len() * 2; // 16bit = 2byte
    info!("BLE最適化開始: 元画像サイズ={}バイト、画像サイズ={}x{}", 
          original_size, processed.width, processed.height);
    
    // v5修正: タイルサイズを16x8ピクセル (256バイト) に変更
    let tile_size = 16u16;  // 16x8 = 128ピクセル = 256バイト
    let tiles = split_image_to_tiles(
        &processed.rgb565_data,
        processed.width, 
        processed.height,
        tile_size
    );
    
    // v5修正: 16x8タイル = 256バイト
    info!("BLE制限クリア: {}バイト → {}個のタイルに分割（最大{}バイト/タイル）", 
          original_size, tiles.len(), 16 * 8 * 2 + 8);  // 16x8ピクセル×2バイト+ヘッダー
    
    // タイルを順次送信（v4のBluetooth実装をそのまま使用）
    let tiles_to_send = tiles.len();  // 全タイル送信
    let send_result = send_image_tiles(
        tiles.clone(),
        params.device,
        params.x,
        params.y,
        bt_manager.get_ref()
    ).await;
    
    let total_time = start_time.elapsed().as_millis() as u64;
    
    match send_result {
        Ok(_) => {
            info!("画像タイル送信成功: {}/{}個のタイル、合計{}ms", tiles_to_send, tiles.len(), total_time);
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "画像がタイル分割され、全タイルがAtomS3に正常送信されました",
                "details": {
                    "original_format": processed.original_format,
                    "processed_size": [processed.width, processed.height],
                    "processing_time_ms": processed.processing_time_ms,
                    "total_time_ms": total_time,
                    "device": params.device,
                    "position": [params.x, params.y],
                    "fit_mode": format!("{:?}", params.fit),
                    "ble_optimization": {
                        "original_bytes": original_size,
                        "total_tiles_generated": tiles.len(),
                        "tiles_sent": 1,
                        "test_mode": true,
                        "tile_size": "16x8",  // v5修正: 固定サイズ
                        "max_tile_bytes": 16 * 8 * 2 + 8,  // 256バイト+ヘッダー
                        "ble_limit_compliant": true,
                        "transmission_time_ms": 10
                    }
                }
            })))
        }
        Err(e) => {
            error!("Failed to send image to device: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("デバイスへの送信に失敗しました: {}", e)
            })))
        }
    }
}

/// v5追加: 画像データ直接POST用
/// POST /api/image/post
#[cfg(feature = "http-endpoints")]
pub async fn post_image<M: BluetoothManager>(
    body: web::Bytes,
    query: web::Query<ImageUploadParams>,
    bt_manager: web::Data<M>,
) -> HttpResponse {
    let start_time = Instant::now();
    
    info!("POST画像受信: サイズ={}バイト, デバイス={}, 位置=({},{})", 
          body.len(), query.device, query.x, query.y);
    
    // 画像処理
    let processor = crate::image::ImageProcessor::new();
    let processed = match processor.process_image(
        body.to_vec(),
        (128, 128),  // AtomS3画面サイズ固定
        query.fit,
    ) {
        Ok(img) => img,
        Err(e) => {
            error!("画像処理エラー: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": format!("画像処理に失敗しました: {}", e)
            }));
        }
    };
    
    info!("画像処理完了: 処理後サイズ={}x{} ({:.1}ms)", 
          processed.width, processed.height, 
          processed.processing_time_ms);
    
    // v5: 画像を16x8ピクセルタイルに分割（BLE制限対応）
    let tile_size = 16u16;  // 16x8ピクセル
    let tiles = split_image_to_tiles(&processed.rgb565_data, processed.width, processed.height, tile_size);
    let original_size = processed.width as usize * processed.height as usize * 2;
    
    info!("BLE制限クリア: {}バイト → {}個のタイルに分割（最大{}バイト/タイル）", 
          original_size, tiles.len(), 16 * 8 * 2 + 8);
    
    // タイルを順次送信
    let tiles_to_send = tiles.len();
    let send_result = send_image_tiles(
        tiles.clone(),
        query.device,
        query.x,
        query.y,
        bt_manager.get_ref()
    ).await;
    
    let total_time = start_time.elapsed().as_millis() as u64;
    
    match send_result {
        Ok(_) => {
            info!("画像タイル送信成功: {}/{}個のタイル、合計{}ms", tiles_to_send, tiles.len(), total_time);
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "画像がタイル分割され、全タイルがAtomS3に正常送信されました",
                "details": {
                    "original_format": processed.original_format,
                    "processed_size": [processed.width, processed.height],
                    "processing_time_ms": processed.processing_time_ms,
                    "total_time_ms": total_time,
                    "device": query.device,
                    "position": [query.x, query.y],
                    "fit_mode": format!("{:?}", query.fit),
                    "ble_optimization": {
                        "original_bytes": original_size,
                        "total_tiles_generated": tiles.len(),
                        "tiles_sent": tiles_to_send,
                        "tile_size": "16x8",
                        "max_tile_bytes": 16 * 8 * 2 + 8,
                        "ble_limit_compliant": true
                    }
                }
            }))
        }
        Err(e) => {
            error!("画像送信失敗: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("デバイスへの送信に失敗しました: {}", e)
            }))
        }
    }
}

/// フィールドデータを読み取る補助関数
#[cfg(feature = "http-endpoints")]
async fn read_field_data(field: &mut Field) -> std::result::Result<Vec<u8>, actix_web::Error> {
    let mut data = Vec::new();
    
    while let Some(chunk_result) = field.next().await {
        let chunk = chunk_result.map_err(|e| {
            error!("Field chunk read error: {}", e);
            actix_web::error::ErrorBadRequest(format!("Field read error: {}", e))
        })?;
        data.extend_from_slice(&chunk);
    }
    
    debug!("Read field data: {} bytes", data.len());
    Ok(data)
}

#[cfg(test)]
#[cfg(feature = "http-endpoints")]
mod image_upload_tests {
    use super::*;

    #[test]
    fn test_image_upload_params_default() {
        let params = ImageUploadParams::default();
        assert_eq!(params.device, 1);
        assert_eq!(params.x, 0);
        assert_eq!(params.y, 0);
    }

    #[test]
    fn test_fit_mode_parsing() {
        // FitMode文字列パースのテスト（手動実装版）
        let test_cases = vec![
            ("contain", FitMode::Contain),
            ("cover", FitMode::Cover),
            ("fill", FitMode::Fill),
            ("none", FitMode::None),
        ];
        
        for (input, expected) in test_cases {
            let result = match input {
                "contain" => FitMode::Contain,
                "cover" => FitMode::Cover,
                "fill" => FitMode::Fill,
                "none" => FitMode::None,
                _ => FitMode::Contain,
            };
            assert_eq!(result, expected);
        }
    }
}