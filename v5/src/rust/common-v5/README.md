# notif v3 共通ライブラリ

## 概要
notif v3のWindows/Linux両プラットフォームで共通のコードを提供するライブラリです。
既存のコードを変更することなく、段階的な移行が可能です。

## 特徴
- **プラットフォーム抽象化**: Bluetoothアクセスをトレイトで抽象化
- **共通ビジネスロジック**: APIハンドラー、プロトコル処理を統一
- **統一設定管理**: 環境変数、設定ファイル、デフォルト値の優先順位
- **型安全**: Rustの型システムを活用した安全な実装

## ディレクトリ構造
```
common-v3/
├── src/
│   ├── error.rs           # 共通エラー型
│   ├── protocol.rs        # プロトコル定義
│   ├── bluetooth/         # Bluetooth抽象化
│   │   ├── traits.rs      # トレイト定義
│   │   └── manager.rs     # 共通マネージャー実装
│   ├── config/            # 設定管理
│   │   └── mod.rs         # 統一設定
│   ├── api/               # API関連
│   │   ├── models.rs      # データモデル
│   │   └── handlers.rs    # 共通ハンドラー
│   └── lib.rs             # ライブラリルート
```

## 使用方法

### 1. 依存関係の追加

**Linux版の場合（linux/Cargo.toml）:**
```toml
[dependencies]
notif-v3-common = { path = "../common-v3" }
btleplug = "0.11"  # Linux固有
```

**Windows版の場合（windows/Cargo.toml）:**
```toml
[dependencies]
notif-v3-common = { path = "../common-v3" }
windows = { version = "0.48", features = [...] }  # Windows固有
```

### 2. トレイトの実装

**Linux版の実装例:**
```rust
use notif_v3_common::{Connection, Result, Command};
use btleplug::platform::Peripheral;
use async_trait::async_trait;

pub struct LinuxConnection {
    device: Peripheral,
    // ...
}

#[async_trait]
impl Connection for LinuxConnection {
    async fn send_command(&mut self, command: Command) -> Result<()> {
        // btleplugを使った実装
        let data = command.encode();
        self.device.write(&self.char, &data, WriteType::WithResponse).await?;
        Ok(())
    }
    
    async fn is_connected(&self) -> bool {
        self.device.is_connected().await.unwrap_or(false)
    }
    
    // 他のメソッド実装...
}
```

**Windows版の実装例:**
```rust
use notif_v3_common::{Connection, Result, Command};
use windows::Devices::Bluetooth::*;
use async_trait::async_trait;

pub struct WindowsConnection {
    device: BluetoothLEDevice,
    // ...
}

#[async_trait]
impl Connection for WindowsConnection {
    async fn send_command(&mut self, command: Command) -> Result<()> {
        // windows-rsを使った実装
        let data = command.encode();
        // Windows固有のAPI呼び出し
        Ok(())
    }
    
    async fn is_connected(&self) -> bool {
        self.device.ConnectionStatus() == BluetoothConnectionStatus::Connected
    }
    
    // 他のメソッド実装...
}
```

### 3. 共通マネージャーの使用

```rust
use notif_v3_common::{CommonBluetoothManager, Settings};

// プラットフォーム固有のスキャナーファクトリー
fn create_scanner() -> Result<Box<dyn Scanner>> {
    #[cfg(target_os = "linux")]
    return Ok(Box::new(LinuxScanner::new()?));
    
    #[cfg(target_os = "windows")]
    return Ok(Box::new(WindowsScanner::new()?));
}

// 共通マネージャーの初期化
let settings = Settings::new()?;
let manager = CommonBluetoothManager::new(
    settings.bluetooth.device_name_prefix,
    create_scanner,
);

// デバイスのスキャンと接続
let devices = manager.scan_and_connect_all().await?;

// コマンド送信
manager.send_command_to_all(Command::Clear { 
    color: RGB::black() 
}).await?;
```

### 4. APIハンドラーの使用

```rust
use actix_web::{web, App, HttpServer};
use notif_v3_common::api::{process_v1_send, process_v2_draw};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let manager = create_bluetooth_manager().await?;
    
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(manager.clone()))
            // v1互換API
            .route("/send", web::get().to(process_v1_send::<MyManager>))
            .route("/send", web::post().to(process_v1_send::<MyManager>))
            // v2 API
            .route("/api/draw", web::post().to(process_v2_draw::<MyManager>))
    })
    .bind("0.0.0.0:18080")?
    .run()
    .await
}
```

## 移行手順

### Phase 1: 新規プロジェクトでの使用
1. 新しいプロジェクトを作成
2. `notif-v3-common`を依存関係に追加
3. プラットフォーム固有実装を作成
4. テスト実施

### Phase 2: 既存コードの段階的移行
1. 既存コードはそのまま残す
2. 新機能は共通ライブラリを使用
3. バグ修正時に共通ライブラリへ移行
4. 最終的に既存コードを削除

### Phase 3: 完全移行
1. すべての機能が共通ライブラリを使用
2. 既存の重複コードを削除
3. ディレクトリ構造の整理

## テスト

```bash
# 単体テスト
cd v3/src/rust/common-v3
cargo test

# 統合テスト（Linux）
cd v3/src/rust/linux
cargo test

# 統合テスト（Windows）
cd v3/src/rust/windows
cargo test
```

## ライセンス
[プロジェクトのライセンスに準拠]