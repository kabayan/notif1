//! 画像フォーマット検出

/// 画像フォーマットをマジックバイトから検出
pub fn detect_format(data: &[u8]) -> String {
    if data.len() < 4 {
        return "unknown".to_string();
    }
    
    match &data[0..4] {
        // JPEG: FF D8 FF
        [0xFF, 0xD8, 0xFF, _] => "jpeg".to_string(),
        
        // PNG: 89 50 4E 47
        [0x89, 0x50, 0x4E, 0x47] => "png".to_string(),
        
        // GIF: 47 49 46 38
        [0x47, 0x49, 0x46, 0x38] => "gif".to_string(),
        
        // BMP: 42 4D
        [0x42, 0x4D, _, _] => "bmp".to_string(),
        
        _ => "unknown".to_string(),
    }
}

/// MIMEタイプ検証（セキュリティ対策）
pub fn validate_image_mime(data: &[u8]) -> bool {
    matches!(detect_format(data).as_str(), "jpeg" | "png" | "gif" | "bmp")
}

/// ファイルサイズ制限チェック（10MB）
pub fn validate_size(data: &[u8]) -> Result<(), crate::error::NotifError> {
    const MAX_SIZE: usize = 10 * 1024 * 1024; // 10MB
    
    if data.len() > MAX_SIZE {
        return Err(crate::error::NotifError::ImageTooLarge(data.len(), MAX_SIZE));
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jpeg_detection() {
        let jpeg_header = vec![0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(detect_format(&jpeg_header), "jpeg");
    }

    #[test]
    fn test_png_detection() {
        let png_header = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(detect_format(&png_header), "png");
    }

    #[test]
    fn test_gif_detection() {
        let gif_header = vec![0x47, 0x49, 0x46, 0x38, 0x39, 0x61];
        assert_eq!(detect_format(&gif_header), "gif");
    }

    #[test]
    fn test_bmp_detection() {
        let bmp_header = vec![0x42, 0x4D, 0x36, 0x58];
        assert_eq!(detect_format(&bmp_header), "bmp");
    }

    #[test]
    fn test_unknown_format() {
        let unknown = vec![0x00, 0x00, 0x00, 0x00];
        assert_eq!(detect_format(&unknown), "unknown");
    }

    #[test]
    fn test_size_validation() {
        let small_data = vec![0u8; 1000];
        assert!(validate_size(&small_data).is_ok());

        let large_data = vec![0u8; 11 * 1024 * 1024]; // 11MB
        assert!(validate_size(&large_data).is_err());
    }

    #[test]
    fn test_mime_validation() {
        let jpeg_data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        assert!(validate_image_mime(&jpeg_data));
        
        let unknown_data = vec![0x00, 0x00, 0x00, 0x00];
        assert!(!validate_image_mime(&unknown_data));
    }

    #[test]
    fn test_short_data() {
        let short_data = vec![0xFF, 0xD8]; // 2バイトのみ
        assert_eq!(detect_format(&short_data), "unknown");
    }
}