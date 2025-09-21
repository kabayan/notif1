//! Linux v3ビルドスクリプト

use chrono::Utc;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // ビルド時刻を環境変数として設定
    let build_time = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);
    
    // Gitコミットハッシュを取得（可能な場合）
    if let Ok(output) = std::process::Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
    {
        let git_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("cargo:rustc-env=GIT_HASH={}", git_hash);
    } else {
        println!("cargo:rustc-env=GIT_HASH=unknown");
    }
    
    // ターゲットアーキテクチャ
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_TARGET={}", target);
    
    // プロファイル（debug/release）
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_PROFILE={}", profile);
    
    // Linux固有の設定を確認
    if cfg!(target_os = "linux") {
        println!("cargo:rustc-cfg=linux_platform");
        
        // btleplugのための設定
        println!("cargo:rustc-link-lib=dylib=dbus-1");
    }
    
    // ビルド番号をタイムスタンプ形式（YYMMDDHHmmss）で生成
    let build_number = Utc::now().format("%y%m%d%H%M%S").to_string();
    
    println!("cargo:warning=Build number generated: {}", build_number);
    
    // ビルド番号を環境変数として設定
    println!("cargo:rustc-env=BUILD_NUMBER={}", build_number);
    
    // 再ビルドのトリガー
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=BUILD_NUMBER");
}