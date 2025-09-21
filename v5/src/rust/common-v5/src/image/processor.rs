//! 画像処理パイプライン

use crate::image::{ProcessedImage, FitMode, ImageTile};
use crate::error::{Result, NotifError};
use image::DynamicImage;
use std::time::Instant;
use tracing::info;

pub struct ImageProcessor;

impl ImageProcessor {
    pub fn new() -> Self {
        Self
    }
    
    /// バイトデータから画像を処理してRGB565に変換
    pub fn process_image(
        &self,
        image_data: Vec<u8>,
        target_size: (u16, u16),    // (128, 128)
        fit_mode: FitMode,
    ) -> Result<ProcessedImage> {
        let start = Instant::now();
        
        // 1. サイズ検証
        super::formats::validate_size(&image_data)?;
        
        // 2. フォーマット検出・検証
        let original_format = super::formats::detect_format(&image_data);
        if original_format == "unknown" {
            return Err(NotifError::UnsupportedFormat("未知の画像形式です".to_string()).into());
        }
        
        // 3. デコード
        let img = image::load_from_memory(&image_data)
            .map_err(|e| NotifError::ImageProcessing(format!("画像のデコードに失敗しました: {}", e)))?;
        
        // 4. リサイズ
        let resized = self.resize_image(img, target_size, fit_mode)?;
        
        // 5. RGB565変換
        let rgb565_data = super::rgb565::to_rgb565(&resized);
        
        let processing_time = start.elapsed().as_millis() as u64;
        
        Ok(ProcessedImage {
            rgb565_data,
            width: target_size.0,
            height: target_size.1,
            original_format,
            processing_time_ms: processing_time,
        })
    }
    
    /// URL から画像を取得して処理
    #[cfg(feature = "http-endpoints")]
    pub async fn process_from_url(
        &self,
        url: &str,
        target_size: (u16, u16),
        fit_mode: FitMode,
        timeout_seconds: u64,
    ) -> Result<ProcessedImage> {
        // SSRF対策
        self.validate_url(url)?;
        
        // HTTP取得（タイムアウト付き）
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|e| NotifError::ImageProcessing(format!("HTTPクライアントエラー: {}", e)))?;
            
        let response = client.get(url).send().await
            .map_err(|e| NotifError::ImageProcessing(format!("HTTPリクエストに失敗しました: {}", e)))?;
            
        if !response.status().is_success() {
            return Err(NotifError::ImageProcessing(format!("HTTPエラー: {}", response.status())).into());
        }
        
        let image_data = response.bytes().await
            .map_err(|e| NotifError::ImageProcessing(format!("レスポンスの読み取りに失敗しました: {}", e)))?
            .to_vec();
        
        // 画像処理
        self.process_image(image_data, target_size, fit_mode)
    }
    
    fn resize_image(&self, img: DynamicImage, target: (u16, u16), mode: FitMode) -> Result<DynamicImage> {
        let (target_width, target_height) = (target.0 as u32, target.1 as u32);
        
        let resized = match mode {
            FitMode::None => {
                // そのまま128x128にクロップ（中央から）
                let (orig_width, orig_height) = (img.width(), img.height());
                let x = orig_width.saturating_sub(target_width) / 2;
                let y = orig_height.saturating_sub(target_height) / 2;
                img.crop_imm(x, y, target_width.min(orig_width), target_height.min(orig_height))
            },
            
            FitMode::Fill => {
                // アスペクト比無視、強制リサイズ
                img.resize_exact(target_width, target_height, image::imageops::FilterType::Lanczos3)
            },
            
            FitMode::Contain => {
                // アスペクト比保持、全体表示（黒い背景で中央配置）
                let resized = img.resize(target_width, target_height, image::imageops::FilterType::Lanczos3);
                
                // 128x128の黒い背景を作成
                let mut background = DynamicImage::new_rgb8(target_width, target_height);
                
                // リサイズした画像を中央に配置
                let (resized_width, resized_height) = (resized.width(), resized.height());
                let x_offset = (target_width - resized_width) / 2;
                let y_offset = (target_height - resized_height) / 2;
                
                image::imageops::overlay(&mut background, &resized, x_offset as i64, y_offset as i64);
                background
            },
            
            FitMode::Cover => {
                // アスペクト比保持、領域を埋める
                img.resize_to_fill(target_width, target_height, image::imageops::FilterType::Lanczos3)
            },
            
            FitMode::ScaleDown => {
                // 元のサイズより小さい場合のみリサイズ（黒い背景で中央配置）
                let (orig_width, orig_height) = (img.width(), img.height());
                if orig_width > target_width || orig_height > target_height {
                    // Containと同じ処理
                    let resized = img.resize(target_width, target_height, image::imageops::FilterType::Lanczos3);
                    let mut background = DynamicImage::new_rgb8(target_width, target_height);
                    let (resized_width, resized_height) = (resized.width(), resized.height());
                    let x_offset = (target_width - resized_width) / 2;
                    let y_offset = (target_height - resized_height) / 2;
                    image::imageops::overlay(&mut background, &resized, x_offset as i64, y_offset as i64);
                    background
                } else {
                    // 小さい画像は黒い背景の中央に配置
                    let mut background = DynamicImage::new_rgb8(target_width, target_height);
                    let x_offset = (target_width - orig_width) / 2;
                    let y_offset = (target_height - orig_height) / 2;
                    image::imageops::overlay(&mut background, &img, x_offset as i64, y_offset as i64);
                    background
                }
            },
        };
        
        Ok(resized)
    }
    
    /// SSRF対策: URLの検証
    #[cfg(feature = "http-endpoints")]
    fn validate_url(&self, url: &str) -> Result<()> {
        let parsed = url.parse::<reqwest::Url>()
            .map_err(|_| NotifError::ImageProcessing("無効なURLです".to_string()))?;
        
        // プロトコル制限
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(NotifError::ImageProcessing("HTTP/HTTPSのみ許可されています".to_string()).into());
        }
        
        // プライベートIP拒否
        if let Some(host) = parsed.host_str() {
            if self.is_private_ip(host)? {
                return Err(NotifError::ImageProcessing("プライベートIPアドレスは許可されていません".to_string()).into());
            }
        }
        
        Ok(())
    }
    
    /// プライベートIP判定
    #[cfg(feature = "http-endpoints")]
    fn is_private_ip(&self, host: &str) -> Result<bool> {
        use std::net::IpAddr;
        
        let ip: IpAddr = match host.parse() {
            Ok(ip) => ip,
            Err(_) => {
                // ホスト名の場合は DNS 解決が必要だが、簡易実装では許可
                return Ok(false);
            }
        };
        
        match ip {
            IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();
                let is_private = 
                    octets[0] == 10 ||                                    // 10.0.0.0/8
                    (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31) || // 172.16.0.0/12
                    (octets[0] == 192 && octets[1] == 168) ||             // 192.168.0.0/16
                    octets[0] == 127;                                     // 127.0.0.0/8 (localhost)
                
                Ok(is_private)
            },
            IpAddr::V6(_) => {
                // IPv6のプライベート範囲チェックは簡易実装では省略
                Ok(false)
            }
        }
    }
    
    /// 大きな画像をBLE送信可能なタイルに分割
    /// タイルサイズ: 32x32 (2KB) でBLE制限 (512バイト) に対応
    pub fn split_image_to_tiles(
        &self, 
        rgb565_data: &[u16], 
        image_width: u16, 
        image_height: u16,
        tile_size: u16
    ) -> Vec<ImageTile> {
        let mut tiles = Vec::new();
        
        // タイル数を計算（切り上げ）
        let tiles_x = (image_width + tile_size - 1) / tile_size;
        let tiles_y = (image_height + tile_size - 1) / tile_size;
        
        info!("画像分割: {}x{}を{}x{}のタイルに分割（{}x{}タイル）", 
              image_width, image_height, tile_size, tile_size, tiles_x, tiles_y);
        
        for tile_y in 0..tiles_y {
            for tile_x in 0..tiles_x {
                let start_x = tile_x * tile_size;
                let start_y = tile_y * tile_size;
                let end_x = std::cmp::min(start_x + tile_size, image_width);
                let end_y = std::cmp::min(start_y + tile_size, image_height);
                
                let actual_width = end_x - start_x;
                let actual_height = end_y - start_y;
                
                // タイルデータを抽出
                let mut tile_data = Vec::with_capacity((actual_width * actual_height) as usize);
                
                for y in start_y..end_y {
                    for x in start_x..end_x {
                        let idx = (y * image_width + x) as usize;
                        if idx < rgb565_data.len() {
                            tile_data.push(rgb565_data[idx]);
                        } else {
                            tile_data.push(0); // パディング
                        }
                    }
                }
                
                tiles.push(ImageTile {
                    x: start_x as u8,
                    y: start_y as u8,
                    width: actual_width as u8,
                    height: actual_height as u8,
                    rgb565_data: tile_data,
                });
            }
        }
        
        info!("画像分割完了: {}個のタイル生成", tiles.len());
        tiles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_creation() {
        let processor = ImageProcessor::new();
        // 作成テスト
        assert!(true);
    }

    #[test] 
    #[cfg(feature = "http-endpoints")]
    fn test_url_validation() {
        let processor = ImageProcessor::new();
        
        // 有効なURL
        assert!(processor.validate_url("https://example.com/image.jpg").is_ok());
        
        // プライベートIP（拒否）
        assert!(processor.validate_url("http://192.168.1.1/image.jpg").is_err());
        assert!(processor.validate_url("http://10.0.0.1/image.jpg").is_err());
        assert!(processor.validate_url("http://127.0.0.1/image.jpg").is_err());
        
        // 不正プロトコル（拒否）
        assert!(processor.validate_url("ftp://example.com/image.jpg").is_err());
        assert!(processor.validate_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn test_image_processing() {
        let processor = ImageProcessor::new();
        
        // 動的に1x1の赤色PNG画像を生成
        let png_data = create_valid_png_data();
        
        let result = processor.process_image(
            png_data, 
            (128, 128), 
            FitMode::Contain
        );
        
        match &result {
            Ok(_) => {},
            Err(e) => panic!("画像処理に失敗: {:?}", e),
        }
        let processed = result.unwrap();
        assert_eq!(processed.width, 128);
        assert_eq!(processed.height, 128);
        assert_eq!(processed.original_format, "png");
        assert!(processed.processing_time_ms >= 0);
        assert!(!processed.rgb565_data.is_empty());
    }

    #[test]
    fn test_unsupported_format() {
        let processor = ImageProcessor::new();
        
        // 無効な画像データ
        let invalid_data = vec![0u8; 100];
        
        let result = processor.process_image(
            invalid_data,
            (128, 128),
            FitMode::Contain
        );
        
        assert!(result.is_err());
    }

    #[test]
    fn test_large_file_rejection() {
        let processor = ImageProcessor::new();
        
        // 11MBの大きなファイル
        let large_data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header
        let mut large_file = large_data;
        large_file.extend(vec![0u8; 11 * 1024 * 1024]); // 11MB
        
        let result = processor.process_image(
            large_file,
            (128, 128),
            FitMode::Contain
        );
        
        assert!(result.is_err());
    }

    #[test] 
    fn test_resize_modes() {
        let processor = ImageProcessor::new();
        let png_data = create_valid_png_data();
        
        // 各リサイズモードのテスト
        let modes = vec![FitMode::Contain, FitMode::Fill, FitMode::Cover, FitMode::ScaleDown, FitMode::None];
        
        for mode in modes {
            let result = processor.process_image(
                png_data.clone(),
                (64, 64),
                mode
            );
            match &result {
                Ok(_) => {},
                Err(e) => panic!("リサイズモード {:?} でエラー: {:?}", mode, e),
            }
        }
    }

    // テスト用の有効なPNG画像データ作成（1x1の赤色画像）
    fn create_valid_png_data() -> Vec<u8> {
        use image::{Rgba, RgbaImage};
        use std::io::Cursor;
        
        // 1x1の赤色画像を作成
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255])); // 赤色
        let dynamic_img = image::DynamicImage::ImageRgba8(img);
        
        // PNGとしてエンコード
        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        dynamic_img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        
        buf
    }
}