# AtomS3 ファームウェアアップロード手順

## 📋 必要なもの

- **M5 AtomS3** デバイス
- **USB-Cケーブル**
- **PlatformIO** または **M5Burner**

## 🚀 方法1: PlatformIO（推奨）

### インストール
```bash
# Python pip経由（推奨）
pip install platformio

# または HomebrewでmacOS
brew install platformio

# 確認
pio --version
```

### アップロード手順
```bash
# ファームウェアディレクトリに移動
cd v5/src/atoms3

# デバイス接続確認
ls /dev/ttyACM*  # Linux
ls /dev/cu.*     # macOS

# ビルドとアップロード
pio run --target upload --upload-port /dev/ttyACM0  # Linux
pio run --target upload --upload-port /dev/cu.usbmodem*  # macOS
pio run --target upload --upload-port COM3         # Windows
```

### 自動ポート検出
```bash
# PlatformIOに自動検出させる
pio run --target upload
```

## 🪟 方法2: M5Burner（Windows推奨）

### ダウンロード・インストール
1. [M5Burner公式ページ](https://docs.m5stack.com/en/uiflow/m5burner/intro)
2. Windows用インストーラーダウンロード
3. インストール実行

### アップロード手順
1. **デバイス接続**
   - AtomS3をUSB-CケーブルでPCに接続
   - デバイスマネージャーでCOMポート番号確認

2. **M5Burner設定**
   - M5Burner起動
   - Device: **ATOM S3** 選択
   - 右上の「Custom .bin」をクリック

3. **ファームウェア選択**
   - `v5/bin/atoms3/atoms3-firmware.bin` を指定
   - またはビルド済みの場合 `v5/src/atoms3/.pio/build/atoms3/firmware.bin`

4. **書き込み実行**
   - PORT（COM番号）を選択
   - 「Burn」ボタンクリック
   - 完了まで待機

## 🐧 方法3: WSL2環境（Windows + Linux）

### Windows側準備
```powershell
# 管理者権限PowerShellで実行

# usbipd-winインストール
winget install --interactive --exact dorssel.usbipd-win

# USBデバイス確認
usbipd list

# 共有設定（BUSIDは実際の値に置き換え）
usbipd bind --busid 2-4

# WSLにアタッチ
usbipd attach --wsl --busid 2-4
```

### WSL側作業
```bash
# USBツールインストール（初回のみ）
sudo apt update
sudo apt install linux-tools-generic hwdata

# デバイス確認
ls /dev/ttyACM*

# PlatformIOでアップロード
cd v5/src/atoms3
pio run --target upload --upload-port /dev/ttyACM0
```

## 🛠️ トラブルシューティング

### デバイスが認識されない

**Linux:**
```bash
# 権限確認
ls -l /dev/ttyACM*
sudo chmod 666 /dev/ttyACM0

# ダイアルアウトグループ追加
sudo usermod -a -G dialout $USER
# ログアウト/ログイン必要
```

**Windows:**
- デバイスマネージャーで「不明なデバイス」を確認
- USBドライバー再インストール
- 別のUSBポートを試す

### ブートローダーモード

手動でブートローダーモードに入る場合：
1. **BOOTボタン**を押しながら
2. **RESETボタン**を押す
3. BOOTボタンを離す
4. アップロード実行

### アップロードエラー

```bash
# キャッシュクリア
pio run --target clean

# 再ビルド
pio run

# 強制アップロード
pio run --target upload --upload-port /dev/ttyACM0 --force
```

### バージョン確認

アップロード後の確認方法：
```bash
# シリアルモニター
pio device monitor --port /dev/ttyACM0 --baud 115200

# 終了: Ctrl+C
```

## 📊 ファームウェア情報

### ビルド構成
- **フレームワーク**: Arduino ESP32
- **ボード**: M5Stack ATOM S3
- **CPU**: ESP32-S3
- **Flash**: 8MB
- **RAM**: 512KB

### 主要機能
- **Bluetooth LE**: デバイス通信
- **LCD制御**: 128x128 カラーディスプレイ
- **画像表示**: JPEG, PNG, BMP対応
- **テキスト表示**: 日本語フォント対応
- **接続最適化**: Connection Interval調整

### 設定可能項目
```cpp
// platformio.ini で変更可能
#define DEVICE_NAME_PREFIX "notif_atoms3"
#define CONNECTION_INTERVAL_MIN 11  // 13.75ms
#define CONNECTION_INTERVAL_MAX 11  // 13.75ms
#define SLAVE_LATENCY 0
#define SUPERVISION_TIMEOUT 300     // 3000ms
```

## 🔗 関連リンク

- [PlatformIO公式ドキュメント](https://docs.platformio.org/)
- [M5Burner公式ページ](https://docs.m5stack.com/en/uiflow/m5burner/intro)
- [ESP32-S3データシート](https://www.espressif.com/sites/default/files/documentation/esp32-s3_datasheet_en.pdf)
- [M5 AtomS3公式ページ](https://docs.m5stack.com/en/core/AtomS3)