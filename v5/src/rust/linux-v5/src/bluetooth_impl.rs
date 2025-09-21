//! Linux固有のBluetooth実装

use async_trait::async_trait;
use btleplug::api::{Central, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use notif_common_v5::{
    Connection, DeviceCapabilities, DeviceInfo, NotifError, Result, Scanner,
    Command, protocol::uuid as protocol_uuid,
};

/// Linux固有データ
#[derive(Clone)]
pub struct LinuxPlatformData {
    peripheral: Peripheral,
}

impl Debug for LinuxPlatformData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinuxPlatformData")
            .field("peripheral", &"<btleplug::Peripheral>")
            .finish()
    }
}

/// Linux Bluetooth接続
pub struct LinuxConnection {
    peripheral: Peripheral,
    device_info: DeviceInfo,
    command_char: Characteristic,
    status_char: Option<Characteristic>,
}

impl Debug for LinuxConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinuxConnection")
            .field("device_info", &self.device_info)
            .finish()
    }
}

impl LinuxConnection {
    /// 新しい接続を作成
    pub async fn new(peripheral: Peripheral, device_name: String) -> Result<Self> {
        // 注: Linux版btleplugではMTU設定が自動的に行われます
        // デフォルトでは通常512バイト以上のMTUが確保されます
        info!("Creating connection for device: {}", device_name);
        
        // サービスUUID
        let service_uuid = Uuid::parse_str(protocol_uuid::SERVICE)
            .map_err(|e| NotifError::Bluetooth(format!("Invalid service UUID: {}", e)))?;
        
        // キャラクタリスティックUUID
        let command_uuid = Uuid::parse_str(protocol_uuid::COMMAND_CHAR)
            .map_err(|e| NotifError::Bluetooth(format!("Invalid command UUID: {}", e)))?;
        let status_uuid = Uuid::parse_str(protocol_uuid::STATUS_CHAR)
            .map_err(|e| NotifError::Bluetooth(format!("Invalid status UUID: {}", e)))?;
        
        // サービスの発見
        peripheral.discover_services().await
            .map_err(|e| NotifError::Bluetooth(format!("Failed to discover services: {}", e)))?;
        
        // サービスを検索
        let services = peripheral.services();
        let service = services.iter()
            .find(|s| s.uuid == service_uuid)
            .ok_or_else(|| NotifError::Bluetooth(format!("Service {} not found", service_uuid)))?;
        
        // キャラクタリスティックを検索
        let command_char = service.characteristics.iter()
            .find(|c| c.uuid == command_uuid)
            .ok_or_else(|| NotifError::Bluetooth("Command characteristic not found".to_string()))?
            .clone();
        
        let status_char = service.characteristics.iter()
            .find(|c| c.uuid == status_uuid)
            .cloned();
        
        // ステータス通知を有効化（可能な場合）
        if let Some(ref char) = status_char {
            let _ = peripheral.subscribe(char).await;
        }
        
        // デバイス情報を作成
        let properties = peripheral.properties().await
            .map_err(|e| NotifError::Bluetooth(format!("Failed to get properties: {}", e)))?
            .unwrap_or_default();
        
        let device_info = DeviceInfo {
            name: device_name,
            address: properties.address.to_string(),
            connected: true,
            number: None,
            signal_strength: properties.rssi.map(|r| r as i8),
            battery_level: None,
            capabilities: DeviceCapabilities::default(),
        };
        
        Ok(LinuxConnection {
            peripheral,
            device_info,
            command_char,
            status_char,
        })
    }
}

#[async_trait]
impl Connection for LinuxConnection {
    async fn send_command(&mut self, command: Command) -> Result<()> {
        let data = command.encode();
        
        // デバッグ: コマンドタイプと最初の数バイトを表示
        if data.len() >= 3 {
            let cmd_type = data[0];
            let payload_len = data[1] as u16 | ((data[2] as u16) << 8);
            debug!("Sending command to Linux device: type=0x{:02X}, payload_len={}, total_bytes={}", 
                  cmd_type, payload_len, data.len());
            
            // 画像タイルの場合のみログ出力（0x06 = CMD_IMAGE）
            if cmd_type == 0x06 {
                debug!("Sending image tile: {} bytes", data.len());
            }
        }
        
        // WriteType選択: パフォーマンスのためWithoutResponseを使用
        // ただし、重要なコマンドや最後のチャンクはWithResponseを使用
        let write_type = if data.len() >= 3 && data[0] == 0x06 {
            // 画像タイルはWithoutResponseで高速送信
            WriteType::WithoutResponse
        } else {
            // その他のコマンドはWithResponseで確実性を保つ
            WriteType::WithResponse
        };
        
        // BLE MTUを考慮（512バイト以下）
        if data.len() > 512 {
            // チャンク分割送信
            let chunks: Vec<_> = data.chunks(512).collect();
            let last_index = chunks.len() - 1;
            
            for (index, chunk) in chunks.iter().enumerate() {
                // 最後のチャンクはWithResponseで確実性を保つ
                let chunk_write_type = if index == last_index {
                    WriteType::WithResponse
                } else {
                    write_type
                };
                
                self.peripheral
                    .write(&self.command_char, chunk, chunk_write_type)
                    .await
                    .map_err(|e| NotifError::Bluetooth(format!("Write failed: {}", e)))?;
            }
        } else {
            self.peripheral
                .write(&self.command_char, &data, write_type)
                .await
                .map_err(|e| NotifError::Bluetooth(format!("Write failed: {}", e)))?;
        }
        
        Ok(())
    }
    
    async fn is_connected(&self) -> bool {
        self.peripheral.is_connected().await.unwrap_or(false)
    }
    
    async fn get_device_info(&self) -> DeviceInfo {
        let mut info = self.device_info.clone();
        info.connected = self.is_connected().await;
        
        // 最新のRSSIを取得
        if let Ok(Some(props)) = self.peripheral.properties().await {
            info.signal_strength = props.rssi.map(|r| r as i8);
        }
        
        info
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        self.peripheral.disconnect().await
            .map_err(|e| NotifError::Bluetooth(format!("Disconnect failed: {}", e)))?;
        self.device_info.connected = false;
        Ok(())
    }
    
    async fn reconnect(&mut self) -> Result<()> {
        if !self.is_connected().await {
            self.peripheral.connect().await
                .map_err(|e| NotifError::Bluetooth(format!("Reconnect failed: {}", e)))?;
            
            // サービスを再発見
            self.peripheral.discover_services().await
                .map_err(|e| NotifError::Bluetooth(format!("Service rediscovery failed: {}", e)))?;
            
            self.device_info.connected = true;
        }
        Ok(())
    }
    
    async fn get_battery_level(&self) -> Option<u8> {
        // TODO: Battery Service (0x180F) から読み取り
        None
    }
    
    async fn get_signal_strength(&self) -> Option<i8> {
        if let Ok(Some(props)) = self.peripheral.properties().await {
            props.rssi.map(|r| r as i8)
        } else {
            None
        }
    }
}

/// Linux Bluetoothスキャナー
#[derive(Clone)]
pub struct LinuxScanner {
    adapter: Arc<Adapter>,
    scanning: Arc<Mutex<bool>>,
}

impl LinuxScanner {
    /// 新しいスキャナーを作成
    pub async fn new() -> Result<Self> {
        info!("Initializing Linux Bluetooth scanner...");
        
        info!("Creating Bluetooth manager...");
        let manager = Manager::new().await
            .map_err(|e| {
                error!("Failed to create BT manager: {}", e);
                NotifError::Bluetooth(format!("Failed to create BT manager: {}", e))
            })?;
        
        info!("Getting Bluetooth adapters...");
        let adapters = manager.adapters().await
            .map_err(|e| {
                error!("Failed to get adapters: {}", e);
                NotifError::Bluetooth(format!("Failed to get adapters: {}", e))
            })?;
        
        let adapter = adapters.into_iter().next()
            .ok_or_else(|| {
                error!("No Bluetooth adapter found");
                NotifError::Bluetooth("No Bluetooth adapter found".to_string())
            })?;
        
        // アダプタの情報を取得
        if let Ok(info) = adapter.adapter_info().await {
            info!("Bluetooth adapter initialized successfully: {:?}", info);
        } else {
            info!("Bluetooth adapter initialized (info unavailable)");
        }
        
        Ok(LinuxScanner {
            adapter: Arc::new(adapter),
            scanning: Arc::new(Mutex::new(false)),
        })
    }
}

#[async_trait]
impl Scanner for LinuxScanner {
    async fn scan(
        &self,
        prefix: &str,
        timeout: Duration,
    ) -> Result<Vec<DeviceInfo>> {
        let mut scanning = self.scanning.lock().await;
        if *scanning {
            return Err(NotifError::Bluetooth("Scan already in progress".to_string()));
        }
        *scanning = true;
        drop(scanning);
        
        // スキャン開始
        info!("Attempting to start scan with adapter...");
        match self.adapter.start_scan(ScanFilter::default()).await {
            Ok(_) => {
                info!("Scan started successfully");
            }
            Err(e) => {
                error!("Failed to start scan: {:?}", e);
                *self.scanning.lock().await = false;
                return Err(NotifError::Bluetooth(format!("Failed to start scan: {}", e)));
            }
        }
        
        info!("Scanning for devices with prefix: {}", prefix);
        
        let mut found_devices = Vec::new();
        let start = tokio::time::Instant::now();
        
        while start.elapsed() < timeout {
            let peripherals = self.adapter.peripherals().await
                .map_err(|e| NotifError::Bluetooth(format!("Failed to get peripherals: {}", e)))?;
            
            for peripheral in peripherals {
                if let Ok(Some(properties)) = peripheral.properties().await {
                    if let Some(name) = &properties.local_name {
                        if name.starts_with(prefix) && !found_devices.iter().any(|d: &DeviceInfo| d.name == *name) {
                            info!("Found device: {} at {}", name, properties.address);
                            
                            found_devices.push(DeviceInfo {
                                name: name.clone(),
                                address: properties.address.to_string(),
                                connected: false,
                                number: None,
                                signal_strength: properties.rssi.map(|r| r as i8),
                                battery_level: None,
                                capabilities: DeviceCapabilities::default(),
                            });
                        }
                    }
                }
            }
            
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        
        // スキャン停止
        let _ = self.adapter.stop_scan().await;
        *self.scanning.lock().await = false;
        
        info!("Scan complete. Found {} devices", found_devices.len());
        Ok(found_devices)
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
        // アドレスからPeripheralを検索
        let peripherals = self.adapter.peripherals().await
            .map_err(|e| NotifError::Bluetooth(format!("Failed to get peripherals: {}", e)))?;
        
        for peripheral in peripherals {
            if let Ok(Some(props)) = peripheral.properties().await {
                if props.address.to_string() == device_info.address {
                    info!("Connecting to device: {}", device_info.name);
                    
                    // 接続
                    peripheral.connect().await
                        .map_err(|e| NotifError::Bluetooth(format!("Connection failed: {}", e)))?;
                    
                    // 接続オブジェクトを作成
                    let connection = LinuxConnection::new(peripheral, device_info.name.clone()).await?;
                    
                    return Ok(Box::new(connection));
                }
            }
        }
        
        Err(NotifError::DeviceNotFound(device_info.name.clone()))
    }
    
    async fn stop_scan(&self) -> Result<()> {
        let _ = self.adapter.stop_scan().await;
        *self.scanning.lock().await = false;
        Ok(())
    }
}

/// Linux Bluetoothマネージャーファクトリー
pub async fn create_bluetooth_manager() -> Result<notif_common_v5::CommonBluetoothManager> {
    info!("Creating Linux Bluetooth manager factory...");
    let device_prefix = std::env::var("DEVICE_NAME_PREFIX")
        .unwrap_or_else(|_| "notif_atoms3".to_string());
    info!("Device prefix: {}", device_prefix);
    
    // スキャナーを事前に作成
    info!("Creating Linux scanner...");
    let scanner = LinuxScanner::new().await?;
    info!("Linux scanner created successfully");
    let scanner = Arc::new(scanner);
    
    let manager = notif_common_v5::CommonBluetoothManager::new(
        device_prefix,
        move || {
            info!("Returning pre-created Linux scanner");
            Ok(Box::new((*scanner).clone()) as Box<dyn Scanner>)
        },
    );
    
    Ok(manager)
}