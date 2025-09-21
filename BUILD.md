# ビルド環境構築ガイド

## 🎯 概要

Notifサーバーをソースからビルドするための詳細な環境構築手順です。

> **クイックスタート**: 実行済みバイナリを使って今すぐ始めたい場合は **[QUICKSTART.md](QUICKSTART.md)** をご覧ください。

## 📋 前提条件

- インターネット接続（依存関係ダウンロード用）
- 管理者権限（システム依存関係インストール用）
- 約2GB以上の空きディスク容量

## 🐧 Linux環境（Ubuntu/Debian系）

### 1. システム更新
```bash
sudo apt update
sudo apt upgrade -y
```

### 2. 基本開発ツール
```bash
sudo apt install -y \
  curl \
  build-essential \
  pkg-config \
  libssl-dev
```

### 3. Bluetooth関連ライブラリ
```bash
sudo apt install -y \
  libdbus-1-dev \
  libudev-dev \
  bluez \
  bluetooth
```

### 4. Rustツールチェーン
```bash
# Rustupインストール
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 環境変数読み込み
source ~/.cargo/env

# インストール確認
rustc --version
cargo --version
```

### 5. ビルド実行
```bash
cd /path/to/snotif/v5

# 依存関係確認
cargo check

# Linuxバイナリビルド
./scripts/build-linux-v5.sh

# ビルド成果物確認
ls -la ../bin/linux-v5/notif-server-v5
```

## 🪟 Windows環境

### 1. Visual Studio Build Tools

以下いずれかをインストール：

**Option A: Visual Studio Community（推奨）**
- https://visualstudio.microsoft.com/vs/community/ からダウンロード
- インストール時に「C++によるデスクトップ開発」を選択

**Option B: Build Tools のみ**
- https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022
- 「C++ build tools」を選択

### 2. Rustツールチェーン
```powershell
# https://rustup.rs/ からインストーラーダウンロード
# rustup-init.exe を実行

# インストール確認
rustc --version
cargo --version
```

### 3. Git（必要な場合）
```powershell
# Git for Windows
# https://git-scm.com/download/win
```

### 4. ビルド実行
```cmd
cd C:\path\to\snotif\v5

REM 依存関係確認
cargo check

REM Windowsバイナリビルド
scripts\build-windows-v5.sh

REM ビルド成果物確認
dir ..\bin\windows-v5\notif-server-v5.exe
```

## 🚀 クロスコンパイル（Linux上でWindows向けビルド）

### 1. ターゲット追加
```bash
rustup target add x86_64-pc-windows-gnu
```

### 2. MinGW-w64インストール
```bash
sudo apt install -y mingw-w64
```

### 3. クロスコンパイル実行
```bash
cd /path/to/snotif/v5
./scripts/build-windows-v5.sh
```

## 🔧 環境変数設定

### Linux
```bash
# ~/.bashrc または ~/.zshrc に追加
export RUST_LOG=info
export CARGO_NET_GIT_FETCH_WITH_CLI=true
```

### Windows
```cmd
REM システム環境変数またはユーザー環境変数に設定
set RUST_LOG=info
set CARGO_NET_GIT_FETCH_WITH_CLI=true
```

## 📊 ビルド時間の目安

| 環境 | 初回ビルド | 増分ビルド |
|------|------------|------------|
| Linux (4コア) | 5-10分 | 1-3分 |
| Windows (4コア) | 8-15分 | 2-5分 |
| クロスコンパイル | 6-12分 | 2-4分 |

## ❓ よくある問題と解決法

### Linux

**Q: `error: linker 'cc' not found`**
```bash
sudo apt install build-essential
```

**Q: `error: could not find system library 'dbus-1'`**
```bash
sudo apt install libdbus-1-dev pkg-config
```

**Q: Bluetooth権限エラー**
```bash
sudo usermod -a -G bluetooth $USER
# ログアウト/ログイン必要
```

### Windows

**Q: `error: Microsoft Visual C++ 14.0 is required`**
- Visual Studio Build Tools をインストール

**Q: `error: could not find 'link.exe'`**
- Visual Studio Installer で「MSVC v143 - VS 2022 C++ x64/x86 build tools」を追加

**Q: 長いパス名エラー**
```powershell
# 管理者権限でレジストリ編集
New-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" -Name "LongPathsEnabled" -Value 1 -PropertyType DWORD -Force
```

## 🧹 ビルドキャッシュクリア

### 完全クリーンビルド
```bash
cd v5
cargo clean
rm -rf target*/  # または Windows: rmdir /s target*
cargo build --release
```

### キャッシュディレクトリ
```bash
# Linux/macOS
~/.cargo/registry/
~/.cargo/git/

# Windows
%USERPROFILE%\.cargo\registry\
%USERPROFILE%\.cargo\git\
```

## 📈 ビルド最適化

### 並列ビルド
```bash
# CPUコア数に応じて調整
export CARGO_BUILD_JOBS=4
cargo build --release
```

### リンク高速化（Linux）
```bash
# ~/.cargo/config.toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

### 依存関係事前ダウンロード
```bash
cargo fetch
```

## 🔗 参考リンク

- [Rust公式インストールガイド](https://www.rust-lang.org/tools/install)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)
- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [Cross Compilation](https://rust-lang.github.io/rustup/cross-compilation.html)