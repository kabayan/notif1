#!/bin/bash

# Notif v5 統合ビルドスクリプト
# Windows・Linux両版を順次ビルド

set -e

echo "================================================"
echo "Notif v5 Unified Build Script"
echo "Building both Windows and Linux versions"
echo "================================================"

# カレントディレクトリを確認
if [ ! -f "v5/Cargo.toml" ]; then
    echo "❌ Error: このスクリプトはnotifプロジェクトのルートディレクトリで実行してください"
    exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "🚀 Starting unified build process..."

# Linux版をビルド
echo ""
echo "1️⃣  Building Linux version..."
echo "--------------------"
bash "$script_dir/build-linux-v5.sh"

# Windows版をビルド（クロスコンパイル対応）  
echo ""
echo "2️⃣  Building Windows version..."
echo "--------------------"

# Windowsクロスコンパイル環境の確認
if rustup target list --installed | grep -q "x86_64-pc-windows-gnu"; then
    echo "🔧 Windows cross-compilation target found"
    cd v5
    
    echo "🔨 Cross-compiling for Windows..."
    cargo build \
        --release \
        --bin notif-server-v5 \
        --package notif-server-v5-windows \
        --target x86_64-pc-windows-gnu \
        --target-dir target
    
    # バイナリをコピー
    mkdir -p ../bin/windows-v5
    if [ -f "target/x86_64-pc-windows-gnu/release/notif-server-v5.exe" ]; then
        cp "target/x86_64-pc-windows-gnu/release/notif-server-v5.exe" ../bin/windows-v5/
        echo "✅ Windows版（クロスコンパイル）ビルド完了"
    fi
    
    cd ..
else
    echo "⚠️  Windows cross-compilation target not found"
    echo "   Linux環境でWindows版をビルドするには:"
    echo "   rustup target add x86_64-pc-windows-gnu"
    echo "   sudo apt install gcc-mingw-w64-x86-64  # Ubuntu/Debian"
    echo ""
    echo "   通常のWindows版ビルドスクリプトを実行します..."
    bash "$script_dir/build-windows-v5.sh"
fi

echo ""
echo "================================================"
echo "🎉 統合ビルドが完了しました！"
echo ""
echo "生成されたバイナリ:"

if [ -f "bin/linux-v5/notif-server-v5" ]; then
    linux_size=$(du -h bin/linux-v5/notif-server-v5 | cut -f1)
    echo "  ✅ Linux: bin/linux-v5/notif-server-v5 ($linux_size)"
fi

if [ -f "bin/windows-v5/notif-server-v5.exe" ]; then
    windows_size=$(du -h bin/windows-v5/notif-server-v5.exe | cut -f1)
    echo "  ✅ Windows: bin/windows-v5/notif-server-v5.exe ($windows_size)"
elif [ -f "bin/windows-v5/notif-server-v5" ]; then
    windows_size=$(du -h bin/windows-v5/notif-server-v5 | cut -f1)
    echo "  ✅ Windows: bin/windows-v5/notif-server-v5 ($windows_size)"
fi

echo ""
echo "使用方法:"
echo "  Linux:   ./bin/linux-v5/notif-server-v5"
echo "  Windows: ./bin/windows-v5/notif-server-v5.exe"
echo ""
echo "v5の新機能:"
echo "  🖼️  画像アップロード機能"
echo "  🔗 URL画像取得機能"
echo "  🧪 テスト画像配信"
echo "  🌐 MCP統合対応"
echo "  🖥️  クロスプラットフォーム対応"
echo "================================================"