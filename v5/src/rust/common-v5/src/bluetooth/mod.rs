//! Bluetooth共通モジュール

pub mod traits;
pub mod manager;

#[cfg(feature = "mock")]
pub mod mock;

// 再エクスポート
pub use traits::{
    BluetoothManager,
    Connection,
    Scanner,
    DeviceInfo,
    DeviceCapabilities,
    DeviceStatistics,
    PlatformData,
};

pub use manager::CommonBluetoothManager;