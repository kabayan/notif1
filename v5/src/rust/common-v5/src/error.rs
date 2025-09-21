//! 共通エラー型定義

use thiserror::Error;

/// notif v3共通エラー型
#[derive(Debug, Error)]
pub enum NotifError {
    /// Bluetooth関連エラー
    #[error("Bluetooth error: {0}")]
    Bluetooth(String),
    
    /// デバイスが見つからない
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
    
    /// デバイスが接続されていない
    #[error("Device not connected: {0}")]
    DeviceNotConnected(String),
    
    /// 接続エラー
    #[error("Connection error: {0}")]
    Connection(String),
    
    /// 無効な色指定
    #[error("Invalid color: {0}")]
    InvalidColor(String),
    
    /// 無効なコマンド
    #[error("Invalid command: {0}")]
    InvalidCommand(String),
    
    /// 無効なパラメータ
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    
    /// 設定エラー
    #[error("Configuration error: {0}")]
    Config(String),
    
    /// タイムアウト
    #[error("Operation timeout: {0}")]
    Timeout(String),
    
    /// IO エラー
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    /// JSON パースエラー
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    
    /// UTF-8変換エラー
    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    
    /// プラットフォーム固有エラー
    #[error("Platform specific error: {0}")]
    Platform(String),
    
    /// その他のエラー
    #[error("Other error: {0}")]
    Other(String),
    
    // v5新機能エラー（追加のみ）
    #[error("画像処理エラー: {0}")]
    ImageProcessing(String),
    
    #[error("サポートされていない画像形式: {0}")]
    UnsupportedFormat(String),
    
    #[error("画像ファイルサイズが大きすぎます: {0} バイト（最大: {1} バイト）")]
    ImageTooLarge(usize, usize),
    
    #[error("未実装: {0}")]
    NotImplemented(String),
}

/// Result型のエイリアス
pub type Result<T> = std::result::Result<T, NotifError>;

// Note: Windows APIエラーの変換は各プラットフォーム実装で手動で行う

impl NotifError {
    /// HTTPステータスコードを返す
    pub fn status_code(&self) -> u16 {
        match self {
            NotifError::DeviceNotFound(_) => 404,
            NotifError::DeviceNotConnected(_) => 503,
            NotifError::Connection(_) => 503,
            NotifError::InvalidColor(_) | 
            NotifError::InvalidCommand(_) | 
            NotifError::InvalidParameter(_) => 400,
            NotifError::Config(_) => 500,
            NotifError::Timeout(_) => 408,
            NotifError::Bluetooth(_) | 
            NotifError::Platform(_) | 
            NotifError::Other(_) => 500,
            NotifError::Io(_) | 
            NotifError::Json(_) | 
            NotifError::Utf8(_) => 500,
            // v5新機能エラー
            NotifError::ImageProcessing(_) => 500,
            NotifError::UnsupportedFormat(_) => 400,
            NotifError::ImageTooLarge(_, _) => 413,
            NotifError::NotImplemented(_) => 501,
        }
    }
    
    /// エラーコードを返す（APIレスポンス用）
    pub fn error_code(&self) -> &str {
        match self {
            NotifError::DeviceNotFound(_) => "DEVICE_NOT_FOUND",
            NotifError::DeviceNotConnected(_) => "DEVICE_NOT_CONNECTED",
            NotifError::Connection(_) => "CONNECTION_ERROR",
            NotifError::InvalidColor(_) => "INVALID_COLOR",
            NotifError::InvalidCommand(_) => "INVALID_COMMAND",
            NotifError::InvalidParameter(_) => "INVALID_PARAMETER",
            NotifError::Config(_) => "CONFIG_ERROR",
            NotifError::Timeout(_) => "TIMEOUT",
            NotifError::Bluetooth(_) => "BLUETOOTH_ERROR",
            NotifError::Platform(_) => "PLATFORM_ERROR",
            NotifError::Io(_) => "IO_ERROR",
            NotifError::Json(_) => "JSON_ERROR",
            NotifError::Utf8(_) => "UTF8_ERROR",
            NotifError::Other(_) => "UNKNOWN_ERROR",
            // v5新機能エラー
            NotifError::ImageProcessing(_) => "IMAGE_PROCESSING_ERROR",
            NotifError::UnsupportedFormat(_) => "UNSUPPORTED_FORMAT",
            NotifError::ImageTooLarge(_, _) => "IMAGE_TOO_LARGE",
            NotifError::NotImplemented(_) => "NOT_IMPLEMENTED",
        }
    }
}