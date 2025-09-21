//! notif v5 Linux版サーバー

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use notif_common_v5::{
    BluetoothManager, Result, Settings, VERSION,
    api::{process_v1_send, process_v1_status, process_v2_draw, process_v2_draw_query, process_v2_draw_post, process_v2_devices, process_v2_health, process_v2_batch},
    AppState, SessionManager, mcp_handler,
};

// v5画像アップロード機能
use notif_common_v5::api::{upload_image, post_image};

mod bluetooth_impl;
mod platform;

// v5 Web UI提供
async fn serve_ui() -> HttpResponse {
    let html = include_str!("../../common-v5/src/ui.html");
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

use bluetooth_impl::create_bluetooth_manager;
use platform::{LinuxPlatform, SystemResources};

use std::env;
use std::collections::HashMap;
use std::sync::Arc;

#[actix_web::main]
async fn main() -> Result<()> {
    // ログ初期化（v2と同じシンプルな方式）
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");
    
    // ビルド番号を取得
    const BUILD_NUMBER: &str = env!("BUILD_NUMBER");
    info!("notif v5 server starting (Linux version {}, build #{})", VERSION, BUILD_NUMBER);
    
    // プラットフォーム初期化
    LinuxPlatform::initialize().await?;
    
    // プラットフォーム情報を表示
    let platform_info = LinuxPlatform::get_platform_info();
    info!("Platform: {} ({})", platform_info.distribution, platform_info.kernel);
    
    // Bluetooth利用可能性チェック
    info!("Checking Bluetooth availability...");
    if !LinuxPlatform::check_bluetooth_available().await {
        return Err(notif_common_v5::NotifError::Bluetooth(
            "Bluetooth is not available on this system".to_string()
        ));
    }
    
    // システムリソース情報
    let resources = SystemResources::get();
    info!(
        "System resources: {} MB memory available, {} CPUs, load average: {:.2}/{:.2}/{:.2}",
        resources.memory_available_mb,
        resources.cpu_count,
        resources.load_average.0,
        resources.load_average.1,
        resources.load_average.2
    );
    
    // 設定読み込み
    info!("Loading configuration...");
    let settings = Settings::new()?;
    info!("Configuration loaded successfully");
    settings.validate()?;
    info!("Configuration validated successfully");
    
    let bind_address = format!("{}:{}", settings.server.host, settings.server.port);
    info!("Starting HTTP server on {}", bind_address);
    
    // Bluetoothマネージャー初期化
    info!("Initializing Bluetooth manager...");
    let bt_manager = create_bluetooth_manager().await?;
    info!("Bluetooth manager initialized successfully");
    
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
    
    // アプリケーション状態を作成（MCP対応）
    let bt_manager = Arc::new(bt_manager);
    let app_state = Arc::new(AppState {
        bt_manager: bt_manager.clone(),
        session_manager: SessionManager::new(),
    });
    
    let app_state_data = web::Data::new(app_state.clone());
    let bt_manager_data = web::Data::new(bt_manager.clone());
    let bt_manager_for_shutdown = bt_manager.clone();
    
    // シャットダウンハンドラーの設定
    let shutdown_receiver = LinuxPlatform::setup_shutdown_handler().await?;
    
    // 拡張シグナルハンドラー
    platform::setup_signal_handlers().await?;
    
    // HTTPサーバー構築
    let server = HttpServer::new(move || {
        App::new()
            .app_data(app_state_data.clone())
            .app_data(bt_manager_data.clone())
            .wrap(middleware::Logger::default())
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
                    .max_age(3600)
            )
            // Web UI（v5追加）
            .route("/", web::get().to(serve_ui))
            // Test endpoint
            .route("/test", web::get().to(|| async { HttpResponse::Ok().body("Test endpoint works!") }))
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
                |query: web::Query<HashMap<String, String>>, bt_manager: web::Data<Arc<notif_common_v5::CommonBluetoothManager>>| 
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
            ))
            
            // MCP エンドポイント
            .route("/mcp", web::post().to(mcp_handler))
            .route("/mcp", web::get().to(mcp_handler))
            
            // v5画像アップロードエンドポイント
            .route("/api/image/upload", web::post().to(
                |payload: actix_multipart::Multipart, bt_manager: web::Data<Arc<notif_common_v5::CommonBluetoothManager>>| 
                upload_image(payload, bt_manager)
            ))
            .route("/api/image/post", web::post().to(
                |body: web::Bytes, query: web::Query<notif_common_v5::api::ImageUploadParams>, bt_manager: web::Data<Arc<notif_common_v5::CommonBluetoothManager>>| 
                post_image(body, query, bt_manager)
            ))
    })
    .bind(&bind_address)?
    .run();
    
    info!("Server running at http://{}", bind_address);
    info!("MCP endpoint available at http://{}/mcp", bind_address);
    
    // サーバーをグレースフルシャットダウンで実行
    let server_handle = server.handle();
    
    // シャットダウンハンドラー
    let shutdown_task = async move {
        shutdown_receiver.wait().await;
        info!("Shutdown signal received, stopping server...");
        server_handle.stop(true).await;
        bt_manager_for_shutdown.disconnect_all().await.ok();
        info!("Server stopped");
    };
    
    // サーバーとシャットダウンタスクを並行実行
    tokio::select! {
        result = server => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = shutdown_task => {
            info!("Shutdown completed");
        }
    }
    
    Ok(())
}