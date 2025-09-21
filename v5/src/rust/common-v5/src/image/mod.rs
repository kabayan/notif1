//! 画像処理モジュール（v5新機能）
//! v4機能には一切影響しない独立した実装

pub mod processor;
pub mod formats; 
pub mod rgb565;

// 公開API
pub use processor::ImageProcessor;
pub use rgb565::to_rgb565;

/// 画像処理結果
#[derive(Debug, Clone)]
pub struct ProcessedImage {
    pub rgb565_data: Vec<u16>,    // RGB565形式データ
    pub width: u16,
    pub height: u16,
    pub original_format: String,
    pub processing_time_ms: u64,
}

/// 画像タイル（BLE送信用の小分割データ）
#[derive(Debug, Clone)]
pub struct ImageTile {
    pub x: u8,                   // タイル開始X座標
    pub y: u8,                   // タイル開始Y座標
    pub width: u8,               // タイル幅
    pub height: u8,              // タイル高さ
    pub rgb565_data: Vec<u16>,   // RGB565データ
}

/// リサイズモード（元仕様準拠）
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FitMode {
    Contain,  // アスペクト比保持、全体表示
    Cover,    // アスペクト比保持、領域埋める  
    Fill,     // アスペクト比無視、引き延ばし
    #[serde(rename = "scale_down")]
    ScaleDown, // 元のサイズより小さい場合のみリサイズ
    None,     // そのまま
}

impl Default for FitMode {
    fn default() -> Self {
        FitMode::Contain
    }
}

impl std::str::FromStr for FitMode {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "contain" => Ok(FitMode::Contain),
            "cover" => Ok(FitMode::Cover),
            "fill" => Ok(FitMode::Fill),
            "scale_down" => Ok(FitMode::ScaleDown),
            "none" => Ok(FitMode::None),
            _ => Err(format!("Invalid fit mode: {}", s))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fit_mode_from_str() {
        assert_eq!("contain".parse::<FitMode>().unwrap(), FitMode::Contain);
        assert_eq!("cover".parse::<FitMode>().unwrap(), FitMode::Cover);
        assert_eq!("fill".parse::<FitMode>().unwrap(), FitMode::Fill);
        assert_eq!("scale_down".parse::<FitMode>().unwrap(), FitMode::ScaleDown);
        assert_eq!("none".parse::<FitMode>().unwrap(), FitMode::None);
    }

    #[test]
    fn test_image_processor_creation() {
        let processor = ImageProcessor::new();
        // 現段階では作成のみテスト
        assert!(true); // プレースホルダー
    }
}