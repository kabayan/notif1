//! 共通APIモデル定義

use serde::{Deserialize, Serialize};
use crate::protocol::{Command, RGB, Size};
use crate::bluetooth::DeviceInfo;

/// v1互換APIモデル
pub mod v1 {
    use super::*;
    
    /// /send エンドポイントのクエリパラメータ
    #[derive(Debug, Deserialize, Serialize)]
    pub struct SendQuery {
        pub text: Option<String>,
        pub bgcolor: Option<String>,
        pub color: Option<String>,
        pub size: Option<String>,
        pub font: Option<String>,
        pub device: Option<String>,
    }
    
    /// /send エンドポイントのレスポンス
    #[derive(Debug, Serialize, Deserialize)]
    pub struct SendResponse {
        pub status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<String>,
    }
    
    impl SendResponse {
        pub fn ok() -> Self {
            SendResponse {
                status: "ok".to_string(),
                error: None,
            }
        }
        
        pub fn error(msg: String) -> Self {
            SendResponse {
                status: "error".to_string(),
                error: Some(msg),
            }
        }
    }
    
    /// /status エンドポイントのレスポンス
    #[derive(Debug, Serialize, Deserialize)]
    pub struct StatusResponse {
        pub status: String,
        pub connected: bool,
        pub devices: Vec<DeviceStatus>,
        pub server: ServerInfo,
    }
    
    /// デバイスステータス
    #[derive(Debug, Serialize, Deserialize)]
    pub struct DeviceStatus {
        pub id: String,
        pub number: usize,
        pub connected: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub battery: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub signal: Option<i8>,
    }
    
    /// サーバー情報
    #[derive(Debug, Serialize, Deserialize)]
    pub struct ServerInfo {
        pub version: String,
        pub platform: String,
        pub uptime: u64,
    }
}

/// v2 APIモデル
pub mod v2 {
    use super::*;
    use std::collections::HashMap;
    
    /// bool値を柔軟に解析するヘルパー関数
    /// true: "true", "1", "yes", "on"（大文字小文字区別なし）
    /// false: "false", "0", "no", "off"（大文字小文字区別なし）
    fn parse_bool(value: &str) -> Option<bool> {
        match value.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        }
    }
    
    /// デバイスセレクター
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(untagged)]
    pub enum DeviceSelector {
        All(String),           // "all"
        Number(usize),         // 数値
        Id(String),           // デバイスID
    }
    
    impl DeviceSelector {
        pub fn parse(s: Option<String>) -> Self {
            match s.as_deref() {
                None | Some("all") => DeviceSelector::All("all".to_string()),
                Some(s) => {
                    if let Ok(num) = s.parse::<usize>() {
                        DeviceSelector::Number(num)
                    } else {
                        DeviceSelector::Id(s.to_string())
                    }
                }
            }
        }
    }
    
    /// v2クエリパラメータ形式の領域定義
    #[derive(Debug, Deserialize, Serialize)]
    pub struct QueryRegion {
        pub id: u32,
        /// "row1,col1,row2,col2" 形式
        pub area: String,
        /// 背景色（オプション）
        pub bg: Option<String>,
        /// テキスト
        pub text: Option<String>,
        /// テキスト色
        pub tc: Option<String>,
        /// テキストX座標
        pub x: Option<u8>,
        /// テキストY座標  
        pub y: Option<u8>,
        /// フォントサイズ
        pub fs: Option<u8>,
        /// フォント名
        pub fn_: Option<String>,
        /// Base64画像データ
        pub img: Option<String>,
    }
    
    /// v2 描画リクエスト（GET/POST共通）
    #[derive(Debug, Deserialize, Serialize)]
    pub struct DrawQueryRequest {
        /// 全体背景色
        pub bg: Option<String>,
        /// 既存表示に上書きするかどうか（デフォルト: false）
        #[serde(default)]
        pub overwrite: bool,
        /// 領域定義（パラメータから構築）
        #[serde(default)]
        pub regions: Vec<QueryRegion>,
        /// デバイス指定
        pub device: Option<String>,
    }
    
    impl DrawQueryRequest {
        /// クエリパラメータからDrawQueryRequestを構築
        pub fn from_query_params(mut params: HashMap<String, String>) -> Self {
            let mut request = DrawQueryRequest {
                bg: params.get("bg").cloned(),
                overwrite: params.get("overwrite")
                    .and_then(|v| parse_bool(v))
                    .unwrap_or(false),
                device: params.remove("device"),
                regions: Vec::new(),
            };
            
            // r1, r2, ... のパラメータを探す
            let mut region_ids = Vec::new();
            for key in params.keys() {
                if key.starts_with('r') && key.len() > 1 {
                    if let Ok(id) = key[1..].parse::<u32>() {
                        region_ids.push(id);
                    }
                }
            }
            region_ids.sort();
            
            // 各領域のパラメータを収集
            for id in region_ids {
                if let Some(area) = params.get(&format!("r{}", id)) {
                    let region = QueryRegion {
                        id,
                        area: area.clone(),
                        bg: params.get(&format!("bg{}", id)).cloned(),
                        text: params.get(&format!("t{}", id)).cloned(),
                        tc: params.get(&format!("tc{}", id)).cloned(),
                        x: params.get(&format!("x{}", id)).and_then(|s| s.parse().ok()),
                        y: params.get(&format!("y{}", id)).and_then(|s| s.parse().ok()),
                        fs: params.get(&format!("fs{}", id)).and_then(|s| s.parse().ok()),
                        fn_: params.get(&format!("fn{}", id)).cloned(),
                        img: params.get(&format!("img{}", id)).cloned(),
                    };
                    request.regions.push(region);
                }
            }
            
            request
        }
    }
    
    /// /api/draw リクエスト（JSON形式）
    #[derive(Debug, Deserialize, Serialize)]
    pub struct DrawRequest {
        pub device: Option<String>,
        pub command: DrawCommand,
    }
    
    /// 描画コマンド
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(tag = "type")]
    pub enum DrawCommand {
        #[serde(rename = "text")]
        Text {
            x: i32,
            y: i32,
            text: String,
            color: ColorValue,
            size: SizeValue,
            #[serde(default)]
            font: Option<String>,
        },
        
        #[serde(rename = "clear")]
        Clear {
            #[serde(default = "default_black_color")]
            color: ColorValue,
        },
        
        #[serde(rename = "line")]
        Line {
            x1: i32,
            y1: i32,
            x2: i32,
            y2: i32,
            color: ColorValue,
            #[serde(default = "default_line_width")]
            width: u8,
        },
        
        #[serde(rename = "rect")]
        Rect {
            x: i32,
            y: i32,
            width: u32,
            height: u32,
            color: ColorValue,
            #[serde(default)]
            filled: bool,
        },
        
        #[serde(rename = "circle")]
        Circle {
            x: i32,
            y: i32,
            radius: u32,
            color: ColorValue,
            #[serde(default)]
            filled: bool,
        },
        
        #[serde(rename = "image")]
        Image {
            x: i32,
            y: i32,
            data: String,  // Base64
            #[serde(default)]
            width: Option<u32>,
            #[serde(default)]
            height: Option<u32>,
        },
        
        #[serde(rename = "emoji")]
        Emoji {
            x: i32,
            y: i32,
            emoji: String,
            #[serde(default = "default_emoji_size")]
            size: u8,
        },
        
        #[serde(rename = "batch")]
        Batch {
            commands: Vec<DrawCommand>,
        },
    }
    
    fn default_black_color() -> ColorValue {
        ColorValue::RGB([0, 0, 0])
    }
    
    fn default_line_width() -> u8 {
        1
    }
    
    fn default_emoji_size() -> u8 {
        32
    }
    
    /// 色の値（名前またはRGB配列）
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(untagged)]
    pub enum ColorValue {
        Name(String),
        RGB([u8; 3]),
    }
    
    impl ColorValue {
        pub fn to_rgb(&self) -> RGB {
            match self {
                ColorValue::RGB([r, g, b]) => RGB::new(*r, *g, *b),
                ColorValue::Name(name) => parse_color_name(name),
            }
        }
    }
    
    /// サイズの値（文字列または列挙型）
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(untagged)]
    pub enum SizeValue {
        Name(String),
        Number(u8),
    }
    
    impl SizeValue {
        pub fn to_size(&self) -> Size {
            match self {
                SizeValue::Name(name) => Size::from_str(name),
                SizeValue::Number(n) => Size::from_str(&n.to_string()),
            }
        }
    }
    
    /// /api/update リクエスト
    #[derive(Debug, Deserialize, Serialize)]
    pub struct UpdateRequest {
        pub device: Option<String>,
        #[serde(default)]
        pub partial: bool,
    }
    
    /// /api/devices レスポンス
    #[derive(Debug, Serialize, Deserialize)]
    pub struct DevicesResponse {
        pub devices: Vec<DeviceInfo>,
        pub total: usize,
        pub connected: usize,
    }
    
    /// /api/health レスポンス
    #[derive(Debug, Serialize, Deserialize)]
    pub struct HealthResponse {
        pub status: String,
        pub version: String,
        pub platform: String,
        pub uptime_seconds: u64,
        pub bluetooth: BluetoothHealth,
        pub memory: MemoryInfo,
        pub api: ApiStatistics,
    }
    
    /// Bluetoothヘルス情報
    #[derive(Debug, Serialize, Deserialize)]
    pub struct BluetoothHealth {
        pub status: String,
        pub adapter: Option<String>,
        pub devices_connected: usize,
        pub devices_available: usize,
    }
    
    /// メモリ情報
    #[derive(Debug, Serialize, Deserialize)]
    pub struct MemoryInfo {
        pub used_mb: u64,
        pub total_mb: u64,
    }
    
    /// API統計情報
    #[derive(Debug, Serialize, Deserialize)]
    pub struct ApiStatistics {
        pub requests_total: u64,
        pub requests_per_minute: f64,
        pub errors_total: u64,
        pub average_response_time_ms: f64,
    }
    
    /// バッチ操作リクエスト
    #[derive(Debug, Deserialize, Serialize)]
    pub struct BatchRequest {
        pub operations: Vec<BatchOperation>,
        #[serde(default = "default_true")]
        pub parallel: bool,
    }
    
    /// バッチ操作
    #[derive(Debug, Deserialize, Serialize)]
    pub struct BatchOperation {
        pub device: String,
        pub command: DrawCommand,
    }
    
    /// バッチ操作レスポンス
    #[derive(Debug, Serialize, Deserialize)]
    pub struct BatchResponse {
        pub status: String,
        pub results: Vec<BatchResult>,
        pub total_execution_time_ms: u64,
    }
    
    /// バッチ操作結果
    #[derive(Debug, Serialize, Deserialize)]
    pub struct BatchResult {
        pub index: usize,
        pub device: String,
        pub status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<String>,
    }
    
    fn default_true() -> bool {
        true
    }
}

/// 共通レスポンス
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
    pub timestamp: String,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        ApiResponse {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
    
    pub fn error(error: ApiError) -> Self {
        ApiResponse {
            success: false,
            data: None,
            error: Some(error),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// APIエラー
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// 色名をRGBに変換（140色対応）
pub fn parse_color_name(name: &str) -> RGB {
    match name.to_lowercase().as_str() {
        // 基本色
        "black" => RGB::new(0, 0, 0),
        "white" => RGB::new(255, 255, 255),
        "red" => RGB::new(255, 0, 0),
        "green" => RGB::new(0, 255, 0),
        "blue" => RGB::new(0, 0, 255),
        "yellow" => RGB::new(255, 255, 0),
        "cyan" => RGB::new(0, 255, 255),
        "magenta" => RGB::new(255, 0, 255),
        
        // 一般的な色
        "orange" => RGB::new(255, 165, 0),
        "purple" => RGB::new(128, 0, 128),
        "brown" => RGB::new(165, 42, 42),
        "pink" => RGB::new(255, 192, 203),
        "gray" | "grey" => RGB::new(128, 128, 128),
        "darkgreen" => RGB::new(0, 100, 0),
        "darkcyan" => RGB::new(0, 139, 139),
        "maroon" => RGB::new(128, 0, 0),
        "navy" => RGB::new(0, 0, 128),
        "olive" => RGB::new(128, 128, 0),
        "lightgrey" | "lightgray" => RGB::new(211, 211, 211),
        "darkgrey" | "darkgray" => RGB::new(169, 169, 169),
        "teal" => RGB::new(0, 128, 128),
        
        // 拡張色（CSS/HTML標準色）
        "aliceblue" => RGB::new(240, 248, 255),
        "antiquewhite" => RGB::new(250, 235, 215),
        "aqua" => RGB::new(0, 255, 255),
        "aquamarine" => RGB::new(127, 255, 212),
        "azure" => RGB::new(240, 255, 255),
        "beige" => RGB::new(245, 245, 220),
        "bisque" => RGB::new(255, 228, 196),
        "blanchedalmond" => RGB::new(255, 235, 205),
        "blueviolet" => RGB::new(138, 43, 226),
        "burlywood" => RGB::new(222, 184, 135),
        "cadetblue" => RGB::new(95, 158, 160),
        "chartreuse" => RGB::new(127, 255, 0),
        "chocolate" => RGB::new(210, 105, 30),
        "coral" => RGB::new(255, 127, 80),
        "cornflowerblue" => RGB::new(100, 149, 237),
        "cornsilk" => RGB::new(255, 248, 220),
        "crimson" => RGB::new(220, 20, 60),
        "darkblue" => RGB::new(0, 0, 139),
        "darkgoldenrod" => RGB::new(184, 134, 11),
        "darkkhaki" => RGB::new(189, 183, 107),
        "darkmagenta" => RGB::new(139, 0, 139),
        "darkolivegreen" => RGB::new(85, 107, 47),
        "darkorange" => RGB::new(255, 140, 0),
        "darkorchid" => RGB::new(153, 50, 204),
        "darkred" => RGB::new(139, 0, 0),
        "darksalmon" => RGB::new(233, 150, 122),
        "darkseagreen" => RGB::new(143, 188, 143),
        "darkslateblue" => RGB::new(72, 61, 139),
        "darkslategray" | "darkslategrey" => RGB::new(47, 79, 79),
        "darkturquoise" => RGB::new(0, 206, 209),
        "darkviolet" => RGB::new(148, 0, 211),
        "deeppink" => RGB::new(255, 20, 147),
        "deepskyblue" => RGB::new(0, 191, 255),
        "dimgray" | "dimgrey" => RGB::new(105, 105, 105),
        "dodgerblue" => RGB::new(30, 144, 255),
        "firebrick" => RGB::new(178, 34, 34),
        "floralwhite" => RGB::new(255, 250, 240),
        "forestgreen" => RGB::new(34, 139, 34),
        "fuchsia" => RGB::new(255, 0, 255),
        "gainsboro" => RGB::new(220, 220, 220),
        "ghostwhite" => RGB::new(248, 248, 255),
        "gold" => RGB::new(255, 215, 0),
        "goldenrod" => RGB::new(218, 165, 32),
        "greenyellow" => RGB::new(173, 255, 47),
        "honeydew" => RGB::new(240, 255, 240),
        "hotpink" => RGB::new(255, 105, 180),
        "indianred" => RGB::new(205, 92, 92),
        "indigo" => RGB::new(75, 0, 130),
        "ivory" => RGB::new(255, 255, 240),
        "khaki" => RGB::new(240, 230, 140),
        "lavender" => RGB::new(230, 230, 250),
        "lavenderblush" => RGB::new(255, 240, 245),
        "lawngreen" => RGB::new(124, 252, 0),
        "lemonchiffon" => RGB::new(255, 250, 205),
        "lightblue" => RGB::new(173, 216, 230),
        "lightcoral" => RGB::new(240, 128, 128),
        "lightcyan" => RGB::new(224, 255, 255),
        "lightgoldenrodyellow" => RGB::new(250, 250, 210),
        "lightgreen" => RGB::new(144, 238, 144),
        "lightpink" => RGB::new(255, 182, 193),
        "lightsalmon" => RGB::new(255, 160, 122),
        "lightseagreen" => RGB::new(32, 178, 170),
        "lightskyblue" => RGB::new(135, 206, 250),
        "lightslategray" | "lightslategrey" => RGB::new(119, 136, 153),
        "lightsteelblue" => RGB::new(176, 196, 222),
        "lightyellow" => RGB::new(255, 255, 224),
        "lime" => RGB::new(0, 255, 0),
        "limegreen" => RGB::new(50, 205, 50),
        "linen" => RGB::new(250, 240, 230),
        "mediumaquamarine" => RGB::new(102, 205, 170),
        "mediumblue" => RGB::new(0, 0, 205),
        "mediumorchid" => RGB::new(186, 85, 211),
        "mediumpurple" => RGB::new(147, 112, 219),
        "mediumseagreen" => RGB::new(60, 179, 113),
        "mediumslateblue" => RGB::new(123, 104, 238),
        "mediumspringgreen" => RGB::new(0, 250, 154),
        "mediumturquoise" => RGB::new(72, 209, 204),
        "mediumvioletred" => RGB::new(199, 21, 133),
        "midnightblue" => RGB::new(25, 25, 112),
        "mintcream" => RGB::new(245, 255, 250),
        "mistyrose" => RGB::new(255, 228, 225),
        "moccasin" => RGB::new(255, 228, 181),
        "navajowhite" => RGB::new(255, 222, 173),
        "oldlace" => RGB::new(253, 245, 230),
        "olivedrab" => RGB::new(107, 142, 35),
        "orangered" => RGB::new(255, 69, 0),
        "orchid" => RGB::new(218, 112, 214),
        "palegoldenrod" => RGB::new(238, 232, 170),
        "palegreen" => RGB::new(152, 251, 152),
        "paleturquoise" => RGB::new(175, 238, 238),
        "palevioletred" => RGB::new(219, 112, 147),
        "papayawhip" => RGB::new(255, 239, 213),
        "peachpuff" => RGB::new(255, 218, 185),
        "peru" => RGB::new(205, 133, 63),
        "plum" => RGB::new(221, 160, 221),
        "powderblue" => RGB::new(176, 224, 230),
        "rosybrown" => RGB::new(188, 143, 143),
        "royalblue" => RGB::new(65, 105, 225),
        "saddlebrown" => RGB::new(139, 69, 19),
        "salmon" => RGB::new(250, 128, 114),
        "sandybrown" => RGB::new(244, 164, 96),
        "seagreen" => RGB::new(46, 139, 87),
        "seashell" => RGB::new(255, 245, 238),
        "sienna" => RGB::new(160, 82, 45),
        "silver" => RGB::new(192, 192, 192),
        "skyblue" => RGB::new(135, 206, 235),
        "slateblue" => RGB::new(106, 90, 205),
        "slategray" | "slategrey" => RGB::new(112, 128, 144),
        "snow" => RGB::new(255, 250, 250),
        "springgreen" => RGB::new(0, 255, 127),
        "steelblue" => RGB::new(70, 130, 180),
        "tan" => RGB::new(210, 180, 140),
        "thistle" => RGB::new(216, 191, 216),
        "tomato" => RGB::new(255, 99, 71),
        "turquoise" => RGB::new(64, 224, 208),
        "violet" => RGB::new(238, 130, 238),
        "wheat" => RGB::new(245, 222, 179),
        "whitesmoke" => RGB::new(245, 245, 245),
        "yellowgreen" => RGB::new(154, 205, 50),
        
        // HEX形式のチェック（6桁 #RRGGBB）
        hex if hex.starts_with('#') && hex.len() == 7 => {
            parse_hex_color(hex).unwrap_or(RGB::white())
        }
        // HEX形式のチェック（3桁 #RGB）
        hex if hex.starts_with('#') && hex.len() == 4 => {
            parse_hex_color_short(hex).unwrap_or(RGB::white())
        }
        
        // RGBA形式のチェック（例: "rgba(255,0,0,0.8)"）
        rgba if rgba.starts_with("rgba(") && rgba.ends_with(')') => {
            let inner = &rgba[5..rgba.len()-1];
            parse_rgba_string(inner).unwrap_or(RGB::white())
        }
        
        // RGB形式のチェック（例: "rgb(255,0,0)"）
        rgb if rgb.starts_with("rgb(") && rgb.ends_with(')') => {
            let inner = &rgb[4..rgb.len()-1];
            parse_rgb_string(inner).unwrap_or(RGB::white())
        }
        
        // カンマ区切りRGB形式（例: "255,0,0"）
        rgb if rgb.contains(',') => {
            parse_rgb_string(rgb).unwrap_or(RGB::white())
        }
        
        // スペース区切りRGB形式（例: "255 0 0"）
        rgb if rgb.contains(' ') && rgb.split_whitespace().count() == 3 => {
            parse_rgb_space_string(rgb).unwrap_or(RGB::white())
        }
        
        _ => RGB::white(),
    }
}

/// RGB文字列をパース（"255,0,0"形式）
fn parse_rgb_string(rgb: &str) -> Option<RGB> {
    let parts: Vec<&str> = rgb.split(',').map(|s| s.trim()).collect();
    if parts.len() != 3 {
        return None;
    }
    
    let r = parts[0].parse::<u8>().ok()?;
    let g = parts[1].parse::<u8>().ok()?;
    let b = parts[2].parse::<u8>().ok()?;
    
    Some(RGB::new(r, g, b))
}

/// HEX色をRGBに変換（6桁 #RRGGBB）
pub fn parse_hex_color(hex: &str) -> Option<RGB> {
    if !hex.starts_with('#') || hex.len() != 7 {
        return None;
    }
    
    let r = u8::from_str_radix(&hex[1..3], 16).ok()?;
    let g = u8::from_str_radix(&hex[3..5], 16).ok()?;
    let b = u8::from_str_radix(&hex[5..7], 16).ok()?;
    
    Some(RGB::new(r, g, b))
}

/// HEX色をRGBに変換（3桁 #RGB）
pub fn parse_hex_color_short(hex: &str) -> Option<RGB> {
    if !hex.starts_with('#') || hex.len() != 4 {
        return None;
    }
    
    let r_char = hex.chars().nth(1)?;
    let g_char = hex.chars().nth(2)?;
    let b_char = hex.chars().nth(3)?;
    
    let r = u8::from_str_radix(&format!("{}{}", r_char, r_char), 16).ok()?;
    let g = u8::from_str_radix(&format!("{}{}", g_char, g_char), 16).ok()?;
    let b = u8::from_str_radix(&format!("{}{}", b_char, b_char), 16).ok()?;
    
    Some(RGB::new(r, g, b))
}

/// RGBA文字列をパース（"255,0,0,0.8"形式、アルファは無視）
fn parse_rgba_string(rgba: &str) -> Option<RGB> {
    let parts: Vec<&str> = rgba.split(',').map(|s| s.trim()).collect();
    if parts.len() != 4 {
        return None;
    }
    
    let r = parts[0].parse::<u8>().ok()?;
    let g = parts[1].parse::<u8>().ok()?;
    let b = parts[2].parse::<u8>().ok()?;
    // アルファ値は無視（parts[3]）
    
    Some(RGB::new(r, g, b))
}

/// スペース区切りRGB文字列をパース（"255 0 0"形式）
fn parse_rgb_space_string(rgb: &str) -> Option<RGB> {
    let parts: Vec<&str> = rgb.split_whitespace().collect();
    if parts.len() != 3 {
        return None;
    }
    
    let r = parts[0].parse::<u8>().ok()?;
    let g = parts[1].parse::<u8>().ok()?;
    let b = parts[2].parse::<u8>().ok()?;
    
    Some(RGB::new(r, g, b))
}