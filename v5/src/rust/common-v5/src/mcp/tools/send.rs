//! sendツール - テキストメッセージ送信（実績のあるv1 API実装をベース）

use std::sync::Arc;
use crate::mcp::{JsonRpcError, INTERNAL_ERROR, INVALID_PARAMS};
use crate::AppState;
use actix_web::web;
use crate::{BluetoothManager, protocol::{Command, RGB, Size}, api::models::parse_color_name};
use serde_json::{json, Value};
use tracing::{debug, error, info};

/// sendツールの実行（実績のあるv1 API実装をベース）
pub async fn execute(
    arguments: Value,
    data: web::Data<Arc<AppState>>,
) -> Result<Value, JsonRpcError> {
    // パラメータ解析
    let text = arguments
        .get("text")
        .and_then(|t| t.as_str())
        .ok_or_else(|| JsonRpcError {
            code: INVALID_PARAMS,
            message: "Missing required parameter: text".to_string(),
            data: None,
        })?;

    let bgcolor = arguments
        .get("bgcolor")
        .and_then(|b| b.as_str())
        .unwrap_or("black");

    let color = arguments
        .get("color")
        .and_then(|c| c.as_str())
        .unwrap_or("white");

    let size = arguments
        .get("size")
        .and_then(|s| s.as_u64())
        .unwrap_or(3) as u8;

    let device = arguments
        .get("device")
        .and_then(|d| d.as_u64())
        .map(|d| d as usize);

    info!(
        "MCP send tool called: text='{}', bg={}, color={}, size={}, device={:?}",
        text, bgcolor, color, size, device
    );

    // v1 API実装を活用したコマンド生成
    let commands = match build_mcp_send_commands(text, bgcolor, color, size) {
        Ok(cmds) => {
            info!("MCP send tool: Built {} commands", cmds.len());
            cmds
        },
        Err(e) => {
            error!("Failed to build commands: {}", e);
            return Err(JsonRpcError {
                code: INTERNAL_ERROR,
                message: "Failed to build commands".to_string(),
                data: Some(json!({ "error": e.to_string() })),
            });
        }
    };

    let batch_command = Command::Batch { commands };

    // 送信
    let bt_manager = &data.bt_manager;
    let result = if let Some(device_num) = device {
        // 特定デバイスに送信
        bt_manager
            .send_command_by_number(device_num, batch_command)
            .await
    } else {
        // 全デバイスに送信
        bt_manager.send_command_to_all(batch_command).await
    };

    match result {
        Ok(_) => {
            info!("MCP send command executed successfully");
            
            // 等価なcurlコマンドを生成
            let curl_command = if let Some(device_num) = device {
                format!(
                    "curl -G \"http://localhost:18080/send\" \\\n  --data-urlencode \"text={}\" \\\n  --data-urlencode \"bgcolor={}\" \\\n  --data-urlencode \"color={}\" \\\n  --data-urlencode \"size={}\" \\\n  --data-urlencode \"device={}\"",
                    text, bgcolor, color, size, device_num
                )
            } else {
                format!(
                    "curl -G \"http://localhost:18080/send\" \\\n  --data-urlencode \"text={}\" \\\n  --data-urlencode \"bgcolor={}\" \\\n  --data-urlencode \"color={}\" \\\n  --data-urlencode \"size={}\"",
                    text, bgcolor, color, size
                )
            };
            
            Ok(json!({
                "success": true,
                "message": format!("Message sent: {}", text),
                "curl_equivalent": curl_command,
                "api_info": {
                    "endpoint": "/send",
                    "method": "GET",
                    "params": {
                        "text": text,
                        "bgcolor": bgcolor,
                        "color": color,
                        "size": size,
                        "device": device
                    }
                }
            }))
        }
        Err(e) => {
            error!("Failed to send message: {}", e);
            Err(JsonRpcError {
                code: INTERNAL_ERROR,
                message: "Failed to send message".to_string(),
                data: Some(json!({ "error": e.to_string() })),
            })
        }
    }
}

/// MCP send用のコマンド生成（実績のあるv1 API実装をベース）
fn build_mcp_send_commands(
    text: &str,
    bgcolor: &str, 
    color: &str,
    size: u8
) -> crate::error::Result<Vec<Command>> {
    let mut commands = Vec::new();
    
    // 背景色でクリア
    let bg_rgb = parse_color_name(bgcolor);
    commands.push(Command::Clear { color: bg_rgb });
    
    // テキストを表示（v1 API実装を活用）
    let text_rgb = parse_color_name(color);
    let text_size = match size {
        1 => Size::Small,
        2 => Size::Medium,
        3 => Size::Large,
        4 => Size::XLarge,
        _ => Size::Medium,
    };
    
    // v1 API実装と同じテキスト処理を使用
    let text_commands = build_mcp_text_commands(text, text_size, text_rgb)?;
    commands.extend(text_commands);
    
    Ok(commands)
}

/// MCPテキスト処理（v1 API実装をベース）
fn build_mcp_text_commands(text: &str, size: Size, color: RGB) -> crate::error::Result<Vec<Command>> {
    use crate::text::{parse_text_with_emoji, TextSegment};
    
    info!("MCP send tool: Processing text with emoji support: '{}'", text);
    
    let mut commands = Vec::new();
    
    // 改行で分割（\nと\\nの両方に対応）
    let text = text.replace("\\n", "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    info!("MCP send tool: Split into {} lines", lines.len());
    
    // v2互換のグリッド座標系（32x32）での文字サイズを計算
    let (ascii_width_grids, y_spacing_grids) = match size {
        Size::Small => (2, 4),   // サイズ1: ASCII 2グリッド、漢字 4グリッド、4グリッド行間
        Size::Medium => (3, 6),  // サイズ2: ASCII 3グリッド、漢字 6グリッド、6グリッド行間
        Size::Large => (4, 8),   // サイズ3: ASCII 4グリッド、漢字 8グリッド、8グリッド行間
        Size::XLarge => (5, 10), // サイズ4: ASCII 5グリッド、漢字 10グリッド、10グリッド行間
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
        info!("MCP send tool: Line '{}' has {} segments", line, segments.len());
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
                            info!("MCP send tool: Adding text '{}' at ({},{})", line_text, line_start_x, current_y);
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
                        Size::XLarge => 10, // 1行分の高さ
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
                    
                    // 絵文字が画面内に収まるかチェック
                    if current_x + emoji_width <= 32 && current_y + emoji_height <= 32 {
                        info!("MCP send tool: Adding emoji U+{:04X} at ({},{})", code, current_x, current_y);
                        commands.push(Command::Emoji {
                            x: current_x,
                            y: current_y,
                            size: size.to_byte(),
                            code,
                        });
                        current_x += emoji_width;
                    } else {
                        info!("MCP send tool: Emoji U+{:04X} doesn't fit: current_x={}, emoji_width={}, current_y={}, emoji_height={}", 
                              code, current_x, emoji_width, current_y, emoji_height);
                    }
                }
            }
        }
        
        current_y += y_spacing_grids;
    }
    
    Ok(commands)
}