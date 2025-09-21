//! Windows v3ビルドスクリプト

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
    
    // Windows固有の設定
    if cfg!(target_os = "windows") {
        println!("cargo:rustc-cfg=windows_platform");
        
        // Windows SDK関連の設定
        println!("cargo:rustc-link-lib=dylib=user32");
        println!("cargo:rustc-link-lib=dylib=kernel32");
        
        // Windows Runtime用の設定
        println!("cargo:rustc-link-lib=windowsapp");
        
        // マニフェスト埋め込み（管理者権限不要）
        embed_manifest();
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

/// Windowsマニフェストを埋め込む
fn embed_manifest() {
    // マニフェストファイルの内容
    let manifest = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
    <assemblyIdentity
        version="3.0.0.0"
        processorArchitecture="*"
        name="notif.v3.windows"
        type="win32"
    />
    <description>Notif v3 Windows Server</description>
    <dependency>
        <dependentAssembly>
            <assemblyIdentity
                type="win32"
                name="Microsoft.Windows.Common-Controls"
                version="6.0.0.0"
                processorArchitecture="*"
                publicKeyToken="6595b64144ccf1df"
                language="*"
            />
        </dependentAssembly>
    </dependency>
    <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
        <application>
            <!-- Windows 10 / Windows 11 -->
            <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
            <!-- Windows 8.1 -->
            <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}"/>
            <!-- Windows 8 -->
            <supportedOS Id="{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}"/>
            <!-- Windows 7 -->
            <supportedOS Id="{35138b9a-5d96-4fbd-8e2d-a2440225f93a}"/>
        </application>
    </compatibility>
    <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
        <security>
            <requestedPrivileges>
                <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
            </requestedPrivileges>
        </security>
    </trustInfo>
    <application xmlns="urn:schemas-microsoft-com:asm.v3">
        <windowsSettings>
            <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
            <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
        </windowsSettings>
    </application>
</assembly>"#;
    
    // マニフェストファイルを一時的に作成
    let out_dir = env::var("OUT_DIR").unwrap();
    let manifest_path = Path::new(&out_dir).join("notif-v3.exe.manifest");
    fs::write(&manifest_path, manifest).expect("Failed to write manifest");
    
    // リソースファイル（.rc）を作成
    let rc_content = format!(
        r#"#define RT_MANIFEST 24
1 RT_MANIFEST "{}"
"#,
        manifest_path.display().to_string().replace('\\', "\\\\")
    );
    
    let rc_path = Path::new(&out_dir).join("manifest.rc");
    fs::write(&rc_path, rc_content).expect("Failed to write resource file");
    
    // embed-resource crateを使う代わりに、cargo:rustc-link-argを使用
    if cfg!(target_os = "windows") {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest_path.display());
    }
}