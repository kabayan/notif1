//! MCPハンドラー実装

use super::{
    AppState, ClientCapabilities, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
    ServerCapabilities, ServerInfo, SessionManager, ToolsCapability, ResourcesCapability,
    PromptsCapability, INTERNAL_ERROR, INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND,
};
use actix_web::{web, HttpRequest, HttpResponse, Error};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error, info};

/// MCPハンドラー
pub async fn mcp_handler(
    req: HttpRequest,
    data: web::Data<Arc<AppState>>,
    payload: web::Bytes,
) -> Result<HttpResponse, Error> {
    info!("MCP handler called with method: {}", req.method());
    
    // HTTPメソッドによる処理分岐
    match req.method().as_str() {
        "POST" => {
            info!("Handling JSON-RPC request");
            handle_json_rpc(req, data, payload).await
        },
        "GET" => {
            info!("Handling SSE stream request");
            handle_sse_stream(req, data).await
        },
        _ => Ok(HttpResponse::MethodNotAllowed().finish()),
    }
}

/// JSON-RPC処理
async fn handle_json_rpc(
    req: HttpRequest,
    data: web::Data<Arc<AppState>>,
    payload: web::Bytes,
) -> Result<HttpResponse, Error> {
    // セッションIDの取得または作成
    let session_id = req
        .headers()
        .get("Mcp-Session-Id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let session = data.session_manager.create_session();
            session.id.clone()
        });

    // JSON-RPCリクエストのパース
    let request: JsonRpcRequest = match serde_json::from_slice(&payload) {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to parse JSON-RPC request: {}", e);
            return Ok(HttpResponse::BadRequest().json(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(JsonRpcError {
                    code: INVALID_REQUEST,
                    message: "Invalid JSON-RPC request".to_string(),
                    data: Some(json!({ "error": e.to_string() })),
                }),
                id: json!(null),
            }));
        }
    };

    debug!("MCP request: method={}, id={:?}", request.method, request.id);

    // メソッドに応じた処理
    let result = match request.method.as_str() {
        "initialize" => handle_initialize(&data, request.params).await,
        "initialized" => handle_initialized(&data, &session_id).await,
        "tools/list" => super::tools::list().await,
        "tools/call" => handle_tool_call(&data, request.params).await,
        "resources/list" => super::resources::list().await,
        "resources/read" => handle_resource_read(&data, request.params).await,
        "prompts/list" => super::prompts::list().await,
        "prompts/get" => handle_prompt_get(&data, request.params).await,
        _ => Err(JsonRpcError {
            code: METHOD_NOT_FOUND,
            message: format!("Method not found: {}", request.method),
            data: None,
        }),
    };

    // レスポンスの作成
    let response = match result {
        Ok(value) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(value),
            error: None,
            id: request.id,
        },
        Err(error) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(error),
            id: request.id,
        },
    };

    // セッションIDをヘッダーに含める
    Ok(HttpResponse::Ok()
        .insert_header(("Mcp-Session-Id", session_id))
        .json(response))
}

/// SSEストリーム処理（簡略版）
async fn handle_sse_stream(
    req: HttpRequest,
    _data: web::Data<Arc<AppState>>,
) -> Result<HttpResponse, Error> {
    // セッションIDの取得
    let session_id = req
        .headers()
        .get("Mcp-Session-Id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    if session_id.is_none() {
        return Ok(HttpResponse::BadRequest().body("Missing Mcp-Session-Id header"));
    }

    let session_id = session_id.unwrap();
    info!("Starting SSE stream for session: {}", session_id);

    // 簡略版：すぐに空のレスポンスを返す
    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .body("data: {\"type\":\"keepalive\"}\n\n"))
}

/// initializeハンドラー
async fn handle_initialize(
    _data: &web::Data<Arc<AppState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let client_capabilities = params
        .as_ref()
        .and_then(|p| p.get("capabilities"))
        .and_then(|c| serde_json::from_value::<ClientCapabilities>(c.clone()).ok());

    debug!("MCP initialize with capabilities: {:?}", client_capabilities);

    Ok(json!({
        "protocolVersion": "2025-03-26",
        "capabilities": ServerCapabilities {
            tools: Some(ToolsCapability { list_changed: false }),
            resources: Some(ResourcesCapability { 
                subscribe: false,
                list_changed: false 
            }),
            prompts: Some(PromptsCapability { list_changed: false }),
        },
        "serverInfo": ServerInfo {
            name: "notif-v4-mcp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
    }))
}

/// initializedハンドラー
async fn handle_initialized(
    data: &web::Data<Arc<AppState>>,
    session_id: &str,
) -> Result<Value, JsonRpcError> {
    info!("MCP session initialized: {}", session_id);
    
    // セッションを初期化済みとしてマーク
    if let Some(mut session) = data.session_manager.get_session(session_id).await {
        session.initialized = true;
        data.session_manager.update_session(session).await;
    }
    
    Ok(json!({}))
}

/// ツール呼び出しハンドラー
async fn handle_tool_call(
    data: &web::Data<Arc<AppState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError {
        code: INVALID_PARAMS,
        message: "Missing params".to_string(),
        data: None,
    })?;

    let name = params
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| JsonRpcError {
            code: INVALID_PARAMS,
            message: "Missing tool name".to_string(),
            data: None,
        })?;

    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    match name {
        "send" => super::tools::send::execute(arguments, data.clone()).await,
        "draw" => super::tools::draw::execute(arguments, data.clone()).await,
        "status" => super::tools::status::execute(arguments, data.clone()).await,
        "devices.list" => super::tools::devices::list(arguments, data.clone()).await,
        "devices.connect" => super::tools::devices::connect(arguments, data.clone()).await,
        "devices.disconnect" => super::tools::devices::disconnect(arguments, data.clone()).await,
        _ => Err(JsonRpcError {
            code: METHOD_NOT_FOUND,
            message: format!("Tool not found: {}", name),
            data: None,
        }),
    }
}

/// リソース読み取りハンドラー
async fn handle_resource_read(
    data: &web::Data<Arc<AppState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError {
        code: INVALID_PARAMS,
        message: "Missing params".to_string(),
        data: None,
    })?;

    let uri = params
        .get("uri")
        .and_then(|u| u.as_str())
        .ok_or_else(|| JsonRpcError {
            code: INVALID_PARAMS,
            message: "Missing resource URI".to_string(),
            data: None,
        })?;

    match uri {
        "notif://device_state" => super::resources::device_state::read(data.clone()).await,
        "notif://connection_status" => super::resources::connection_status::read(data.clone()).await,
        _ => Err(JsonRpcError {
            code: METHOD_NOT_FOUND,
            message: format!("Resource not found: {}", uri),
            data: None,
        }),
    }
}

/// プロンプト取得ハンドラー
async fn handle_prompt_get(
    data: &web::Data<Arc<AppState>>,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError {
        code: INVALID_PARAMS,
        message: "Missing params".to_string(),
        data: None,
    })?;

    let name = params
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| JsonRpcError {
            code: INVALID_PARAMS,
            message: "Missing prompt name".to_string(),
            data: None,
        })?;

    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    match name {
        "status_display" => super::prompts::status_display::get(arguments, data.clone()).await,
        "progress_bar" => super::prompts::progress_bar::get(arguments, data.clone()).await,
        _ => Err(JsonRpcError {
            code: METHOD_NOT_FOUND,
            message: format!("Prompt not found: {}", name),
            data: None,
        }),
    }
}