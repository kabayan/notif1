//! Bluetooth抽象化トレイト

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::Arc;
use crate::error::Result;
use crate::protocol::Command;

/// デバイス情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// デバイス名
    pub name: String,
    
    /// Bluetoothアドレス
    pub address: String,
    
    /// 接続状態
    pub connected: bool,
    
    /// デバイス番号（1から始まる）
    pub number: Option<usize>,
    
    /// 信号強度（RSSI）
    pub signal_strength: Option<i8>,
    
    /// バッテリーレベル（パーセント）
    pub battery_level: Option<u8>,
    
    /// デバイス機能
    pub capabilities: DeviceCapabilities,
}

/// デバイス機能
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    /// ディスプレイ対応
    pub display: bool,
    
    /// カラー表示対応
    pub color: bool,
    
    /// 絵文字対応
    pub emoji: bool,
    
    /// 領域分割対応
    pub regions: bool,
    
    /// 画面サイズ
    pub display_width: u32,
    pub display_height: u32,
    
    /// 色深度（ビット）
    pub color_depth: u8,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        DeviceCapabilities {
            display: true,
            color: true,
            emoji: true,
            regions: true,
            display_width: 128,
            display_height: 128,
            color_depth: 16,
        }
    }
}

/// Bluetooth接続トレイト
#[async_trait]
pub trait Connection: Send + Sync + Debug {
    /// コマンドを送信
    async fn send_command(&mut self, command: Command) -> Result<()>;
    
    /// 接続状態を確認
    async fn is_connected(&self) -> bool;
    
    /// デバイス情報を取得
    async fn get_device_info(&self) -> DeviceInfo;
    
    /// 切断
    async fn disconnect(&mut self) -> Result<()>;
    
    /// 再接続
    async fn reconnect(&mut self) -> Result<()>;
    
    /// バッテリーレベルを取得（オプション）
    async fn get_battery_level(&self) -> Option<u8> {
        None
    }
    
    /// 信号強度を取得（オプション）
    async fn get_signal_strength(&self) -> Option<i8> {
        None
    }
}

/// デバイススキャナートレイト
#[async_trait]
pub trait Scanner: Send + Sync {
    /// デバイスをスキャン
    async fn scan(
        &self,
        prefix: &str,
        timeout: std::time::Duration,
    ) -> Result<Vec<DeviceInfo>>;
    
    /// 特定のデバイスをスキャン
    async fn scan_for_device(
        &self,
        device_name: &str,
        timeout: std::time::Duration,
    ) -> Result<Option<DeviceInfo>>;
    
    /// デバイスに接続
    async fn connect(&self, device_info: &DeviceInfo) -> Result<Box<dyn Connection>>;
    
    /// スキャンを停止
    async fn stop_scan(&self) -> Result<()>;
}

/// Bluetoothマネージャートレイト
#[async_trait]
pub trait BluetoothManager: Send + Sync {
    /// スキャナーを作成
    fn create_scanner(&self) -> Result<Box<dyn Scanner>>;
    
    /// リトライ機能付きでデバイスに接続
    async fn connect_with_retry(
        &self,
        scanner: &dyn Scanner,
        device_info: &DeviceInfo,
    ) -> Result<Box<dyn Connection>>;
    
    /// 全デバイスをスキャンして接続
    async fn scan_and_connect_all(&self) -> Result<Vec<String>>;
    
    /// 特定のデバイスにコマンドを送信
    async fn send_command_to_device(
        &self,
        device_id: &str,
        command: Command,
    ) -> Result<()>;
    
    /// 全デバイスにコマンドを送信
    async fn send_command_to_all(&self, command: Command) -> Result<()>;
    
    /// デバイス番号を指定してコマンドを送信
    async fn send_command_by_number(
        &self,
        number: usize,
        command: Command,
    ) -> Result<()>;
    
    /// 接続されているデバイスのリストを取得
    async fn list_connected_devices(&self) -> Vec<DeviceInfo>;
    
    /// 特定のデバイスの接続状態を確認
    async fn is_device_connected(&self, device_id: &str) -> bool;
    
    /// 特定のデバイスを切断
    async fn disconnect_device(&self, device_id: &str) -> Result<()>;
    
    /// 全デバイスを切断
    async fn disconnect_all(&self) -> Result<()>;
    
    /// 特定のデバイスに再接続
    async fn reconnect_device(&self, device_id: &str) -> Result<()>;
    
    /// デバイスの自動再接続を設定
    async fn set_auto_reconnect(&self, enabled: bool) -> Result<()>;
    
    /// デバイスの統計情報を取得
    async fn get_statistics(&self) -> DeviceStatistics;
    
    /// keepaliveタスクを開始（Windows環境用の明示的な呼び出し）
    fn start_keepalive(&self);
    
    /// v5追加: 画像タイルを保存（再接続時の復元用）
    async fn save_image_tiles(&self, device_id: &str, tiles: Vec<Command>);
    
    /// v5追加: デバイス番号からデバイス名を取得
    async fn get_device_name_by_number(&self, number: usize) -> Option<String>;
}

/// デバイス統計情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatistics {
    /// 総デバイス数
    pub total_devices: usize,
    
    /// 接続中のデバイス数
    pub connected_devices: usize,
    
    /// 送信コマンド総数
    pub total_commands_sent: u64,
    
    /// エラー総数
    pub total_errors: u64,
    
    /// 平均応答時間（ミリ秒）
    pub average_response_time_ms: f64,
    
    /// 稼働時間（秒）
    pub uptime_seconds: u64,
}

impl Default for DeviceStatistics {
    fn default() -> Self {
        DeviceStatistics {
            total_devices: 0,
            connected_devices: 0,
            total_commands_sent: 0,
            total_errors: 0,
            average_response_time_ms: 0.0,
            uptime_seconds: 0,
        }
    }
}

/// プラットフォーム固有データを保持するトレイト
pub trait PlatformData: Send + Sync + Debug {
    /// プラットフォーム名を取得
    fn platform_name(&self) -> &str;
    
    /// デバッグ情報を取得
    fn debug_info(&self) -> String;
}

/// Arc<T>にBluetoothManagerトレイトを実装
#[async_trait]
impl<T: BluetoothManager> BluetoothManager for Arc<T> {
    fn create_scanner(&self) -> Result<Box<dyn Scanner>> {
        (**self).create_scanner()
    }
    
    async fn connect_with_retry(
        &self,
        scanner: &dyn Scanner,
        device_info: &DeviceInfo,
    ) -> Result<Box<dyn Connection>> {
        (**self).connect_with_retry(scanner, device_info).await
    }
    
    async fn scan_and_connect_all(&self) -> Result<Vec<String>> {
        (**self).scan_and_connect_all().await
    }
    
    async fn send_command_to_device(
        &self,
        device_id: &str,
        command: Command,
    ) -> Result<()> {
        (**self).send_command_to_device(device_id, command).await
    }
    
    async fn send_command_to_all(&self, command: Command) -> Result<()> {
        (**self).send_command_to_all(command).await
    }
    
    async fn send_command_by_number(
        &self,
        device_number: usize,
        command: Command,
    ) -> Result<()> {
        (**self).send_command_by_number(device_number, command).await
    }
    
    async fn list_connected_devices(&self) -> Vec<DeviceInfo> {
        (**self).list_connected_devices().await
    }
    
    async fn is_device_connected(&self, device_id: &str) -> bool {
        (**self).is_device_connected(device_id).await
    }
    
    async fn disconnect_device(&self, device_id: &str) -> Result<()> {
        (**self).disconnect_device(device_id).await
    }
    
    async fn disconnect_all(&self) -> Result<()> {
        (**self).disconnect_all().await
    }
    
    async fn reconnect_device(&self, device_id: &str) -> Result<()> {
        (**self).reconnect_device(device_id).await
    }
    
    async fn set_auto_reconnect(&self, enabled: bool) -> Result<()> {
        (**self).set_auto_reconnect(enabled).await
    }
    
    async fn get_statistics(&self) -> DeviceStatistics {
        (**self).get_statistics().await
    }
    
    fn start_keepalive(&self) {
        (**self).start_keepalive()
    }
    
    async fn save_image_tiles(&self, device_id: &str, tiles: Vec<Command>) {
        (**self).save_image_tiles(device_id, tiles).await
    }
    
    async fn get_device_name_by_number(&self, number: usize) -> Option<String> {
        (**self).get_device_name_by_number(number).await
    }
}