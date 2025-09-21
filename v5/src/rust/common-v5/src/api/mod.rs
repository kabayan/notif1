//! 共通APIモジュール

pub mod models;
pub mod handlers;

// 再エクスポート
pub use models::{v1, v2, ApiResponse, ApiError};
pub use handlers::{
    process_v1_send,
    process_v1_status,
    process_v2_draw,
    process_v2_draw_query,
    process_v2_draw_post,
    process_v2_devices,
    process_v2_health,
    process_v2_batch,
    ImageUploadParams,
};

// v5画像アップロード機能（http-endpoints有効時のみ）
#[cfg(feature = "http-endpoints")]
pub use handlers::{
    upload_image,
    post_image,
};