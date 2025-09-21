//! Windows固有のBluetooth実装

use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use windows::{
    core::{GUID, HSTRING},
    Devices::Bluetooth::{
        Advertisement::{
            BluetoothLEAdvertisementWatcher, BluetoothLEScanningMode,
            BluetoothLEAdvertisementReceivedEventArgs,
        },
        BluetoothConnectionStatus, BluetoothLEDevice,
        GenericAttributeProfile::{
            GattCharacteristic, GattCommunicationStatus,
            GattWriteOption, GattDeviceService,
            GattClientCharacteristicConfigurationDescriptorValue,
        },
    },
    Foundation::{EventRegistrationToken, TypedEventHandler},
    Storage::Streams::{DataWriter, IBuffer},
};

use notif_common_v5::{
    Connection, DeviceCapabilities, DeviceInfo, NotifError, Result, Scanner,
    Command, protocol::uuid as protocol_uuid,
};

/// Windows Errorを NotifErrorに変換（v2スタイル）
fn windows_error_to_notif_error(err: windows::core::Error) -> NotifError {
    NotifError::Bluetooth(format!("Windows API error: {}", err.message()))
}

/// Windows固有データ
#[derive(Clone)]
pub struct WindowsPlatformData {
    device: BluetoothLEDevice,
}

impl Debug for WindowsPlatformData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsPlatformData")
            .field("device", &"<BluetoothLEDevice>")
            .finish()
    }
}

/// Windows Bluetooth接続
pub struct WindowsConnection {
    device: BluetoothLEDevice,
    device_info: DeviceInfo,
    command_char: GattCharacteristic,
    status_char: Option<GattCharacteristic>,
    service: GattDeviceService,
    // Connection Interval推定用のフィールド
    last_send_time: Option<Instant>,
    send_intervals: Vec<u64>,  // ミリ秒単位の送信間隔を記録
}

impl Debug for WindowsConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsConnection")
            .field("device_info", &self.device_info)
            .finish()
    }
}

impl WindowsConnection {
    /// 新しい接続を作成
    pub async fn new(device: BluetoothLEDevice, device_name: String) -> Result<Self> {
        Self::new_with_optimization(device, device_name, false).await
    }
    
    /// 新しい接続を作成（最適化オプション付き）
    pub async fn new_with_optimization(device: BluetoothLEDevice, device_name: String, optimize: bool) -> Result<Self> {
        info!("Creating connection for device: {}", device_name);
        
        // デバイス接続状態を確認
        let connection_status = device.ConnectionStatus().map_err(windows_error_to_notif_error)?;
        info!("Device connection status: {:?}", connection_status);
        
        if connection_status != BluetoothConnectionStatus::Connected {
            info!("Device not connected, attempting to trigger connection...");
            
            // 接続をトリガーするためにGATTサービスを要求（v2と同じ方法）
            let _ = device.GetGattServicesAsync()
                .map_err(windows_error_to_notif_error)?
                .get()
                .map_err(windows_error_to_notif_error)?;
            
            // 少し待ってから再度接続状態を確認
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            let new_status = device.ConnectionStatus().map_err(windows_error_to_notif_error)?;
            info!("Device connection status after retry: {:?}", new_status);
            
            if new_status != BluetoothConnectionStatus::Connected {
                return Err(NotifError::Bluetooth("Device not connected after connection attempt".to_string()));
            }
        }
        
        // サービスUUID
        let service_uuid = parse_guid(protocol_uuid::SERVICE)?;
        info!("Looking for service UUID: {}", protocol_uuid::SERVICE);
        
        // GATTサービスの取得
        let services_result = device.GetGattServicesAsync()
            .map_err(windows_error_to_notif_error)?
            .get()
            .map_err(windows_error_to_notif_error)?;
        
        let status = services_result.Status().map_err(windows_error_to_notif_error)?;
        info!("GATT services status: {:?}", status);
        
        if status != GattCommunicationStatus::Success {
            return Err(NotifError::Bluetooth(format!("Failed to get GATT services: {:?}", status)));
        }
        
        let services = services_result.Services().map_err(windows_error_to_notif_error)?;
        let service_count = services.Size().map_err(windows_error_to_notif_error)?;
        info!("Found {} GATT services", service_count);
        
        // すべてのサービスをログ出力してデバッグ
        for i in 0..service_count {
            let service = services.GetAt(i).map_err(windows_error_to_notif_error)?;
            let uuid = service.Uuid().map_err(windows_error_to_notif_error)?;
            debug!("Service {}: {}", i, format_guid(&uuid));
        }
        
        let mut target_service = None;
        
        for i in 0..service_count {
            let service = services.GetAt(i).map_err(windows_error_to_notif_error)?;
            let uuid = service.Uuid().map_err(windows_error_to_notif_error)?;
            if uuid == service_uuid {
                info!("Found target service at index {}", i);
                target_service = Some(service);
                break;
            }
        }
        
        let service = target_service
            .ok_or_else(|| NotifError::Bluetooth(format!("Service {} not found", protocol_uuid::SERVICE)))?;
        
        // キャラクタリスティックの取得
        let command_uuid = parse_guid(protocol_uuid::COMMAND_CHAR)?;
        let status_uuid = parse_guid(protocol_uuid::STATUS_CHAR)?;
        
        let chars_result = service.GetCharacteristicsAsync()
            .map_err(windows_error_to_notif_error)?
            .get()
            .map_err(windows_error_to_notif_error)?;
        
        if chars_result.Status().map_err(windows_error_to_notif_error)? != GattCommunicationStatus::Success {
            return Err(NotifError::Bluetooth("Failed to get characteristics".to_string()));
        }
        
        let chars = chars_result.Characteristics().map_err(windows_error_to_notif_error)?;
        let mut command_char = None;
        let mut status_char = None;
        
        for i in 0..chars.Size().map_err(windows_error_to_notif_error)? {
            let char = chars.GetAt(i).map_err(windows_error_to_notif_error)?;
            let char_uuid = char.Uuid().map_err(windows_error_to_notif_error)?;
            
            if char_uuid == command_uuid {
                command_char = Some(char);
            } else if char_uuid == status_uuid {
                status_char = Some(char);
            }
        }
        
        let command_char = command_char
            .ok_or_else(|| NotifError::Bluetooth("Command characteristic not found".to_string()))?;
        let status_char = status_char
            .ok_or_else(|| NotifError::Bluetooth("Status characteristic not found".to_string()))?;
        
        info!("Found required characteristics for device: {}", device_name);
        
        // デバイス接続情報をログ出力
        if let Ok(address) = device.BluetoothAddress() {
            info!("Device address: {:016X}", address);
        }
        if let Ok(status) = device.ConnectionStatus() {
            info!("Connection status: {:?}", status);
        }
        
        // ステータス通知を有効化（v2互換）
        let cccd_value = GattClientCharacteristicConfigurationDescriptorValue::Notify;
        let write_result = status_char.WriteClientCharacteristicConfigurationDescriptorAsync(cccd_value)
            .map_err(windows_error_to_notif_error)?
            .get()
            .map_err(windows_error_to_notif_error)?;
        
        if write_result != GattCommunicationStatus::Success {
            return Err(NotifError::Bluetooth("Failed to enable notifications".to_string()));
        }
        
        // デバイス情報を作成
        let device_info = DeviceInfo {
            name: device_name,
            address: format!("{:016X}", device.BluetoothAddress().map_err(windows_error_to_notif_error)?),
            connected: device.ConnectionStatus().map_err(windows_error_to_notif_error)? == BluetoothConnectionStatus::Connected,
            number: None,
            signal_strength: None,
            battery_level: None,
            capabilities: DeviceCapabilities::default(),
        };
        
        let mut connection = WindowsConnection {
            device,
            device_info,
            command_char,
            status_char: Some(status_char),
            service,
            last_send_time: None,
            send_intervals: Vec::new(),
        };
        
        // 接続最適化が有効な場合
        if optimize {
            info!("接続速度の最適化を開始: {}", device_name);
            if let Err(e) = connection.optimize_connection_speed().await {
                warn!("接続最適化に失敗しましたが、接続は継続します: {}", e);
            }
        }
        
        Ok(connection)
    }
}

#[async_trait]
impl Connection for WindowsConnection {
    async fn send_command(&mut self, command: Command) -> Result<()> {
        let data = command.encode();
        
        // Connection Interval推定用の送信時刻記録
        let now = Instant::now();
        if let Some(last_time) = self.last_send_time {
            let interval_ms = now.duration_since(last_time).as_millis() as u64;
            if interval_ms < 2000 {  // 2秒以内の間隔のみ記録
                self.send_intervals.push(interval_ms);
                
                // 10サンプル毎にConnection Intervalを推定
                if self.send_intervals.len() >= 10 {
                    let avg_interval = self.send_intervals.iter().sum::<u64>() / self.send_intervals.len() as u64;
                    info!("[接続パラメータ推定] デバイス: {} - 平均送信間隔: {}ms (推定Connection Interval: {}-{}ms)", 
                          self.device_info.name, avg_interval, 
                          avg_interval / 2, avg_interval);
                    self.send_intervals.clear();
                }
            }
        }
        self.last_send_time = Some(now);
        
        info!("Sending command to Windows device: {} bytes", data.len());
        
        // デバッグ用：送信データの16進ダンプ
        let hex_data: String = data.iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");
        debug!("Command data (hex): {}", hex_data);
        
        // 送信サイズをログに記録（MTU調査用）
        if data.len() > 20 {
            info!("Large packet transmission: {} bytes to device {}", data.len(), self.device_info.name);
        }
        
        // DataWriterを使用してデータをIBufferに変換
        let writer = DataWriter::new().map_err(windows_error_to_notif_error)?;
        writer.WriteBytes(&data).map_err(windows_error_to_notif_error)?;
        let buffer = writer.DetachBuffer().map_err(windows_error_to_notif_error)?;
        
        // BLE MTUを考慮（512バイト以下） - v2互換の書き込みメソッドを使用
        let write_result = self.command_char
            .WriteValueWithResultAsync(&buffer)
            .map_err(windows_error_to_notif_error)?
            .get()
            .map_err(windows_error_to_notif_error)?;
        
        let status = write_result.Status().map_err(windows_error_to_notif_error)?;
        
        // 送信結果の詳細をログ出力
        if data.len() > 100 {
            info!("Write completed for {} bytes, status: {:?}, device: {}", 
                 data.len(), status, self.device_info.name);
        } else {
            debug!("Write result status: {:?}", status);
        }
        
        if status != GattCommunicationStatus::Success {
            return Err(NotifError::Bluetooth(format!(
                "Failed to write command: {:?}", status
            )));
        }
        
        // ステータス特性がある場合、応答を確認（デバッグ用）
        if let Some(ref status_char) = self.status_char {
            // ステータスを読み取ってみる（非同期で待たない）
            if let Ok(read_result) = status_char.ReadValueAsync() {
                if let Ok(result) = read_result.get() {
                    if result.Status().map_err(windows_error_to_notif_error)? == GattCommunicationStatus::Success {
                        if let Ok(value) = result.Value() {
                            if let Ok(reader) = windows::Storage::Streams::DataReader::FromBuffer(&value) {
                                if let Ok(length) = reader.UnconsumedBufferLength() {
                                    if length > 0 {
                                        if let Ok(status_byte) = reader.ReadByte() {
                                            debug!("Device status after command: 0x{:02X}", status_byte);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn is_connected(&self) -> bool {
        self.device.ConnectionStatus()
            .map(|s| s == BluetoothConnectionStatus::Connected)
            .unwrap_or(false)
    }
    
    async fn get_device_info(&self) -> DeviceInfo {
        let mut info = self.device_info.clone();
        info.connected = self.is_connected().await;
        info
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        // Windows APIではデバイスのClose()メソッドを呼ぶ
        self.device.Close().map_err(windows_error_to_notif_error)?;
        self.device_info.connected = false;
        Ok(())
    }
    
    async fn reconnect(&mut self) -> Result<()> {
        if !self.is_connected().await {
            info!("Reconnecting to device: {}", self.device_info.name);
            
            // Windowsでは再接続は新しいデバイスインスタンスを取得する必要がある
            let address = self.device.BluetoothAddress().map_err(windows_error_to_notif_error)?;
            
            // デバイスを再取得
            let device = BluetoothLEDevice::FromBluetoothAddressAsync(address)
                .map_err(windows_error_to_notif_error)?
                .get()
                .map_err(windows_error_to_notif_error)?;
            
            self.device = device;
            
            // サービスを再取得
            let service_uuid = parse_guid(protocol_uuid::SERVICE)?;
            let services_result = self.device.GetGattServicesAsync()
                .map_err(windows_error_to_notif_error)?
                .get()
                .map_err(windows_error_to_notif_error)?;
            
            if services_result.Status().map_err(windows_error_to_notif_error)? != GattCommunicationStatus::Success {
                return Err(NotifError::Bluetooth("Failed to get GATT services on reconnect".to_string()));
            }
            
            let services = services_result.Services().map_err(windows_error_to_notif_error)?;
            let mut found_service = false;
            
            for i in 0..services.Size().map_err(windows_error_to_notif_error)? {
                let service = services.GetAt(i).map_err(windows_error_to_notif_error)?;
                if service.Uuid().map_err(windows_error_to_notif_error)? == service_uuid {
                    self.service = service;
                    found_service = true;
                    break;
                }
            }
            
            if !found_service {
                return Err(NotifError::Bluetooth("Service not found on reconnect".to_string()));
            }
            
            // キャラクタリスティックも再取得する必要がある
            let command_uuid = parse_guid(protocol_uuid::COMMAND_CHAR)?;
            let status_uuid = parse_guid(protocol_uuid::STATUS_CHAR)?;
            
            let chars_result = self.service.GetCharacteristicsAsync()
                .map_err(windows_error_to_notif_error)?
                .get()
                .map_err(windows_error_to_notif_error)?;
            
            if chars_result.Status().map_err(windows_error_to_notif_error)? != GattCommunicationStatus::Success {
                return Err(NotifError::Bluetooth("Failed to get characteristics on reconnect".to_string()));
            }
            
            let chars = chars_result.Characteristics().map_err(windows_error_to_notif_error)?;
            let mut command_char = None;
            let mut status_char = None;
            
            for i in 0..chars.Size().map_err(windows_error_to_notif_error)? {
                let char = chars.GetAt(i).map_err(windows_error_to_notif_error)?;
                let char_uuid = char.Uuid().map_err(windows_error_to_notif_error)?;
                
                if char_uuid == command_uuid {
                    command_char = Some(char);
                } else if char_uuid == status_uuid {
                    status_char = Some(char);
                }
            }
            
            // コマンドキャラクタリスティックを更新
            if let Some(cmd_char) = command_char {
                self.command_char = cmd_char;
            } else {
                return Err(NotifError::Bluetooth("Command characteristic not found on reconnect".to_string()));
            }
            
            // ステータスキャラクタリスティックを更新（存在する場合）
            if let Some(stat_char) = status_char {
                // ステータス通知を再度有効化
                let cccd_value = GattClientCharacteristicConfigurationDescriptorValue::Notify;
                let write_result = stat_char.WriteClientCharacteristicConfigurationDescriptorAsync(cccd_value)
                    .map_err(windows_error_to_notif_error)?
                    .get()
                    .map_err(windows_error_to_notif_error)?;
                
                if write_result != GattCommunicationStatus::Success {
                    warn!("Failed to re-enable notifications on reconnect");
                }
                
                self.status_char = Some(stat_char);
            }
            
            self.device_info.connected = true;
            info!("Successfully reconnected to device: {}", self.device_info.name);
        }
        Ok(())
    }
    
    async fn get_battery_level(&self) -> Option<u8> {
        // TODO: Battery Service (0x180F) から読み取り
        None
    }
    
    async fn get_signal_strength(&self) -> Option<i8> {
        // Windows APIではRSSI取得が直接サポートされていない
        None
    }
    
    /// 接続速度を測定して最適化を試みる
    async fn optimize_connection_speed(&mut self) -> Result<()> {
        info!("接続速度を測定中: {}", self.device_info.name);
        
        // テストコマンドを5回送信して平均速度を測定
        let test_start = Instant::now();
        let test_command = Command::Clear;
        
        for i in 0..5 {
            if let Err(e) = self.send_command(test_command.clone()).await {
                warn!("テストコマンド {}の送信失敗: {}", i + 1, e);
                return Ok(());  // 最適化を中止するがエラーにはしない
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        let elapsed = test_start.elapsed().as_millis() as u64;
        let avg_interval = elapsed / 5;
        
        info!("現在の平均送信間隔: {}ms (デバイス: {})", avg_interval, self.device_info.name);
        
        // 300ms以上の場合は遅いと判断して再接続を試行
        if avg_interval > 300 {
            info!("遅い接続を検出 ({}ms)。再接続による最適化を試行します...", avg_interval);
            
            // デバイスを一旦切断
            self.device.Close().map_err(windows_error_to_notif_error)?;
            tokio::time::sleep(Duration::from_millis(2000)).await;
            
            // デバイスの再検出と再接続
            info!("デバイスに再接続中: {}", self.device_info.name);
            let address = self.device.BluetoothAddress().map_err(windows_error_to_notif_error)?;
            
            // デバイスを再取得
            let device = BluetoothLEDevice::FromBluetoothAddressAsync(address)
                .map_err(windows_error_to_notif_error)?
                .get()
                .map_err(windows_error_to_notif_error)?;
            
            self.device = device;
            
            // GATTサービスの再取得
            let service_uuid = parse_guid(protocol_uuid::SERVICE)?;
            let services_result = self.device.GetGattServicesForUuidAsync(&service_uuid)
                .map_err(windows_error_to_notif_error)?
                .get()
                .map_err(windows_error_to_notif_error)?;
            
            if services_result.Status().map_err(windows_error_to_notif_error)? != GattCommunicationStatus::Success {
                warn!("再接続時のサービス取得に失敗しました");
                return Ok(());
            }
            
            let services = services_result.Services().map_err(windows_error_to_notif_error)?;
            if services.Size().map_err(windows_error_to_notif_error)? == 0 {
                warn!("再接続時にサービスが見つかりませんでした");
                return Ok(());
            }
            
            // 新しいサービスとCharacteristicを設定
            let service = services.GetAt(0).map_err(windows_error_to_notif_error)?;
            let command_uuid = parse_guid(protocol_uuid::CHAR_COMMAND)?;
            let chars_result = service.GetCharacteristicsForUuidAsync(&command_uuid)
                .map_err(windows_error_to_notif_error)?
                .get()
                .map_err(windows_error_to_notif_error)?;
            
            if chars_result.Status().map_err(windows_error_to_notif_error)? == GattCommunicationStatus::Success {
                let chars = chars_result.Characteristics().map_err(windows_error_to_notif_error)?;
                if chars.Size().map_err(windows_error_to_notif_error)? > 0 {
                    self.command_char = chars.GetAt(0).map_err(windows_error_to_notif_error)?;
                    self.service = service;
                    
                    // 再接続後の速度を測定
                    let retest_start = Instant::now();
                    for _ in 0..5 {
                        let _ = self.send_command(test_command.clone()).await;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    let new_elapsed = retest_start.elapsed().as_millis() as u64;
                    let new_avg_interval = new_elapsed / 5;
                    
                    if new_avg_interval < avg_interval {
                        info!("接続最適化成功! {}ms → {}ms ({}%改善)", 
                              avg_interval, new_avg_interval, 
                              ((avg_interval - new_avg_interval) * 100 / avg_interval));
                    } else {
                        info!("再接続後も速度は変わりませんでした: {}ms", new_avg_interval);
                    }
                }
            }
        } else {
            info!("接続速度は良好です: {}ms", avg_interval);
        }
        
        // send_intervalsをクリア（最適化後は新しい計測を開始）
        self.send_intervals.clear();
        
        Ok(())
    }
}

/// Windows Bluetoothスキャナー
pub struct WindowsScanner {
    watcher: Arc<Mutex<BluetoothLEAdvertisementWatcher>>,
    found_devices: Arc<Mutex<Vec<DeviceInfo>>>,
}

impl WindowsScanner {
    /// 新しいスキャナーを作成
    pub async fn new() -> Result<Self> {
        let watcher = BluetoothLEAdvertisementWatcher::new()
            .map_err(windows_error_to_notif_error)?;
        
        // スキャン設定
        watcher.SetScanningMode(BluetoothLEScanningMode::Active)
            .map_err(windows_error_to_notif_error)?;
        
        info!("Windows Bluetooth scanner initialized");
        
        Ok(WindowsScanner {
            watcher: Arc::new(Mutex::new(watcher)),
            found_devices: Arc::new(Mutex::new(Vec::new())),
        })
    }
    
    /// デバイス検出ハンドラー
    async fn on_device_found(
        &self,
        args: &BluetoothLEAdvertisementReceivedEventArgs,
        prefix: String,
    ) -> windows::core::Result<()> {
        let address = args.BluetoothAddress()?;
        let advertisement = args.Advertisement()?;
        
        // ローカル名を取得
        if let Ok(local_name_section) = advertisement.LocalName() {
            let name = local_name_section.to_string();
            
            if name.starts_with(&prefix) {
                let mut devices = self.found_devices.lock().await;
                
                // 既に登録済みでないか確認
                if !devices.iter().any(|d| d.name == name) {
                    info!("Found device: {} at {:016X}", name, address);
                    
                    devices.push(DeviceInfo {
                        name: name.clone(),
                        address: format!("{:016X}", address),
                        connected: false,
                        number: None,
                        signal_strength: Some(args.RawSignalStrengthInDBm()? as i8),
                        battery_level: None,
                        capabilities: DeviceCapabilities::default(),
                    });
                }
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl Scanner for WindowsScanner {
    async fn scan(
        &self,
        prefix: &str,
        timeout: Duration,
    ) -> Result<Vec<DeviceInfo>> {
        // スキャン実装をトークナイズしてSendイシューを避ける
        let result: Result<(windows::Foundation::EventRegistrationToken, _)> = {
            let watcher = self.watcher.lock().await;
            let found_devices = self.found_devices.clone();
            let prefix = prefix.to_string();
            
            // デバイスリストをクリア
            found_devices.lock().await.clear();
            
            // イベントハンドラーを設定
            let found_devices_for_handler = found_devices.clone();
            let prefix_for_handler = prefix.clone();
            
            let handler = TypedEventHandler::new(move |_, args: &Option<BluetoothLEAdvertisementReceivedEventArgs>| {
                if let Some(args) = args {
                    let devices = found_devices_for_handler.clone();
                    let prefix = prefix_for_handler.clone();
                    
                    // 同期的にデバイスリストに追加（v2スタイル）
                    let address = args.BluetoothAddress().unwrap_or(0);
                    if let Ok(advertisement) = args.Advertisement() {
                        if let Ok(local_name) = advertisement.LocalName() {
                            let name = local_name.to_string();
                            if name.starts_with(&prefix) {
                                if let Ok(mut devices) = devices.try_lock() {
                                    if !devices.iter().any(|d| d.name == name) {
                                        devices.push(DeviceInfo {
                                            name: name.clone(),
                                            address: format!("{:016X}", address),
                                            connected: false,
                                            number: None,
                                            signal_strength: args.RawSignalStrengthInDBm().ok().map(|r| r as i8),
                                            battery_level: None,
                                            capabilities: DeviceCapabilities::default(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(())
            });
            
            let token = watcher.Received(&handler).map_err(windows_error_to_notif_error)?;
            
            // スキャン開始
            watcher.Start().map_err(windows_error_to_notif_error)?;
            info!("Scanning for devices with prefix: {}", prefix);
            
            Ok((token, found_devices))
        };
        
        let (token, found_devices) = result?;
        
        // タイムアウトまで待機
        tokio::time::sleep(timeout).await;
        
        // スキャン停止
        {
            let watcher = self.watcher.lock().await;
            watcher.Stop().map_err(windows_error_to_notif_error)?;
            watcher.RemoveReceived(token).map_err(windows_error_to_notif_error)?;
        }
        
        let devices = found_devices.lock().await.clone();
        info!("Scan complete. Found {} devices", devices.len());
        
        Ok(devices)
    }
    
    async fn scan_for_device(
        &self,
        device_name: &str,
        timeout: Duration,
    ) -> Result<Option<DeviceInfo>> {
        let devices = self.scan("", timeout).await?;
        Ok(devices.into_iter().find(|d| d.name == device_name))
    }
    
    async fn connect(&self, device_info: &DeviceInfo) -> Result<Box<dyn Connection>> {
        // アドレスからBluetoothLEDeviceを取得
        let address = u64::from_str_radix(&device_info.address, 16)
            .map_err(|e| NotifError::Bluetooth(format!("Invalid address format: {}", e)))?;
        
        info!("Connecting to device: {} at {:016X}", device_info.name, address);
        
        let device = BluetoothLEDevice::FromBluetoothAddressAsync(address)
            .map_err(windows_error_to_notif_error)?
            .get()
            .map_err(windows_error_to_notif_error)?;
        
        // 接続オブジェクトを作成
        let connection = WindowsConnection::new(device, device_info.name.clone()).await?;
        
        Ok(Box::new(connection))
    }
    
    async fn stop_scan(&self) -> Result<()> {
        let watcher = self.watcher.lock().await;
        watcher.Stop().map_err(windows_error_to_notif_error)?;
        Ok(())
    }
}

/// GUIDをパース
fn parse_guid(uuid_str: &str) -> Result<GUID> {
    let uuid = Uuid::parse_str(uuid_str)
        .map_err(|e| NotifError::Bluetooth(format!("Invalid UUID: {}", e)))?;
    
    let bytes = uuid.as_bytes();
    Ok(GUID {
        data1: u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        data2: u16::from_be_bytes([bytes[4], bytes[5]]),
        data3: u16::from_be_bytes([bytes[6], bytes[7]]),
        data4: [bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]],
    })
}

/// GUIDを文字列にフォーマット
fn format_guid(guid: &GUID) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0], guid.data4[1],
        guid.data4[2], guid.data4[3], guid.data4[4], guid.data4[5], guid.data4[6], guid.data4[7]
    )
}

/// Windows Bluetoothマネージャーファクトリー
pub async fn create_bluetooth_manager() -> Result<notif_common_v5::CommonBluetoothManager> {
    let device_prefix = std::env::var("DEVICE_NAME_PREFIX")
        .unwrap_or_else(|_| "notif_atoms3".to_string());
    
    let manager = notif_common_v5::CommonBluetoothManager::new(
        device_prefix,
        || {
            // Scannerを非同期で作成するためのラッパー
            let scanner = futures::executor::block_on(WindowsScanner::new())?;
            Ok(Box::new(scanner) as Box<dyn Scanner>)
        },
    );
    
    Ok(manager)
}