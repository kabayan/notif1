# 📚 クイックスタートガイド

## 🚀 5分で始める Notif

このガイドでは、実行済みバイナリを使って最も簡単にNotifを始める方法を説明します。

## ✅ 事前準備

1. **AtomS3デバイス** - ファームウェアがインストール済み
2. **Bluetooth対応PC** - Linux または Windows
3. **このsnotifフォルダ** - 実行済みバイナリ含む

## 🎯 ステップ1: サーバー起動

### Linux の場合
```bash
# snotifディレクトリに移動
cd /path/to/snotif

# サーバー起動
./bin/linux-v5/notif-server-v5
```

### Windows の場合
```cmd
REM snotifディレクトリに移動
cd C:\path\to\snotif

REM サーバー起動
.\bin\windows-v5\notif-server-v5.exe
```

**成功例:**
```
=================================
Notif Server Starting
=================================
🚀 HTTP Server: http://127.0.0.1:18080
🔗 MCP Server: mcp://localhost:18080
📱 Scanning for devices...
```

## 🎯 ステップ2: デバイス接続確認

### ブラウザから確認
1. ブラウザで http://localhost:18080 を開く
2. 「Device Status」セクションを確認
3. `notif_atoms3_XXXXXX` デバイスが表示されるまで待つ

### コマンドラインから確認
```bash
curl http://localhost:18080/status
```

**成功例:**
```json
{
  "status": "ok",
  "device": "notif_atoms3_c9d8ec",
  "connected": true,
  "server": "5.0.0"
}
```

## 🎯 ステップ3: 初回メッセージ送信

### ブラウザから（最も簡単）
1. http://localhost:18080 の「Send Message」フォームを使用
2. テキスト: `Hello World!`
3. 背景色: `blue`
4. 文字色: `white`
5. 「Send」ボタンクリック

### cURLから
```bash
curl -G "http://localhost:18080/send" \
  --data-urlencode "text=Hello World!" \
  --data-urlencode "bgcolor=blue" \
  --data-urlencode "color=white"
```

**成功すると AtomS3 に青い背景で白文字の "Hello World!" が表示されます。**

## 🎯 ステップ4: 画像機能を試す

### 画像アップロード
```bash
# 画像ファイルをアップロード
curl -X POST http://localhost:18080/api/image/upload \
  -F "image=@your-image.png" \
  -F "device=1"
```

### URL画像送信
```bash
# Web上の画像URLから直接送信
curl -X POST http://localhost:18080/api/image/url \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com/image.png", "device": 1}'
```

### テスト画像
```bash
# 内蔵テスト画像を試す
curl http://localhost:18080/test-images/gradient.png
```

## 🎯 ステップ5: 基本操作をマスター

### 色を変える
```bash
# 緑背景、黒文字
curl -G "http://localhost:18080/send" \
  --data-urlencode "text=Success!" \
  --data-urlencode "bgcolor=green" \
  --data-urlencode "color=black"

# 赤背景、白文字
curl -G "http://localhost:18080/send" \
  --data-urlencode "text=Error!" \
  --data-urlencode "bgcolor=red" \
  --data-urlencode "color=white"
```

### フォントサイズを変える
```bash
# 大きいサイズ
curl -G "http://localhost:18080/send" \
  --data-urlencode "text=BIG" \
  --data-urlencode "size=4" \
  --data-urlencode "bgcolor=black" \
  --data-urlencode "color=yellow"

# 小さいサイズ
curl -G "http://localhost:18080/send" \
  --data-urlencode "text=small text here" \
  --data-urlencode "size=1" \
  --data-urlencode "bgcolor=white" \
  --data-urlencode "color=black"
```

### 日本語を表示
```bash
curl -G "http://localhost:18080/send" \
  --data-urlencode "text=こんにちは！" \
  --data-urlencode "font=lgfxJapanGothic_24" \
  --data-urlencode "bgcolor=purple" \
  --data-urlencode "color=white"
```

## ❓ トラブルシューティング

### デバイスが見つからない場合

1. **AtomS3の電源確認**
   - USBケーブルで給電されているか
   - LED点灯しているか

2. **Bluetoothの確認**
   ```bash
   # Linux
   bluetoothctl power on
   bluetoothctl scan on

   # Windows
   # 設定 > デバイス > Bluetooth で確認
   ```

3. **ファームウェア確認**
   - AtomS3にファームウェアがインストールされているか
   - デバイス名が `notif_atoms3_XXXXXX` 形式か

### サーバーが起動しない場合

1. **ポート使用確認**
   ```bash
   # Linux
   sudo lsof -i :18080

   # Windows
   netstat -an | findstr :18080
   ```

2. **権限確認**
   ```bash
   # Linux - Bluetooth権限
   sudo usermod -a -G bluetooth $USER
   # ログアウト/ログイン必要
   ```

3. **別ポートで起動**
   ```bash
   # 環境変数でポート変更
   PORT=18081 ./bin/linux-v5/notif-server-v5
   ```

### 接続が不安定な場合

1. **既存ペアリング削除**
   - OSのBluetooth設定から過去のペアリングを削除

2. **サーバー再起動**
   ```bash
   # Ctrl+C でサーバー停止後、再起動
   ./bin/linux-v5/notif-server-v5
   ```

## 🎉 成功！次のステップ

クイックスタートが完了しました！

### さらに学ぶ
- 📖 [完全なREADME.md](README.md) - 全機能とAPI仕様
- 🛠️ [BUILD.md](BUILD.md) - ソースからビルドする方法
- 🌐 WebUI - http://localhost:18080 で全機能をGUIで操作

### 応用例
- **開発通知**: CI/CDパイプラインからビルド状況を通知
- **IoT監視**: センサーデータの閾値超過アラート
- **会議通知**: カレンダー連携でミーティング開始通知
- **画像表示**: QRコード、グラフ、アイコンの表示

---

🎊 **おめでとうございます！** Notifの基本的な使い方をマスターしました。