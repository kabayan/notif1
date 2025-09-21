#!/bin/bash
# Linux v5サーバービルドスクリプト

set -e  # エラー時に停止

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/../.."
V5_DIR="$PROJECT_ROOT/v5"
LINUX_V5_DIR="$V5_DIR/src/rust/linux-v5"
OUTPUT_DIR="$PROJECT_ROOT/bin/linux-v5"

echo "==================================="
echo "  Notif v5 Linux Build Script"
echo "==================================="
echo "Build started at: $(date)"
echo "Project root: $PROJECT_ROOT"
echo ""

# 出力ディレクトリ作成
mkdir -p "$OUTPUT_DIR"

# テスト結果ディレクトリ作成
mkdir -p "$V5_DIR/test-results/phase01a"

# v5ディレクトリに移動
cd "$LINUX_V5_DIR"

# ビルド実行
echo "Building Linux v5 server..."
echo "Timeout: 30 minutes (1800000ms)"
echo "Using target directory: target-v5"

timeout 1800 cargo build --release --target-dir target-v5 2>&1 | tee "$V5_DIR/test-results/phase01a/build.log"

if [ ${PIPESTATUS[0]} -eq 0 ]; then
    echo "✅ Build successful!"
    
    # ビルド結果確認
    echo "Checking build output..."
    ls -la target-v5/release/notif* || echo "No notif binaries found in target-v5/release/"
    
    # 実際のバイナリ名を検索
    BINARY_PATH=$(find target-v5/release -maxdepth 1 -type f -executable -name "notif*" | head -1)
    
    if [ -n "$BINARY_PATH" ]; then
        echo "Found binary: $BINARY_PATH"
        # バイナリコピー
        cp "$BINARY_PATH" "$OUTPUT_DIR/notif-server-v5"
        
        # 実行権限付与
        chmod +x "$OUTPUT_DIR/notif-server-v5"
        
        # ファイルサイズ表示
        ls -lh "$OUTPUT_DIR/notif-server-v5"
        
        echo ""
        echo "Binary location: $OUTPUT_DIR/notif-server-v5"
        echo "Build completed at: $(date)"
    else
        echo "❌ Binary not found in target/release/"
        exit 1
    fi
else
    echo "❌ Build failed!"
    exit 1
fi
