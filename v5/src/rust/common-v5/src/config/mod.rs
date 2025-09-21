//! 共通設定管理モジュール

use serde::{Deserialize, Serialize};
use std::env;
use crate::error::{NotifError, Result};

/// サーバー設定
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// バインドするホストアドレス
    pub host: String,
    
    /// ポート番号
    pub port: u16,
    
    /// ワーカースレッド数
    pub workers: Option<usize>,
    
    /// リクエストタイムアウト（秒）
    pub request_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            host: "0.0.0.0".to_string(),
            port: 18080,
            workers: None,
            request_timeout_secs: 30,
        }
    }
}

/// Bluetooth設定
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BluetoothConfig {
    /// デバイス名のプレフィックス
    pub device_name_prefix: String,
    
    /// スキャンタイムアウト（秒）
    pub scan_timeout_secs: u64,
    
    /// 自動再接続
    pub auto_reconnect: bool,
    
    /// 再接続試行回数
    pub reconnect_attempts: u32,
    
    /// 再接続間隔（秒）
    pub reconnect_interval_secs: u64,
    
    /// 最大同時接続数
    pub max_connections: usize,
    
    /// コマンドタイムアウト（ミリ秒）
    pub command_timeout_ms: u64,
}

impl Default for BluetoothConfig {
    fn default() -> Self {
        BluetoothConfig {
            device_name_prefix: "notif_atoms3".to_string(),
            scan_timeout_secs: 10,
            auto_reconnect: true,
            reconnect_attempts: 3,
            reconnect_interval_secs: 5,
            max_connections: 10,
            command_timeout_ms: 5000,
        }
    }
}

/// ロギング設定
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// ログレベル
    pub level: String,
    
    /// ログ出力先
    pub output: String,
    
    /// ログファイルパス（ファイル出力の場合）
    pub file_path: Option<String>,
    
    /// ログローテーションサイズ（MB）
    pub rotation_size_mb: Option<u64>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        LoggingConfig {
            level: "info".to_string(),
            output: "stdout".to_string(),
            file_path: None,
            rotation_size_mb: None,
        }
    }
}

/// API設定
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiConfig {
    /// CORS許可オリジン
    pub cors_origins: Vec<String>,
    
    /// レート制限（リクエスト/分）
    pub rate_limit_per_minute: Option<u32>,
    
    /// 最大リクエストボディサイズ（バイト）
    pub max_body_size: usize,
    
    /// APIキー認証の有効化
    pub api_key_enabled: bool,
    
    /// APIキー（有効な場合）
    pub api_key: Option<String>,
}

/// パフォーマンス設定
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    /// 高優先度プロセス（Windows）
    pub high_priority: Option<bool>,
    
    /// スレッドプール最適化
    pub optimize_thread_pool: Option<bool>,
    
    /// メモリプール使用
    pub use_memory_pool: Option<bool>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        ApiConfig {
            cors_origins: vec!["*".to_string()],
            rate_limit_per_minute: None,
            max_body_size: 10 * 1024 * 1024, // 10MB
            api_key_enabled: false,
            api_key: None,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        PerformanceConfig {
            high_priority: Some(false),
            optimize_thread_pool: Some(true),
            use_memory_pool: Some(false),
        }
    }
}

/// アプリケーション設定
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    /// サーバー設定
    pub server: ServerConfig,
    
    /// Bluetooth設定
    pub bluetooth: BluetoothConfig,
    
    /// ロギング設定
    pub logging: LoggingConfig,
    
    /// API設定
    pub api: ApiConfig,
    
    /// パフォーマンス設定
    pub performance: PerformanceConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            server: ServerConfig::default(),
            bluetooth: BluetoothConfig::default(),
            logging: LoggingConfig::default(),
            api: ApiConfig::default(),
            performance: PerformanceConfig::default(),
        }
    }
}

impl Settings {
    /// 設定を読み込む
    /// 
    /// 読み込み優先順位：
    /// 1. 環境変数
    /// 2. 設定ファイル（指定された場合）
    /// 3. デフォルト値
    pub fn new() -> Result<Self> {
        let mut settings = Self::default();
        
        // 設定ファイルパスを環境変数から取得
        if let Ok(config_path) = env::var("CONFIG_FILE") {
            settings = Self::from_file(&config_path)?;
        }
        
        // 環境変数で上書き
        settings.override_from_env();
        
        Ok(settings)
    }
    
    /// 設定ファイルから読み込む
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| NotifError::Config(format!("Failed to read config file: {}", e)))?;
        
        // JSON形式
        if path.ends_with(".json") {
            serde_json::from_str(&content)
                .map_err(|e| NotifError::Config(format!("Failed to parse JSON config: {}", e)))
        }
        // TOML形式
        else if path.ends_with(".toml") {
            toml::from_str(&content)
                .map_err(|e| NotifError::Config(format!("Failed to parse TOML config: {}", e)))
        }
        // YAML形式
        else if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content)
                .map_err(|e| NotifError::Config(format!("Failed to parse YAML config: {}", e)))
        } else {
            Err(NotifError::Config("Unsupported config file format".to_string()))
        }
    }
    
    /// 環境変数で設定を上書き
    fn override_from_env(&mut self) {
        // サーバー設定
        if let Ok(host) = env::var("HOST") {
            self.server.host = host;
        }
        if let Ok(port) = env::var("PORT") {
            if let Ok(port) = port.parse() {
                self.server.port = port;
            }
        }
        if let Ok(workers) = env::var("WORKERS") {
            if let Ok(workers) = workers.parse() {
                self.server.workers = Some(workers);
            }
        }
        
        // Bluetooth設定
        if let Ok(device_name) = env::var("DEVICE_NAME_PREFIX") {
            self.bluetooth.device_name_prefix = device_name;
        }
        if let Ok(scan_timeout) = env::var("SCAN_TIMEOUT") {
            if let Ok(timeout) = scan_timeout.parse() {
                self.bluetooth.scan_timeout_secs = timeout;
            }
        }
        if let Ok(auto_reconnect) = env::var("AUTO_RECONNECT") {
            self.bluetooth.auto_reconnect = auto_reconnect.to_lowercase() == "true" 
                || auto_reconnect == "1";
        }
        if let Ok(max_connections) = env::var("MAX_CONNECTIONS") {
            if let Ok(max) = max_connections.parse() {
                self.bluetooth.max_connections = max;
            }
        }
        
        // ロギング設定
        if let Ok(log_level) = env::var("LOG_LEVEL") {
            self.logging.level = log_level;
        }
        if let Ok(log_output) = env::var("LOG_OUTPUT") {
            self.logging.output = log_output;
        }
        if let Ok(log_file) = env::var("LOG_FILE") {
            self.logging.file_path = Some(log_file);
        }
        
        // API設定
        if let Ok(cors_origins) = env::var("CORS_ORIGINS") {
            self.api.cors_origins = cors_origins.split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }
        if let Ok(rate_limit) = env::var("RATE_LIMIT") {
            if let Ok(limit) = rate_limit.parse() {
                self.api.rate_limit_per_minute = Some(limit);
            }
        }
        if let Ok(api_key_enabled) = env::var("API_KEY_ENABLED") {
            self.api.api_key_enabled = api_key_enabled.to_lowercase() == "true" 
                || api_key_enabled == "1";
        }
        if let Ok(api_key) = env::var("API_KEY") {
            self.api.api_key = Some(api_key);
        }
    }
    
    /// 設定を検証
    pub fn validate(&self) -> Result<()> {
        // ポート番号の検証
        if self.server.port == 0 {
            return Err(NotifError::Config("Invalid port number: 0".to_string()));
        }
        
        // デバイス名プレフィックスの検証
        if self.bluetooth.device_name_prefix.is_empty() {
            return Err(NotifError::Config("Device name prefix cannot be empty".to_string()));
        }
        
        // APIキーの検証
        if self.api.api_key_enabled && self.api.api_key.is_none() {
            return Err(NotifError::Config("API key is required when API key authentication is enabled".to_string()));
        }
        
        Ok(())
    }
    
    /// 設定をファイルに保存
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let content = if path.ends_with(".json") {
            serde_json::to_string_pretty(self)
                .map_err(|e| NotifError::Config(format!("Failed to serialize to JSON: {}", e)))?
        } else if path.ends_with(".toml") {
            toml::to_string_pretty(self)
                .map_err(|e| NotifError::Config(format!("Failed to serialize to TOML: {}", e)))?
        } else if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::to_string(self)
                .map_err(|e| NotifError::Config(format!("Failed to serialize to YAML: {}", e)))?
        } else {
            return Err(NotifError::Config("Unsupported config file format".to_string()));
        };
        
        std::fs::write(path, content)
            .map_err(|e| NotifError::Config(format!("Failed to write config file: {}", e)))?;
        
        Ok(())
    }
}

// 追加の依存関係が必要な場合のfeature flag
#[cfg(feature = "config-toml")]
use toml;

#[cfg(feature = "config-yaml")]
use serde_yaml;