//! 共通Bluetoothマネージャー実装

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace, warn};
use std::time::{Duration, Instant};

use crate::error::{NotifError, Result};
use crate::protocol::Command;
use super::traits::{BluetoothManager, Connection, DeviceInfo, DeviceStatistics, Scanner};

/// マルチデバイス管理の共通実装
pub struct CommonBluetoothManager {
    /// デバイス名 -> 接続のマップ
    connections: Arc<RwLock<HashMap<String, Box<dyn Connection>>>>,
    
    /// 接続順番号 -> デバイス名のマップ（1から始まる）
    device_order: Arc<RwLock<Vec<String>>>,
    
    /// デバイス名のプレフィックス
    device_name_prefix: String,
    
    /// 自動再接続フラグ
    auto_reconnect: Arc<RwLock<bool>>,
    
    /// 統計情報
    statistics: Arc<RwLock<Statistics>>,
    
    /// スキャナー作成関数
    scanner_factory: Arc<dyn Fn() -> Result<Box<dyn Scanner>> + Send + Sync>,
    
    /// 最後に送信したコマンド（再接続時の復元用）
    last_commands: Arc<RwLock<HashMap<String, Command>>>,
    
    /// v5追加: 最後に送信した画像の全タイル（再接続時の復元用）
    last_image_tiles: Arc<RwLock<HashMap<String, Vec<crate::protocol::Command>>>>,
}

/// 内部統計情報
struct Statistics {
    start_time: Instant,
    total_commands_sent: u64,
    total_errors: u64,
    total_response_time_ms: u64,
    command_count: u64,
}

impl CommonBluetoothManager {
    /// 新しいマネージャーを作成
    pub fn new<F>(device_name_prefix: String, scanner_factory: F) -> Self
    where
        F: Fn() -> Result<Box<dyn Scanner>> + Send + Sync + 'static,
    {
        CommonBluetoothManager {
            connections: Arc::new(RwLock::new(HashMap::new())),
            device_order: Arc::new(RwLock::new(Vec::new())),
            device_name_prefix,
            auto_reconnect: Arc::new(RwLock::new(true)),
            statistics: Arc::new(RwLock::new(Statistics {
                start_time: Instant::now(),
                total_commands_sent: 0,
                total_errors: 0,
                total_response_time_ms: 0,
                command_count: 0,
            })),
            scanner_factory: Arc::new(scanner_factory),
            last_commands: Arc::new(RwLock::new(HashMap::new())),
            last_image_tiles: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// デバイスを追加
    pub async fn add_device(&self, device_name: String, mut connection: Box<dyn Connection>) -> Result<()> {
        let mut connections = self.connections.write().await;
        let mut device_order = self.device_order.write().await;
        
        // デバイス番号を計算（1から開始）
        let device_number = if connections.contains_key(&device_name) {
            // 既存デバイスの場合、現在の位置を維持
            device_order.iter().position(|name| name == &device_name)
                .map(|pos| pos + 1)
                .unwrap_or(device_order.len() + 1)
        } else {
            // 新しいデバイスの場合
            device_order.len() + 1
        };
        
        // 既に存在する場合は上書き
        if !connections.contains_key(&device_name) {
            device_order.push(device_name.clone());
        }
        
        // 接続が安定するまで待つ（ATOMS3の初期化待ち）
        info!("Waiting for connection to stabilize...");
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        
        // 接続完了メッセージをデバイス画面に表示（v2互換）
        use crate::protocol::{Command, RGB, Size};
        
        // まず単純なClearコマンドのみ送信してテスト
        let clear_command = Command::Clear {
            color: RGB::new(0, 64, 0),  // 暗い緑
        };
        
        info!("Sending test Clear command to device {}...", device_name);
        if let Err(e) = connection.send_command(clear_command.clone()).await {
            warn!("Failed to send Clear command to device {}: {}", device_name, e);
        } else {
            info!("Clear command sent successfully to device {}", device_name);
        }
        
        // 少し待ってから次のコマンドを送信
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        let connect_msg = Command::Text {
            x: 5,
            y: 10,
            size: Size::Medium,
            color: RGB::new(255, 255, 255),
            text: "接続済み".to_string(),
        };
        
        info!("Sending Text command to device {}...", device_name);
        if let Err(e) = connection.send_command(connect_msg.clone()).await {
            warn!("Failed to send Text command to device {}: {}", device_name, e);
        } else {
            info!("Text command sent successfully to device {}", device_name);
        }
        
        // デバイス番号メッセージ
        let device_num_msg = Command::Text {
            x: 5,
            y: 16,  // 接続済みメッセージの下に表示
            size: Size::Small,  // フォントサイズを小さく
            color: RGB::new(255, 255, 255),
            text: format!("Device #{}", device_number),
        };
        
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        if let Err(e) = connection.send_command(device_num_msg.clone()).await {
            warn!("Failed to send device number message to device {}: {}", device_name, e);
        } else {
            info!("Device number message sent successfully to device {}", device_name);
        }
        
        // 初期表示状態を最後のコマンドとして保存（再接続時の復元用）
        {
            let initial_display = Command::Batch {
                commands: vec![
                    clear_command,
                    connect_msg,
                    device_num_msg,
                ],
            };
            let mut last_commands = self.last_commands.write().await;
            last_commands.insert(device_name.clone(), initial_display);
        }
        
        // デバッグ用のBatchコマンドテストは削除（本番環境では不要）
        // 必要に応じて、以下のコメントを解除してテスト可能
        /*
        info!("Testing Batch command to device {}...", device_name);
        let batch_command = Command::Batch {
            commands: vec![
                Command::Clear { color: RGB::new(0, 0, 64) },  // 暗い青に変える
                Command::Text {
                    x: 5,
                    y: 10,
                    size: Size::Medium,
                    color: RGB::new(255, 255, 0),  // 黄色
                    text: "Batch Test".to_string(),
                },
            ],
        };
        
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        
        if let Err(e) = connection.send_command(batch_command).await {
            warn!("Failed to send Batch command to device {}: {}", device_name, e);
        } else {
            info!("Batch command sent successfully to device {}", device_name);
        }
        */
        
        connections.insert(device_name.clone(), connection);
        info!("Added device: {} (position: {})", device_name, device_number);
        
        Ok(())
    }
    
    /// デバイスを削除
    pub async fn remove_device(&self, device_name: &str) -> Result<()> {
        let mut connections = self.connections.write().await;
        let mut device_order = self.device_order.write().await;
        
        if let Some(mut conn) = connections.remove(device_name) {
            // 切断を試みる
            let _ = conn.disconnect().await;
            
            // 順序リストからも削除
            device_order.retain(|name| name != device_name);
            
            info!("Removed device: {}", device_name);
            Ok(())
        } else {
            Err(NotifError::DeviceNotFound(device_name.to_string()))
        }
    }
    
    /// 統計情報を更新
    async fn update_statistics(&self, success: bool, response_time_ms: u64) {
        let mut stats = self.statistics.write().await;
        
        if success {
            stats.total_commands_sent += 1;
            stats.total_response_time_ms += response_time_ms;
            stats.command_count += 1;
        } else {
            stats.total_errors += 1;
        }
    }
    
    // 5秒間隔のkeepaliveタスクを開始（内部メソッド）
    fn start_keepalive_task(&self) {
        // info!("start_keepalive_task called");  // Keepaliveログ抑制
        let connections = self.connections.clone();
        let device_order = self.device_order.clone();
        let last_commands = self.last_commands.clone();
        let last_image_tiles = self.last_image_tiles.clone();  // v5追加
        
        // info!("Spawning keepalive task...");  // Keepaliveログ抑制
        tokio::spawn(async move {
            // info!("Keepalive task started inside spawn");  // Keepaliveログ抑制
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            // 最初のtickは即座に実行されるので、最初のチェックまで5秒待つ
            
            loop {
                // info!("Keepalive: Before interval.tick()");  // Keepaliveログ抑制
                interval.tick().await;
                // info!("Keepalive: After interval.tick() - checking device connections...");  // Keepaliveログ抑制
                
                // 全接続デバイスの接続状態をチェック
                let devices = device_order.read().await.clone();
                // info!("Keepalive: Checking {} devices", devices.len());  // Keepaliveログ抑制
                for device_id in devices {
                    // まず接続の存在をチェック
                    let connection_exists = connections.read().await.contains_key(&device_id);
                    
                    if connection_exists {
                        // 接続状態をチェック
                        let is_connected = {
                            let connections_guard = connections.read().await;
                            if let Some(connection) = connections_guard.get(&device_id) {
                                connection.is_connected().await
                            } else {
                                false
                            }
                        };
                        
                        if !is_connected {
                            // warn!("Keepalive: Device {} disconnected, attempting reconnect", device_id);  // Keepaliveログ抑制
                            
                            // 再接続を試みる
                            let mut connections_guard = connections.write().await;
                            if let Some(connection) = connections_guard.get_mut(&device_id) {
                                if let Err(e) = connection.reconnect().await {
                                    // error!("Keepalive: Failed to reconnect {}: {}", device_id, e);  // Keepaliveログ抑制
                                } else {
                                    // info!("Keepalive: Successfully reconnected {}", device_id);  // Keepaliveログ抑制
                                    
                                    // v5修正: 再接続後、画像タイルがある場合は全タイル再送信
                                    let last_image_tiles = {
                                        let tiles_guard = last_image_tiles.read().await;
                                        tiles_guard.get(&device_id).cloned()
                                    };
                                    
                                    if let Some(tiles) = last_image_tiles {
                                        // info!("Keepalive: Restoring {} image tiles for {}", tiles.len(), device_id);  // Keepaliveログ抑制
                                        for tile_command in tiles {
                                            if let Err(e) = connection.send_command(tile_command).await {
                                                // warn!("Keepalive: Failed to restore image tile for {}: {}", device_id, e);  // Keepaliveログ抑制
                                                break;
                                            }
                                        }
                                    } else {
                                        // 画像でない場合は通常のコマンド復元
                                        let last_command = {
                                            let last_commands_guard = last_commands.read().await;
                                            last_commands_guard.get(&device_id).cloned()
                                        };
                                        
                                        if let Some(command) = last_command {
                                            // info!("Keepalive: Restoring last display for {}", device_id);  // Keepaliveログ抑制
                                            if let Err(e) = connection.send_command(command).await {
                                                // warn!("Keepalive: Failed to restore display for {}: {}", device_id, e);  // Keepaliveログ抑制
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            // info!("Keepalive: Device {} is connected", device_id);  // Keepaliveログ抑制
                        }
                    }
                }
            }
        });
    }
}

#[async_trait]
impl BluetoothManager for CommonBluetoothManager {
    fn create_scanner(&self) -> Result<Box<dyn Scanner>> {
        (self.scanner_factory)()
    }
    
    /// リトライ機能付きで接続を試行
    async fn connect_with_retry(
        &self,
        scanner: &dyn Scanner,
        device_info: &DeviceInfo,
    ) -> Result<Box<dyn Connection>> {
        const MAX_RETRIES: u32 = 3;
        const RETRY_DELAY_MS: u64 = 2000;
        
        let mut last_error = None;
        
        for attempt in 1..=MAX_RETRIES {
            info!("Connection attempt {}/{} for device: {}", attempt, MAX_RETRIES, device_info.name);
            
            match scanner.connect(device_info).await {
                Ok(connection) => {
                    info!("Successfully connected to {} on attempt {}", device_info.name, attempt);
                    return Ok(connection);
                }
                Err(e) => {
                    last_error = Some(e);
                    warn!("Connection attempt {} failed for {}: {}", attempt, device_info.name, last_error.as_ref().unwrap());
                    
                    // 最後の試行でない場合はリトライ間隔を待つ
                    if attempt < MAX_RETRIES {
                        info!("Waiting {}ms before retry attempt {}...", RETRY_DELAY_MS, attempt + 1);
                        tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                    }
                }
            }
        }
        
        // 全ての試行が失敗した場合
        Err(last_error.unwrap_or_else(|| NotifError::Connection("Unknown connection error".to_string())))
    }
    
    async fn scan_and_connect_all(&self) -> Result<Vec<String>> {
        info!("Scanning for all devices with prefix: {}", self.device_name_prefix);
        
        let scanner = self.create_scanner()?;
        let devices = scanner.scan(
            &self.device_name_prefix,
            std::time::Duration::from_secs(10),
        ).await?;
        
        let mut connected_devices = Vec::new();
        
        for device_info in devices {
            // プレフィックスでフィルター
            if !device_info.name.starts_with(&self.device_name_prefix) {
                continue;
            }
            
            // 既に接続済みかチェック
            {
                let connections = self.connections.read().await;
                if connections.contains_key(&device_info.name) {
                    info!("Device {} is already connected", device_info.name);
                    connected_devices.push(device_info.name.clone());
                    continue;
                }
            }
            
            info!("Attempting to connect to device: {}", device_info.name);
            
            // リトライ機能付き接続
            match self.connect_with_retry(&*scanner, &device_info).await {
                Ok(connection) => {
                    let device_name = device_info.name.clone();
                    self.add_device(device_name.clone(), connection).await?;
                    connected_devices.push(device_name);
                }
                Err(e) => {
                    error!("Failed to connect to {} after all retries: {}", device_info.name, e);
                }
            }
        }
        
        scanner.stop_scan().await?;
        
        info!("Connected to {} device(s)", connected_devices.len());
        
        // 接続されたデバイスがある場合、keepaliveタスクを開始
        // info!("Checking if keepalive should be started: devices = {}", connected_devices.len());  // Keepaliveログ抑制
        if !connected_devices.is_empty() {
            // info!("Starting keepalive task...");  // Keepaliveログ抑制
            self.start_keepalive_task();
            // info!("Started keepalive task (5 second interval) for {} devices", connected_devices.len());  // Keepaliveログ抑制
        } else {
            // info!("No devices connected, skipping keepalive task");  // Keepaliveログ抑制
        }
        
        Ok(connected_devices)
    }
    
    async fn send_command_to_device(
        &self,
        device_id: &str,
        command: Command,
    ) -> Result<()> {
        let start_time = Instant::now();
        
        let mut connections = self.connections.write().await;
        
        if let Some(connection) = connections.get_mut(device_id) {
            debug!("Sending command to device: {}", device_id);
            
            match connection.send_command(command.clone()).await {
                Ok(_) => {
                    let response_time = start_time.elapsed().as_millis() as u64;
                    self.update_statistics(true, response_time).await;
                    
                    // 送信成功時、最後のコマンドを保存
                    // v5修正: CMD_IMAGEは複数タイルに分割されるため保存しない（再接続時の問題を防ぐ）
                    match &command {
                        Command::Image { .. } => {
                            // 画像タイルは保存しない（128個のタイルが個別に送信されるため）
                        }
                        _ => {
                            let mut last_commands = self.last_commands.write().await;
                            last_commands.insert(device_id.to_string(), command);
                        }
                    }
                    
                    Ok(())
                }
                Err(e) => {
                    self.update_statistics(false, 0).await;
                    
                    // 自動再接続を試みる
                    if *self.auto_reconnect.read().await && !connection.is_connected().await {
                        warn!("Device {} disconnected, attempting reconnect...", device_id);
                        if let Err(reconnect_err) = connection.reconnect().await {
                            error!("Failed to reconnect to {}: {}", device_id, reconnect_err);
                        }
                    }
                    
                    Err(e)
                }
            }
        } else {
            self.update_statistics(false, 0).await;
            Err(NotifError::DeviceNotFound(device_id.to_string()))
        }
    }
    
    async fn send_command_to_all(&self, command: Command) -> Result<()> {
        let connections = self.connections.read().await;
        
        if connections.is_empty() {
            return Err(NotifError::DeviceNotConnected("No devices connected".to_string()));
        }
        
        // コネクションのリストを取得
        let device_ids: Vec<String> = connections.keys().cloned().collect();
        drop(connections); // ロックを解放
        
        // 各デバイスに順次送信
        let mut any_error = None;
        for device_id in device_ids {
            if let Err(e) = self.send_command_to_device(&device_id, command.clone()).await {
                if any_error.is_none() {
                    any_error = Some(e);
                }
            }
        }
        
        // エラーがあれば最初のエラーを返す
        if let Some(e) = any_error {
            return Err(e);
        }
        
        Ok(())
    }
    
    async fn send_command_by_number(
        &self,
        number: usize,
        command: Command,
    ) -> Result<()> {
        let device_order = self.device_order.read().await;
        
        if number == 0 || number > device_order.len() {
            return Err(NotifError::DeviceNotFound(format!("Device #{}", number)));
        }
        
        let device_name = &device_order[number - 1];
        self.send_command_to_device(device_name, command).await
    }
    
    async fn list_connected_devices(&self) -> Vec<DeviceInfo> {
        let connections = self.connections.read().await;
        let device_order = self.device_order.read().await;
        
        let mut devices = Vec::new();
        
        for (index, device_name) in device_order.iter().enumerate() {
            if let Some(connection) = connections.get(device_name) {
                let mut info = connection.get_device_info().await;
                info.number = Some(index + 1);
                devices.push(info);
            }
        }
        
        devices
    }
    
    async fn is_device_connected(&self, device_id: &str) -> bool {
        let connections = self.connections.read().await;
        
        if let Some(connection) = connections.get(device_id) {
            connection.is_connected().await
        } else {
            false
        }
    }
    
    async fn disconnect_device(&self, device_id: &str) -> Result<()> {
        self.remove_device(device_id).await
    }
    
    async fn disconnect_all(&self) -> Result<()> {
        let mut connections = self.connections.write().await;
        let mut device_order = self.device_order.write().await;
        
        for (device_name, mut connection) in connections.drain() {
            info!("Disconnecting device: {}", device_name);
            if let Err(e) = connection.disconnect().await {
                warn!("Failed to disconnect {}: {}", device_name, e);
            }
        }
        
        device_order.clear();
        
        Ok(())
    }
    
    async fn reconnect_device(&self, device_id: &str) -> Result<()> {
        let mut connections = self.connections.write().await;
        
        if let Some(connection) = connections.get_mut(device_id) {
            connection.reconnect().await
        } else {
            Err(NotifError::DeviceNotFound(device_id.to_string()))
        }
    }
    
    async fn set_auto_reconnect(&self, enabled: bool) -> Result<()> {
        *self.auto_reconnect.write().await = enabled;
        info!("Auto-reconnect set to: {}", enabled);
        Ok(())
    }
    
    async fn get_statistics(&self) -> DeviceStatistics {
        let stats = self.statistics.read().await;
        let connections = self.connections.read().await;
        
        let average_response_time_ms = if stats.command_count > 0 {
            stats.total_response_time_ms as f64 / stats.command_count as f64
        } else {
            0.0
        };
        
        DeviceStatistics {
            total_devices: connections.len(),
            connected_devices: connections.values()
                .filter(|conn| futures::executor::block_on(conn.is_connected()))
                .count(),
            total_commands_sent: stats.total_commands_sent,
            total_errors: stats.total_errors,
            average_response_time_ms,
            uptime_seconds: stats.start_time.elapsed().as_secs(),
        }
    }
    
    fn start_keepalive(&self) {
        // info!("Explicitly starting keepalive task (called from start_keepalive)");  // Keepaliveログ抑制
        self.start_keepalive_task();
    }
    
    async fn save_image_tiles(&self, device_id: &str, tiles: Vec<Command>) {
        let mut image_tiles = self.last_image_tiles.write().await;
        image_tiles.insert(device_id.to_string(), tiles);
        
        // 画像を保存したら通常のコマンドはクリア
        let mut last_commands = self.last_commands.write().await;
        last_commands.remove(device_id);
    }
    
    async fn get_device_name_by_number(&self, number: usize) -> Option<String> {
        let device_order = self.device_order.read().await;
        
        if number == 0 || number > device_order.len() {
            return None;
        }
        
        Some(device_order[number - 1].clone())
    }
}