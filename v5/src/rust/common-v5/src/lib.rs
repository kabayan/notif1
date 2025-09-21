//! notif v3 共通ライブラリ
//! 
//! Windows/Linux両プラットフォームで共通のコードを提供

pub mod error;
pub mod protocol;
pub mod bluetooth;
pub mod config;
pub mod api;
pub mod text;
pub mod mcp;

// v5新機能（追加のみ）
pub mod image;

// バージョン情報
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

// 主要な型の再エクスポート
pub use error::{NotifError, Result};
pub use protocol::{Command, RGB, Size, StatusCode};
pub use bluetooth::{
    BluetoothManager,
    Connection,
    Scanner,
    DeviceInfo,
    DeviceCapabilities,
    CommonBluetoothManager,
};
pub use config::Settings;
pub use text::{
    TextSegment,
    parse_text_with_emoji,
    process_line_with_emoji,
    wrap_text_with_emoji,
    is_emoji,
    emoji_string_to_codepoint,
    codepoint_to_emoji_string,
};
pub use mcp::{AppState, SessionManager, mcp_handler};

// v5新機能の公開（追加のみ）
pub use image::{ImageProcessor, ProcessedImage, FitMode};

/// プラットフォーム情報
pub fn platform_info() -> PlatformInfo {
    PlatformInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        family: std::env::consts::FAMILY.to_string(),
    }
}

/// プラットフォーム情報構造体
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
    pub family: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
        assert!(!NAME.is_empty());
    }
    
    #[test]
    fn test_platform_info() {
        let info = platform_info();
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
    }
}