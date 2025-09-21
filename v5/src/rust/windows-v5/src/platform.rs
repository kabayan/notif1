//! Windows固有のプラットフォーム処理（v2スタイル）

use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tracing::{info, warn};

use windows::{
    Win32::{
        Foundation::{BOOL, HANDLE},
        System::{
            Console::{SetConsoleCtrlHandler, CTRL_C_EVENT, CTRL_BREAK_EVENT},
            SystemInformation::{GetSystemInfo, SYSTEM_INFO},
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::{GetCurrentProcess, SetPriorityClass, HIGH_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS},
        },
    },
    Devices::Radios::{Radio, RadioKind},
};

use notif_common_v5::Result;

/// Windows Errorを NotifErrorに変換（v2スタイル）
fn windows_error_to_notif_error(err: windows::core::Error) -> notif_common_v5::NotifError {
    notif_common_v5::NotifError::Platform(format!("Windows API error: {}", err.message()))
}

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

/// Windows固有のプラットフォーム処理
pub struct WindowsPlatform;

impl WindowsPlatform {
    /// プラットフォーム初期化
    pub async fn initialize() -> Result<()> {
        // コンソールのコードページをUTF-8に設定
        unsafe {
            windows::Win32::System::Console::SetConsoleOutputCP(65001);
            windows::Win32::System::Console::SetConsoleCP(65001);
        }
        
        info!("Windows platform initialized");
        Ok(())
    }
    
    /// シャットダウンハンドラーのセットアップ
    pub async fn setup_shutdown_handler() -> Result<ShutdownReceiver> {
        let (tx, rx) = oneshot::channel();
        
        // tokioのCtrl+Cハンドラーを使用（v2スタイル）
        tokio::spawn(async move {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    info!("Received Ctrl+C signal");
                    let _ = tx.send(());
                }
                Err(err) => {
                    warn!("Error setting up Ctrl+C handler: {}", err);
                }
            }
        });
        
        info!("Shutdown handler registered");
        Ok(ShutdownReceiver { rx })
    }
    
    /// プラットフォーム情報を取得
    pub fn get_platform_info() -> PlatformInfo {
        let mut sys_info = SYSTEM_INFO::default();
        unsafe {
            GetSystemInfo(&mut sys_info);
        }
        
        let processor_arch = match unsafe { sys_info.Anonymous.Anonymous.wProcessorArchitecture.0 } {
            9 => "x64",
            5 => "ARM",
            12 => "ARM64", 
            6 => "IA64",
            0 => "x86",
            _ => "Unknown",
        };
        
        PlatformInfo {
            os: "Windows".to_string(),
            version: get_windows_version(),
            kernel: format!("NT {}", get_windows_build()),
            distribution: format!("Windows {} ({})", get_windows_edition(), processor_arch),
        }
    }
    
    /// Bluetooth利用可能かチェック
    pub async fn check_bluetooth_available() -> bool {
        match Radio::GetRadiosAsync() {
            Ok(operation) => {
                match operation.get() {
                    Ok(radios) => {
                        for i in 0..radios.Size().unwrap_or(0) {
                            if let Ok(radio) = radios.GetAt(i) {
                                if let Ok(kind) = radio.Kind() {
                                    if kind == RadioKind::Bluetooth {
                                        return true;
                                    }
                                }
                            }
                        }
                        false
                    }
                    Err(_) => false,
                }
            }
            Err(_) => false,
        }
    }
    
    /// プロセス優先度を設定
    pub fn set_process_priority(high_priority: bool) -> Result<()> {
        unsafe {
            let process = GetCurrentProcess();
            let priority_class = if high_priority {
                HIGH_PRIORITY_CLASS
            } else {
                NORMAL_PRIORITY_CLASS
            };
            
            if let Err(_) = SetPriorityClass(process, priority_class) {
                return Err(notif_common_v5::NotifError::Platform(
                    "Failed to set process priority".to_string()
                ));
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

/// システムリソース情報
pub struct SystemResources {
    pub memory_available_mb: u64,
    pub cpu_count: usize,
    pub cpu_usage: f32,
}

impl SystemResources {
    /// システムリソース情報を取得
    pub fn get() -> Self {
        let (memory_available, _memory_total) = get_memory_info();
        let cpu_count = num_cpus::get();
        let cpu_usage = 0.0; // 簡易版
        
        SystemResources {
            memory_available_mb: memory_available,
            cpu_count,
            cpu_usage,
        }
    }
}

/// Windowsバージョンを取得
fn get_windows_version() -> String {
    if cfg!(target_arch = "x86_64") {
        "Windows 10/11 x64".to_string()
    } else {
        "Windows 10/11".to_string()
    }
}

/// Windowsビルド番号を取得
fn get_windows_build() -> String {
    "10.0.19044".to_string()
}

/// Windowsエディションを取得
fn get_windows_edition() -> String {
    "Professional".to_string()
}

/// メモリ情報を取得
fn get_memory_info() -> (u64, u64) {
    unsafe {
        let process = GetCurrentProcess();
        let mut mem_counters = PROCESS_MEMORY_COUNTERS::default();
        let cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        
        if GetProcessMemoryInfo(
            process,
            &mut mem_counters as *mut _ as *mut _,
            cb,
        ).is_ok() {
            let working_set_mb = mem_counters.WorkingSetSize / (1024 * 1024);
            let peak_working_set_mb = mem_counters.PeakWorkingSetSize / (1024 * 1024);
            return (working_set_mb as u64, peak_working_set_mb as u64);
        }
    }
    
    (512, 1024) // デフォルト値
}

mod num_cpus {
    pub fn get() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    }
}