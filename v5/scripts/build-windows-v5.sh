#!/bin/bash

# Notif v5 Windows ビルドスクリプト
# 画像送信機能付きBluetoothサーバー

set -e

echo "================================================"
echo "Notif v5 Windows Build Script"
echo "================================================"

# カレントディレクトリを確認
if [ ! -f "v5/Cargo.toml" ]; then
    echo "❌ Error: このスクリプトはnotifプロジェクトのルートディレクトリで実行してください"
    exit 1
fi

cd v5

echo "🏗️  Building Notif v5 Windows版..."

# 依存関係の確認
echo "📦 Checking dependencies..."
cargo check --quiet

# Windows向けビルド（Release）
echo "🔨 Building Windows release binary..."
cargo build \
    --release \
    --bin notif-server-v5 \
    --package notif-server-v5-windows \
    --target-dir target

# ビルド成功確認
if [ -f "target/release/notif-server-v5.exe" ] || [ -f "target/release/notif-server-v5" ]; then
    echo "✅ ビルド成功！"
    
    # バイナリを適切な場所にコピー
    mkdir -p ../bin/windows-v5
    
    if [ -f "target/release/notif-server-v5.exe" ]; then
        cp target/release/notif-server-v5.exe ../bin/windows-v5/
        echo "📁 バイナリを bin/windows-v5/notif-server-v5.exe にコピーしました"
        
        # サイズ情報
        file_size=$(du -h target/release/notif-server-v5.exe | cut -f1)
        echo "📊 ファイルサイズ: $file_size"
    else
        cp target/release/notif-server-v5 ../bin/windows-v5/
        echo "📁 バイナリを bin/windows-v5/notif-server-v5 にコピーしました"
        
        # サイズ情報
        file_size=$(du -h target/release/notif-server-v5 | cut -f1)
        echo "📊 ファイルサイズ: $file_size"
    fi
    
    # バージョン情報
    echo "📋 ビルド情報:"
    echo "   - Version: v5.0.0"
    echo "   - Platform: Windows"
    echo "   - Features: Image upload, URL fetch, MCP support"
    echo "   - Target: x86_64-pc-windows-msvc"
    echo "   - Build time: $(date)"
    
else
    echo "❌ ビルドに失敗しました"
    exit 1
fi

echo "================================================"
echo "✨ Notif v5 Windows版のビルドが完了しました！"
echo ""
echo "使用方法:"
echo "  ./bin/windows-v5/notif-server-v5.exe"
echo ""
echo "設定:"
echo "  環境変数またはconfig.jsonで設定可能"
echo "  例: HOST=127.0.0.1 PORT=18080 LOG_LEVEL=info"
echo ""
echo "新機能:"
echo "  - 画像アップロード: POST /api/image/upload"
echo "  - URL画像送信: POST /api/image/url"  
echo "  - テスト画像: GET /test-images/{filename}"
echo "================================================"