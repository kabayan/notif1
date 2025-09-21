//! MCPセッション管理

use super::{ClientCapabilities, JsonRpcMessage};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// セッション状態
#[derive(Debug, Clone)]
pub struct SessionState {
    pub id: String,
    pub client_capabilities: Option<ClientCapabilities>,
    pub event_counter: u64,
    pub pending_messages: Vec<JsonRpcMessage>,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub initialized: bool,
}

impl SessionState {
    /// 新しいセッションを作成
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            client_capabilities: None,
            event_counter: 0,
            pending_messages: Vec::new(),
            created_at: now,
            last_activity: now,
            initialized: false,
        }
    }

    /// 次のメッセージを取得
    pub async fn get_next_message(&self, after_id: u64) -> Option<JsonRpcMessage> {
        if self.event_counter > after_id && !self.pending_messages.is_empty() {
            self.pending_messages.get((after_id as usize).saturating_sub(self.event_counter as usize))
                .cloned()
        } else {
            None
        }
    }

    /// メッセージを追加
    pub fn add_message(&mut self, message: JsonRpcMessage) {
        self.pending_messages.push(message);
        self.event_counter += 1;
        self.last_activity = Utc::now();
    }

    /// アクティビティを更新
    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
    }
}

/// セッションマネージャー
#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<Mutex<HashMap<String, SessionState>>>,
}

impl SessionManager {
    /// 新しいセッションマネージャーを作成
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// セッションを作成
    pub fn create_session(&self) -> SessionState {
        let session = SessionState::new();
        let session_clone = session.clone();
        
        tokio::spawn({
            let sessions = self.sessions.clone();
            let session_id = session.id.clone();
            async move {
                let mut sessions = sessions.lock().await;
                sessions.insert(session_id, session);
            }
        });
        
        session_clone
    }

    /// セッションを取得
    pub async fn get_session(&self, session_id: &str) -> Option<SessionState> {
        let sessions = self.sessions.lock().await;
        sessions.get(session_id).cloned()
    }

    /// セッションを更新
    pub async fn update_session(&self, session: SessionState) {
        let mut sessions = self.sessions.lock().await;
        sessions.insert(session.id.clone(), session);
    }

    /// セッションを削除
    pub async fn remove_session(&self, session_id: &str) {
        let mut sessions = self.sessions.lock().await;
        sessions.remove(session_id);
    }

    /// 期限切れセッションをクリーンアップ（1時間以上アクティビティがない）
    pub async fn cleanup_expired(&self) {
        let mut sessions = self.sessions.lock().await;
        let now = Utc::now();
        sessions.retain(|_, session| {
            (now - session.last_activity).num_hours() < 1
        });
    }
}