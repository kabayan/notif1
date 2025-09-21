//! プロトコル定義（共通）

use serde::{Deserialize, Serialize};
use crate::error::{NotifError, Result};

/// RGB色
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RGB {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        RGB { r, g, b }
    }
    
    pub fn black() -> Self {
        RGB::new(0, 0, 0)
    }
    
    pub fn white() -> Self {
        RGB::new(255, 255, 255)
    }
}

/// フォントサイズ
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Size {
    Small,
    Medium,
    Large,
    XLarge,
}

impl Size {
    pub fn to_byte(&self) -> u8 {
        match self {
            Size::Small => 1,
            Size::Medium => 2,
            Size::Large => 3,
            Size::XLarge => 4,
        }
    }
    
    pub fn from_str(s: &str) -> Self {
        match s {
            "1" | "small" => Size::Small,
            "2" | "medium" => Size::Medium,
            "3" | "large" => Size::Large,
            "4" | "xlarge" | "extralarge" => Size::XLarge,
            _ => Size::Medium,
        }
    }
    
    pub fn to_font_size(&self) -> u8 {
        match self {
            Size::Small => 8,
            Size::Medium => 12,
            Size::Large => 16,
            Size::XLarge => 24,
        }
    }
}

/// コマンドタイプ
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Command {
    /// テキスト表示 (v2/ATOMS3互換)
    Text {
        x: u8,
        y: u8,
        size: Size,
        color: RGB,
        text: String,
    },
    
    /// 画面クリア
    Clear {
        color: RGB,
    },
    
    /// 線描画 (v2/ATOMS3互換)
    Line {
        x1: u8,
        y1: u8,
        x2: u8,
        y2: u8,
        width: u8,
        color: RGB,
    },
    
    /// 矩形描画 (v2/ATOMS3互換)
    Rect {
        x: u8,
        y: u8,
        width: u8,
        height: u8,
        fill: bool,
        color: RGB,
    },
    
    /// 円描画 (v2/ATOMS3互換)
    Circle {
        x: u8,
        y: u8,
        radius: u8,
        color: RGB,
        filled: bool,
    },
    
    /// 画像表示 (v2/ATOMS3互換)
    Image {
        x: u8,
        y: u8,
        width: u8,
        height: u8,
        format: u8,
        data: Vec<u8>,
    },
    
    /// 絵文字表示 (v2/ATOMS3互換)
    Emoji {
        x: u8,
        y: u8,
        size: u8,
        code: u32,
    },
    
    /// 領域分割
    Region {
        regions: Vec<Region>,
    },
    
    /// バッチコマンド
    Batch {
        commands: Vec<Command>,
    },
    
    /// 画面更新
    Update,
}

/// 領域定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub content: Box<Command>,
}

impl Command {
    /// コマンドをバイト列にエンコード
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::new();
        
        match self {
            Command::Text { x, y, size, color, text } => {
                let text_bytes = text.as_bytes();
                let payload_len = 7 + text_bytes.len(); // x(1) + y(1) + size(1) + color(3) + text_len(1) + text
                
                data.push(command_type::TEXT); // 0x02 - ATOMS3互換
                data.push((payload_len & 0xFF) as u8); // ペイロード長(リトルエンディアン)
                data.push((payload_len >> 8) as u8);
                data.extend_from_slice(&[*x, *y, size.to_byte()]);
                data.extend_from_slice(&[color.r, color.g, color.b]);
                data.push(text_bytes.len() as u8);
                data.extend_from_slice(text_bytes);
            }
            
            Command::Clear { color } => {
                data.push(command_type::CLEAR); // 0x01 - ATOMS3互換
                data.extend_from_slice(&[3, 0]); // ペイロード長(リトルエンディアン)
                data.extend_from_slice(&[color.r, color.g, color.b]);
            }
            
            Command::Line { x1, y1, x2, y2, width, color } => {
                data.push(command_type::LINE); // 0x05 - ATOMS3互換
                data.extend_from_slice(&[8, 0]); // ペイロード長(リトルエンディアン)
                data.extend_from_slice(&[*x1, *y1, *x2, *y2, *width]);
                data.extend_from_slice(&[color.r, color.g, color.b]);
            }
            
            Command::Rect { x, y, width, height, fill, color } => {
                data.push(command_type::RECT); // 0x04 - ATOMS3互換
                data.extend_from_slice(&[8, 0]); // ペイロード長(リトルエンディアン)
                data.extend_from_slice(&[*x, *y, *width, *height]);
                data.push(if *fill { 1 } else { 0 });
                data.extend_from_slice(&[color.r, color.g, color.b]);
            }
            
            Command::Circle { x, y, radius, color, filled } => {
                // 注：CircleはATOMS3ファームウェアで未サポートの可能性があります
                data.push(0x05); // CMD_CIRCLE (カスタム)
                data.extend_from_slice(&[7, 0]); // ペイロード長(リトルエンディアン)
                data.extend_from_slice(&[*x, *y, *radius]);
                data.extend_from_slice(&[color.r, color.g, color.b]);
                data.push(if *filled { 1 } else { 0 });
            }
            
            Command::Image { x, y, width, height, format, data: image_data } => {
                let payload_len = 5 + image_data.len(); // x(1) + y(1) + w(1) + h(1) + format(1) + data
                
                data.push(command_type::IMAGE); // 0x06 - ATOMS3互換
                data.push((payload_len & 0xFF) as u8); // ペイロード長(リトルエンディアン)
                data.push((payload_len >> 8) as u8);
                data.extend_from_slice(&[*x, *y, *width, *height, *format]);
                data.extend_from_slice(image_data);
            }
            
            Command::Emoji { x, y, size, code } => {
                data.push(command_type::EMOJI); // 0x03 - ATOMS3互換
                data.extend_from_slice(&[7, 0]); // ペイロード長(リトルエンディアン)
                data.extend_from_slice(&[*x, *y, *size]);
                data.extend_from_slice(&code.to_le_bytes());
            }
            
            Command::Update => {
                data.push(0x08); // CMD_UPDATE
            }
            
            Command::Batch { commands } => {
                let mut payload = vec![commands.len() as u8];
                for cmd in commands {
                    payload.extend_from_slice(&cmd.encode());
                }
                
                data.push(command_type::BATCH); // 0x10 - ATOMS3互換
                let payload_len = payload.len();
                data.push((payload_len & 0xFF) as u8);
                data.push((payload_len >> 8) as u8);
                data.extend_from_slice(&payload);
            }
            
            Command::Region { regions } => {
                // Region処理は複雑なのでATOMS3で未サポートの可能性があります
                data.push(command_type::REGION); // 0x0A (カスタム)
                let mut payload = vec![regions.len() as u8];
                for region in regions {
                    payload.extend_from_slice(&[region.x as u8, region.y as u8]);
                    payload.extend_from_slice(&[region.width as u8, region.height as u8]);
                    let content_data = region.content.encode();
                    payload.extend_from_slice(&(content_data.len() as u16).to_le_bytes());
                    payload.extend_from_slice(&content_data);
                }
                data.push((payload.len() & 0xFF) as u8);
                data.push((payload.len() >> 8) as u8);
                data.extend_from_slice(&payload);
            }
        }
        
        data
    }
}

/// ステータスコード
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatusCode {
    Success = 0x00,
    Error = 0x01,
    InvalidCommand = 0x02,
    OutOfMemory = 0x03,
    Busy = 0x04,
}

impl StatusCode {
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x00 => StatusCode::Success,
            0x01 => StatusCode::Error,
            0x02 => StatusCode::InvalidCommand,
            0x03 => StatusCode::OutOfMemory,
            0x04 => StatusCode::Busy,
            _ => StatusCode::Error,
        }
    }
}

/// Bluetooth UUID定義（v2互換）
pub mod uuid {
    /// サービスUUID（v2互換）
    pub const SERVICE: &str = "12345678-1234-5678-1234-56789abcdef0";
    
    /// コマンド送信用キャラクタリスティック（v2互換）
    pub const COMMAND_CHAR: &str = "12345678-1234-5678-1234-56789abcdef1";
    
    /// ステータス通知用キャラクタリスティック（v2互換）
    pub const STATUS_CHAR: &str = "12345678-1234-5678-1234-56789abcdef2";
    
    /// 設定用キャラクタリスティック（v2互換）
    pub const CONFIG_CHAR: &str = "12345678-1234-5678-1234-56789abcdef3";
}

/// コマンドタイプ（バイト値） - ATOMS3ファームウェア互換
pub mod command_type {
    pub const CLEAR: u8 = 0x01;  // ATOMS3: CMD_CLEAR = 0x01
    pub const TEXT: u8 = 0x02;   // ATOMS3: CMD_TEXT = 0x02  
    pub const EMOJI: u8 = 0x03;  // ATOMS3: CMD_EMOJI = 0x03
    pub const RECT: u8 = 0x04;   // ATOMS3: CMD_RECT = 0x04
    pub const LINE: u8 = 0x05;   // ATOMS3: CMD_LINE = 0x05
    pub const IMAGE: u8 = 0x06;  // ATOMS3: CMD_IMAGE = 0x06
    pub const UPDATE: u8 = 0x08;
    pub const BATCH: u8 = 0x10;  // ATOMS3: CMD_BATCH = 0x10
    pub const REGION: u8 = 0x0A;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_command_encode() {
        // テスト用の画像データ（RGB565形式の4バイト）
        let img_data = vec![0x12, 0x34, 0x56, 0x78];
        let cmd = Command::Image {
            x: 10,
            y: 20,
            width: 128,
            height: 128,
            format: 0, // RGB565
            data: img_data.clone(),
        };
        
        let encoded = cmd.encode();
        
        // ヘッダー確認: コマンドタイプ
        assert_eq!(encoded[0], command_type::IMAGE); // 0x06
        
        // ペイロード長確認（5 + データ長）リトルエンディアン
        let payload_len = ((encoded[2] as u16) << 8) | (encoded[1] as u16);
        assert_eq!(payload_len, 5 + img_data.len() as u16);
        
        // パラメータ確認
        assert_eq!(encoded[3], 10);  // x
        assert_eq!(encoded[4], 20);  // y
        assert_eq!(encoded[5], 128); // width
        assert_eq!(encoded[6], 128); // height
        assert_eq!(encoded[7], 0);   // format
        
        // データ部分確認
        assert_eq!(&encoded[8..], &img_data);
    }

    #[test]
    fn test_image_command_empty_data() {
        // 空の画像データのテスト
        let cmd = Command::Image {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
            format: 1,
            data: vec![],
        };
        
        let encoded = cmd.encode();
        
        // ヘッダー部分のみ確認
        assert_eq!(encoded[0], command_type::IMAGE);
        
        // ペイロード長は5（パラメータのみ）
        let payload_len = ((encoded[2] as u16) << 8) | (encoded[1] as u16);
        assert_eq!(payload_len, 5);
        
        // 全体長確認（ヘッダー3バイト + パラメータ5バイト）
        assert_eq!(encoded.len(), 8);
    }

    #[test]
    fn test_image_command_large_data() {
        // 大きなデータのテスト（AtomS3の128x128画像相当）
        let img_data = vec![0xAB; 128 * 128 * 2]; // 128x128 RGB565 = 32KB
        let cmd = Command::Image {
            x: 0,
            y: 0,
            width: 128,
            height: 128,
            format: 0,
            data: img_data.clone(),
        };
        
        let encoded = cmd.encode();
        
        // ペイロード長確認
        let payload_len = ((encoded[2] as u16) << 8) | (encoded[1] as u16);
        assert_eq!(payload_len, 5 + img_data.len() as u16);
        
        // 全体長確認
        assert_eq!(encoded.len(), 3 + 5 + img_data.len());
        
        // データ部分が正しく配置されていることを確認
        assert_eq!(&encoded[8..], &img_data);
    }
}