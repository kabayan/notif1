//! Linux固有のプラットフォーム処理

use tokio::signal;
use tokio::sync::oneshot;
use tracing::{info, warn};

use notif_common_v5::Result;

/// シャットダウンシグナル受信器
pub struct ShutdownReceiver {
    rx: oneshot::Receiver<()>,
}

impl ShutdownReceiver {
    /// シグナルを待機
    pub async fn wait(self) {
        let _ = self.rx.await;
    }
}

/// Linux固有のプラットフォーム処理
pub struct LinuxPlatform;

impl LinuxPlatform {
    /// プラットフォーム初期化
    pub async fn initialize() -> Result<()> {
        // Linux固有の初期化処理
        info!("Linux platform initialized");
        Ok(())
    }
    
    /// Ctrl+Cハンドラーのセットアップ
    pub async fn setup_shutdown_handler() -> Result<ShutdownReceiver> {
        let (tx, rx) = oneshot::channel();
        
        tokio::spawn(async move {
            match signal::ctrl_c().await {
                Ok(()) => {
                    info!("Received Ctrl+C signal");
                    let _ = tx.send(());
                }
                Err(err) => {
                    warn!("Unable to listen for shutdown signal: {}", err);
                }
            }
        });
        
        Ok(ShutdownReceiver { rx })
    }
    
    /// プラットフォーム情報を取得
    pub fn get_platform_info() -> PlatformInfo {
        PlatformInfo {
            os: "Linux".to_string(),
            version: get_linux_version(),
            kernel: get_kernel_version(),
            distribution: get_distribution(),
        }
    }
    
    /// Bluetooth利用可能かチェック
    pub async fn check_bluetooth_available() -> bool {
        // systemdサービスのチェック（簡易版）
        match tokio::process::Command::new("systemctl")
            .args(&["is-active", "bluetooth"])
            .output()
            .await
        {
            Ok(output) => {
                let status = String::from_utf8_lossy(&output.stdout);
                status.trim() == "active"
            }
            Err(_) => {
                // systemctlが使えない場合は、/sys/class/bluetoothをチェック
                std::path::Path::new("/sys/class/bluetooth").exists()
            }
        }
    }
    
    /// プロセス優先度を設定
    pub fn set_process_priority(priority: i32) -> Result<()> {
        unsafe {
            let pid = 0; // 0 = current process
            let result = libc::setpriority(libc::PRIO_PROCESS, pid as u32, priority);
            if result == -1 {
                let error = std::io::Error::last_os_error();
                warn!("Failed to set process priority: {}", error);
                // エラーでも続行
            }
        }
        Ok(())
    }
}

/// プラットフォーム情報
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    pub os: String,
    pub version: String,
    pub kernel: String,
    pub distribution: String,
}

/// Linuxバージョンを取得
fn get_linux_version() -> String {
    std::fs::read_to_string("/proc/version")
        .unwrap_or_else(|_| "Unknown".to_string())
        .lines()
        .next()
        .unwrap_or("Unknown")
        .to_string()
}

/// カーネルバージョンを取得
fn get_kernel_version() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_else(|_| "Unknown".to_string())
        .trim()
        .to_string()
}

/// ディストリビューション情報を取得
fn get_distribution() -> String {
    // /etc/os-releaseから読み取り
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("PRETTY_NAME=") {
                return line
                    .trim_start_matches("PRETTY_NAME=")
                    .trim_matches('"')
                    .to_string();
            }
        }
    }
    
    // /etc/lsb-releaseから読み取り
    if let Ok(content) = std::fs::read_to_string("/etc/lsb-release") {
        for line in content.lines() {
            if line.starts_with("DISTRIB_DESCRIPTION=") {
                return line
                    .trim_start_matches("DISTRIB_DESCRIPTION=")
                    .trim_matches('"')
                    .to_string();
            }
        }
    }
    
    "Unknown".to_string()
}

/// シグナルハンドラーの設定（拡張版）
pub async fn setup_signal_handlers() -> Result<()> {
    use tokio::signal::unix::{signal, SignalKind};
    
    // SIGTERM
    let mut sigterm = signal(SignalKind::terminate())
        .map_err(|e| notif_common_v5::NotifError::Platform(format!("Failed to setup SIGTERM handler: {}", e)))?;
    
    // SIGHUP
    let mut sighup = signal(SignalKind::hangup())
        .map_err(|e| notif_common_v5::NotifError::Platform(format!("Failed to setup SIGHUP handler: {}", e)))?;
    
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = sigterm.recv() => {
                    info!("Received SIGTERM signal");
                    break;
                }
                _ = sighup.recv() => {
                    info!("Received SIGHUP signal - reloading configuration");
                    // 設定リロード処理
                }
            }
        }
    });
    
    Ok(())
}

/// システムリソース情報
pub struct SystemResources {
    pub memory_available_mb: u64,
    pub memory_total_mb: u64,
    pub cpu_count: usize,
    pub load_average: (f32, f32, f32),
}

impl SystemResources {
    /// システムリソース情報を取得
    pub fn get() -> Self {
        let memory = get_memory_info();
        let cpu_count = num_cpus::get();
        let load_avg = get_load_average();
        
        SystemResources {
            memory_available_mb: memory.0,
            memory_total_mb: memory.1,
            cpu_count,
            load_average: load_avg,
        }
    }
}

/// メモリ情報を取得
fn get_memory_info() -> (u64, u64) {
    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        let mut total = 0u64;
        let mut available = 0u64;
        
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(value) = parse_meminfo_line(line) {
                    total = value / 1024; // KB to MB
                }
            } else if line.starts_with("MemAvailable:") {
                if let Some(value) = parse_meminfo_line(line) {
                    available = value / 1024; // KB to MB
                }
            }
        }
        
        return (available, total);
    }
    
    (0, 0)
}

/// /proc/meminfoの行をパース
fn parse_meminfo_line(line: &str) -> Option<u64> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse().ok()
    } else {
        None
    }
}

/// ロードアベレージを取得
fn get_load_average() -> (f32, f32, f32) {
    if let Ok(content) = std::fs::read_to_string("/proc/loadavg") {
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() >= 3 {
            let load1 = parts[0].parse().unwrap_or(0.0);
            let load5 = parts[1].parse().unwrap_or(0.0);
            let load15 = parts[2].parse().unwrap_or(0.0);
            return (load1, load5, load15);
        }
    }
    
    (0.0, 0.0, 0.0)
}

// num_cpus crateの代わりの簡易実装
mod num_cpus {
    pub fn get() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    }
}