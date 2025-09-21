//! notif v4 Windows版サーバー

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer, HttpResponse};
use tracing::{info, Level};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

use notif_common_v5::{
    BluetoothManager, Result, Settings, VERSION,
    api::{process_v1_send, process_v1_status, process_v2_draw, process_v2_draw_query, process_v2_draw_post, process_v2_devices, process_v2_health, process_v2_batch},
    AppState, SessionManager, mcp_handler,
};

// v5新機能のuse文追加（条件付きインポート）
#[cfg(feature = "http-endpoints")]
use notif_common_v5::api::handlers::{upload_image, post_image};

mod bluetooth_impl;
mod platform;

use bluetooth_impl::create_bluetooth_manager;
use platform::{WindowsPlatform, SystemResources};

use std::env;
use std::fs;
use std::sync::Arc;
use std::collections::HashMap;
use clap::Parser;

/// Notif v5 サーバー
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// ログレベル設定 (error, warn, info, debug, trace)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// 詳細ログ出力（notif_common_v5のログも表示）
    #[arg(short, long)]
    verbose: bool,

    /// 静かなモード（エラーのみ表示）
    #[arg(short, long)]
    quiet: bool,
}

#[actix_web::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    // 実行ディレクトリを取得
    let exe_path = env::current_exe()
        .expect("Failed to get executable path");
    let exe_dir = exe_path.parent()
        .expect("Failed to get executable directory");
    
    // logsディレクトリを作成
    let logs_dir = exe_dir.join("logs");
    fs::create_dir_all(&logs_dir)
        .expect("Failed to create logs directory");
    
    // ログファイルのパスを生成（日時付き）
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let log_file_path = logs_dir.join(format!("notif-v5-{}.log", timestamp));
    let display_log_path = logs_dir.join(format!("display-{}.log", timestamp));
    
    // ファイルライターを作成
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&log_file_path)
        .expect("Failed to open log file");
    
    // ログレベルを決定
    let log_level = if args.quiet {
        Level::ERROR
    } else {
        match args.log_level.to_lowercase().as_str() {
            "error" => Level::ERROR,
            "warn" => Level::WARN,
            "info" => Level::INFO,
            "debug" => Level::DEBUG,
            "trace" => Level::TRACE,
            _ => Level::INFO,
        }
    };

    // ログフィルターを設定
    let env_filter = if args.verbose {
        // 詳細モード：すべてのログを表示
        tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(log_level.into())
    } else {
        // 通常モード：notif_common_v5のログは警告以上のみ表示
        tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(log_level.into())
            .add_directive("notif_common_v5=warn".parse().unwrap())
    };

    // ログ初期化（コンソールとファイルの両方に出力）
    let file_layer = fmt::layer()
        .with_writer(file)
        .with_ansi(false);  // ファイルにはANSIエスケープコードを出力しない
    
    let console_layer = fmt::layer()
        .with_writer(std::io::stdout);
    
    tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .with(env_filter)
        .init();
    
    // 画面表示ログファイルパスを環境変数に設定（後で使用）
    std::env::set_var("DISPLAY_LOG_PATH", display_log_path.to_str().unwrap());
    
    info!("Log file: {}", log_file_path.display());
    info!("Display log: {}", display_log_path.display());
    
    // ビルド番号を取得
    const BUILD_NUMBER: &str = env!("BUILD_NUMBER");
    info!("notif v5 server starting (Windows version {}, build #{})", VERSION, BUILD_NUMBER);
    if !args.verbose {
        info!("ログレベル: {} (notif_common_v5のログは抑制中、--verbose で有効化)", log_level);
    } else {
        info!("ログレベル: {} (詳細モード)", log_level);
    }
    
    // プラットフォーム初期化
    WindowsPlatform::initialize().await?;
    
    // プラットフォーム情報を表示
    let platform_info = WindowsPlatform::get_platform_info();
    info!("Platform: {} ({})", platform_info.distribution, platform_info.kernel);
    
    // Bluetooth利用可能性チェック
    if !WindowsPlatform::check_bluetooth_available().await {
        return Err(notif_common_v5::NotifError::Bluetooth(
            "Bluetooth is not available on this system".to_string()
        ));
    }
    
    // システムリソース情報
    let resources = SystemResources::get();
    info!(
        "System resources: {} MB memory in use, {} CPUs, CPU usage: {:.1}%",
        resources.memory_available_mb,
        resources.cpu_count,
        resources.cpu_usage
    );
    
    // 設定読み込み
    let settings = Settings::new()?;
    settings.validate()?;
    
    let bind_address = format!("{}:{}", settings.server.host, settings.server.port);
    info!("Starting HTTP server on {}", bind_address);
    
    // Bluetoothマネージャー初期化
    let bt_manager = create_bluetooth_manager().await?;
    
    // デバイスのスキャンと接続
    info!("Scanning for devices with prefix: {}", settings.bluetooth.device_name_prefix);
    match bt_manager.scan_and_connect_all().await {
        Ok(devices) => {
            info!("Connected to {} device(s): {:?}", devices.len(), devices);
        }
        Err(e) => {
            tracing::warn!("Failed to connect to devices: {}. Server will start anyway.", e);
        }
    }
    
    // 自動再接続の設定
    bt_manager.set_auto_reconnect(settings.bluetooth.auto_reconnect).await?;
    
    // プロセス優先度を高に設定（オプション）
    if settings.performance.high_priority.unwrap_or(false) {
        WindowsPlatform::set_process_priority(true)?;
        info!("Process priority set to HIGH");
    }
    
    // MCPサポートは常に有効
    info!("MCP support enabled");
    
    // アプリケーション状態の作成
    let bt_manager = Arc::new(bt_manager);
    let app_state = AppState {
        bt_manager: bt_manager.clone(),
        session_manager: SessionManager::new(),
    };
    
    let bt_manager_for_shutdown = bt_manager.clone();
    let bt_manager_data = web::Data::new(bt_manager.clone());
    let app_state_data = web::Data::new(Arc::new(app_state));
    
    // シャットダウンハンドラーの設定
    let shutdown_receiver = WindowsPlatform::setup_shutdown_handler().await?;
    
    // HTTPサーバー構築
    let server = HttpServer::new(move || {
        let cors = if settings.api.cors_origins.contains(&"*".to_string()) {
            Cors::default()
                .allow_any_origin()
                .allow_any_method()
                .allow_any_header()
        } else {
            let mut cors = Cors::default();
            for origin in &settings.api.cors_origins {
                cors = cors.allowed_origin(origin);
            }
            cors.allow_any_method()
                .allow_any_header()
        };
        
        let mut app = App::new()
            .app_data(app_state_data.clone())
            .app_data(bt_manager_data.clone())
            // v5追加: 画像アップロード用にペイロードサイズを10MBに設定
            .app_data(web::PayloadConfig::new(10 * 1024 * 1024)) // 10MB
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .wrap(middleware::NormalizePath::trim())
            // ルートページ（UI）
            .route("/", web::get().to(serve_ui))
            // ヘルスチェック
            .route("/health", web::get().to(|| async { HttpResponse::Ok().body("OK") }))
            
            // v1 API (v1,v2,v3互換エンドポイント)
            .route("/send", web::get().to(
                |query: web::Query<notif_common_v5::api::models::v1::SendQuery>, bt_manager: web::Data<Arc<notif_common_v5::CommonBluetoothManager>>| 
                process_v1_send(query.into_inner(), bt_manager)
            ))
            .route("/send", web::post().to(
                |query: web::Query<notif_common_v5::api::models::v1::SendQuery>, bt_manager: web::Data<Arc<notif_common_v5::CommonBluetoothManager>>| 
                process_v1_send(query.into_inner(), bt_manager)
            ))
            .route("/status", web::get().to(
                |bt_manager: web::Data<Arc<notif_common_v5::CommonBluetoothManager>>| 
                process_v1_status(bt_manager)
            ))
            
            // v2 API (v2,v3互換エンドポイント)
            .route("/api/draw", web::post().to(
                |req: web::Json<notif_common_v5::api::models::v2::DrawQueryRequest>, bt_manager: web::Data<Arc<notif_common_v5::CommonBluetoothManager>>| 
                process_v2_draw_post(req.into_inner(), bt_manager)
            ))
            .route("/api/draw", web::get().to(
                |query: web::Query<std::collections::HashMap<String, String>>, bt_manager: web::Data<Arc<notif_common_v5::CommonBluetoothManager>>| 
                process_v2_draw_query(query.into_inner(), bt_manager)
            ))
            .route("/api/devices", web::get().to(
                |bt_manager: web::Data<Arc<notif_common_v5::CommonBluetoothManager>>| 
                process_v2_devices(bt_manager)
            ))
            .route("/api/health", web::get().to(
                |bt_manager: web::Data<Arc<notif_common_v5::CommonBluetoothManager>>| 
                process_v2_health(bt_manager)
            ))
            .route("/api/batch", web::post().to(
                |req: web::Json<notif_common_v5::api::models::v2::BatchRequest>, bt_manager: web::Data<Arc<notif_common_v5::CommonBluetoothManager>>| 
                process_v2_batch(req.into_inner(), bt_manager)
            ));
        
        // MCPエンドポイント
        app = app
            .route("/mcp", web::post().to(mcp_handler))
            .route("/mcp", web::get().to(mcp_handler));
        
        // v5新機能: 画像アップロードエンドポイント（既存機能に影響なし）
        #[cfg(feature = "http-endpoints")]
        {
            app = app.route("/api/image/upload", web::post().to(
                |payload: actix_multipart::Multipart, bt_manager: web::Data<Arc<notif_common_v5::CommonBluetoothManager>>|
                upload_image(payload, bt_manager)
            ))
            // v5追加: 画像データ直接POSTエンドポイント
            .route("/api/image/post", web::post().to(
                |body: actix_web::web::Bytes, query: actix_web::web::Query<notif_common_v5::api::handlers::ImageUploadParams>, bt_manager: web::Data<Arc<notif_common_v5::CommonBluetoothManager>>|
                post_image(body, query, bt_manager)
            ));
        }
        
        app
    })
    .bind(&bind_address)?
    .run();
    
    info!("Server running at http://{}", bind_address);
    info!("MCP endpoint available at http://{}/mcp", bind_address);
    
    // サーバーとシグナルハンドラーを並行実行
    tokio::select! {
        result = server => {
            // サーバーが終了した場合
            result?;
        }
        _ = shutdown_receiver.wait() => {
            // シャットダウンシグナルを受信
            info!("Received shutdown signal, disconnecting devices...");
            
            // すべてのデバイスを切断
            if let Err(e) = bt_manager_for_shutdown.disconnect_all().await {
                tracing::warn!("Failed to disconnect devices: {}", e);
            }
            
            info!("Server shutting down gracefully");
        }
    }
    
    Ok(())
}

/// v5追加: Web UI提供
async fn serve_ui() -> HttpResponse {
    let html = include_str!("../../common-v5/src/ui.html");
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}