//! RGB565変換処理

use image::DynamicImage;

/// RGB888からRGB565への高精度変換
pub fn to_rgb565(img: &DynamicImage) -> Vec<u16> {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut result = Vec::with_capacity((width * height) as usize);
    
    for pixel in rgba.pixels() {
        let r = pixel[0];
        let g = pixel[1]; 
        let b = pixel[2];
        let _a = pixel[3]; // アルファチャンネルは無視（RGB565は透過非対応）
        
        // 高精度変換（丸め処理込み）
        let r5 = ((r as u16 * 31 + 127) / 255) as u16; // 8bit -> 5bit
        let g6 = ((g as u16 * 63 + 127) / 255) as u16; // 8bit -> 6bit  
        let b5 = ((b as u16 * 31 + 127) / 255) as u16; // 8bit -> 5bit
        
        let rgb565 = (r5 << 11) | (g6 << 5) | b5;
        result.push(rgb565);
    }
    
    result
}

/// RGB565をバイト配列に変換（AtomS3転送用）
pub fn rgb565_to_bytes(rgb565_data: &[u16]) -> Vec<u8> {
    let mut result = Vec::with_capacity(rgb565_data.len() * 2);
    
    for &pixel in rgb565_data {
        // リトルエンディアン形式
        result.push((pixel & 0xFF) as u8);      // 下位バイト
        result.push((pixel >> 8) as u8);        // 上位バイト
    }
    
    result
}

/// RGB565からRGB888への逆変換（デバッグ用）
#[cfg(test)]
pub fn rgb565_to_rgb888(rgb565: u16) -> (u8, u8, u8) {
    let r5 = (rgb565 >> 11) & 0x1F;
    let g6 = (rgb565 >> 5) & 0x3F;
    let b5 = rgb565 & 0x1F;
    
    // 5bit -> 8bit, 6bit -> 8bit 拡張
    let r = ((r5 * 255 + 15) / 31) as u8;
    let g = ((g6 * 255 + 31) / 63) as u8;
    let b = ((b5 * 255 + 15) / 31) as u8;
    
    (r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn test_rgb565_conversion() {
        // 1x1の赤色画像を作成
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255])); // 赤
        let dynamic_img = DynamicImage::ImageRgba8(img);
        
        let rgb565_data = to_rgb565(&dynamic_img);
        assert_eq!(rgb565_data.len(), 1);
        
        // 赤色のRGB565値確認（R=31, G=0, B=0 -> 0xF800）
        assert_eq!(rgb565_data[0], 0xF800);
    }

    #[test]
    fn test_debug_red_128x128_tile() {
        println!("🔍 Debug: Testing 128x128 red image conversion and tile extraction");
        
        // Create 128x128 pure red image
        let mut img = RgbaImage::new(128, 128);
        for y in 0..128 {
            for x in 0..128 {
                img.put_pixel(x, y, Rgba([255, 0, 0, 255])); // Pure red #FF0000
            }
        }
        let dynamic_img = DynamicImage::ImageRgba8(img);
        
        // Convert to RGB565
        let rgb565_data = to_rgb565(&dynamic_img);
        println!("Total pixels converted: {}", rgb565_data.len());
        
        // Check first few pixels
        for i in 0..10.min(rgb565_data.len()) {
            println!("Pixel {}: 0x{:04X}", i, rgb565_data[i]);
            assert_eq!(rgb565_data[i], 0xF800, "Pixel {} should be pure red (0xF800)", i);
        }
        
        // Extract 8x8 tile (similar to BLE transmission)
        let tile_size = 8;
        let mut tile_data = Vec::new();
        
        for y in 0..tile_size {
            for x in 0..tile_size {
                let idx = y * 128 + x;
                tile_data.push(rgb565_data[idx]);
            }
        }
        
        println!("Tile size: {} pixels", tile_data.len());
        assert_eq!(tile_data.len(), 64); // 8x8 = 64 pixels
        
        // Convert to bytes (BLE format)
        let tile_bytes = rgb565_to_bytes(&tile_data);
        println!("Tile bytes length: {}", tile_bytes.len());
        assert_eq!(tile_bytes.len(), 128); // 64 pixels * 2 bytes = 128 bytes
        
        // Check byte pattern
        println!("First 10 bytes: {:02X?}", &tile_bytes[0..10]);
        
        // Each RGB565 value 0xF800 should become bytes [0x00, 0xF8] in little-endian
        for i in (0..20).step_by(2) {
            let low_byte = tile_bytes[i];
            let high_byte = tile_bytes[i + 1];
            let rgb565_value = u16::from_le_bytes([low_byte, high_byte]);
            
            println!("Bytes[{}:{}]: [0x{:02X}, 0x{:02X}] -> RGB565: 0x{:04X}", 
                     i, i+1, low_byte, high_byte, rgb565_value);
            
            assert_eq!(rgb565_value, 0xF800, "RGB565 value should be 0xF800");
            assert_eq!(low_byte, 0x00, "Low byte should be 0x00 for 0xF800");
            assert_eq!(high_byte, 0xF8, "High byte should be 0xF8 for 0xF800");
        }
        
        // Check if we get the problematic 0xE8E4 pattern anywhere
        let mut found_e8e4 = false;
        for i in (0..tile_bytes.len()).step_by(2) {
            if i + 1 < tile_bytes.len() {
                let value = u16::from_le_bytes([tile_bytes[i], tile_bytes[i + 1]]);
                if value == 0xE8E4 {
                    found_e8e4 = true;
                    println!("❌ Found problematic 0xE8E4 at bytes {}:{}", i, i+1);
                }
            }
        }
        
        assert!(!found_e8e4, "Should not find 0xE8E4 pattern in pure red image");
        println!("✅ No 0xE8E4 pattern found - conversion is correct");
    }

    #[test]
    fn test_investigate_e8e4_pattern() {
        println!("🔍 Investigating the 0xE8E4 pattern issue");
        
        // Test what color combination could produce 0xE8E4
        // 0xE8E4 = 1110 1000 1110 0100
        // RGB565: RRRRR GGGGGG BBBBB
        // R = 11101 = 29, G = 000111 = 7, B = 00100 = 4
        
        let e8e4_r5 = (0xE8E4 >> 11) & 0x1F;  // 29
        let e8e4_g6 = (0xE8E4 >> 5) & 0x3F;   // 7  
        let e8e4_b5 = 0xE8E4 & 0x1F;          // 4
        
        println!("0xE8E4 RGB565 components: R5={}, G6={}, B5={}", e8e4_r5, e8e4_g6, e8e4_b5);
        
        // Convert back to RGB888
        let r888 = ((e8e4_r5 * 255 + 15) / 31) as u8;
        let g888 = ((e8e4_g6 * 255 + 31) / 63) as u8;
        let b888 = ((e8e4_b5 * 255 + 15) / 31) as u8;
        
        println!("0xE8E4 as RGB888: ({}, {}, {})", r888, g888, b888);
        
        // Test various color inputs that might produce this
        let test_colors = vec![
            (255, 0, 0),    // Pure red - should give 0xF800
            (232, 7, 33),   // Approximation of what might give E8E4
            (227, 147, 66), // Another approximation
        ];
        
        for (r, g, b) in test_colors {
            let mut img = RgbaImage::new(1, 1);
            img.put_pixel(0, 0, Rgba([r, g, b, 255]));
            let dynamic_img = DynamicImage::ImageRgba8(img);
            
            let rgb565_data = to_rgb565(&dynamic_img);
            let result = rgb565_data[0];
            
            println!("RGB({}, {}, {}) -> RGB565: 0x{:04X}", r, g, b, result);
            
            if result == 0xE8E4 {
                println!("  ⚠️ Found input that produces 0xE8E4!");
            }
        }
        
        // Test what happens with PNG loading
        println!("\n🔍 Testing PNG processing pipeline");
        
        // Create a simple PNG in memory
        use std::io::Cursor;
        let mut red_img = RgbaImage::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                red_img.put_pixel(x, y, Rgba([255, 0, 0, 255])); // Pure red
            }
        }
        
        // Encode as PNG
        let mut png_buffer = Vec::new();
        {
            let mut cursor = Cursor::new(&mut png_buffer);
            let dyn_img = DynamicImage::ImageRgba8(red_img);
            dyn_img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        }
        
        println!("Created {}byte PNG", png_buffer.len());
        
        // Decode the PNG (simulating server processing)
        let decoded_img = image::load_from_memory(&png_buffer).unwrap();
        let rgba_img = decoded_img.to_rgba8();
        
        println!("Decoded PNG: {}x{}", rgba_img.width(), rgba_img.height());
        let first_pixel = rgba_img.get_pixel(0, 0);
        println!("First pixel from PNG: RGBA({}, {}, {}, {})", 
                 first_pixel[0], first_pixel[1], first_pixel[2], first_pixel[3]);
        
        // Convert decoded PNG to RGB565
        let rgb565_from_png = to_rgb565(&decoded_img);
        println!("RGB565 from decoded PNG: 0x{:04X}", rgb565_from_png[0]);
        
        assert_eq!(rgb565_from_png[0], 0xF800, "PNG decode should preserve pure red");
    }

    #[test]
    fn test_rgb565_to_bytes() {
        let rgb565_data = vec![0xF800, 0x07E0, 0x001F]; // 赤、緑、青
        let bytes = rgb565_to_bytes(&rgb565_data);
        
        assert_eq!(bytes.len(), 6);
        // リトルエンディアン確認
        assert_eq!(bytes[0], 0x00); // 0xF800の下位バイト
        assert_eq!(bytes[1], 0xF8); // 0xF800の上位バイト
    }

    #[test]
    fn test_color_accuracy() {
        // 白色の変換精度テスト
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([255, 255, 255, 255])); // 白
        let dynamic_img = DynamicImage::ImageRgba8(img);
        
        let rgb565_data = to_rgb565(&dynamic_img);
        let (r, g, b) = rgb565_to_rgb888(rgb565_data[0]);
        
        // 精度確認（RGB565の制限内で最大値）
        assert!(r >= 248); // 31*255/31 = 255, but with precision loss
        assert!(g >= 252); // 63*255/63 = 255, but with precision loss  
        assert!(b >= 248); // 31*255/31 = 255, but with precision loss
    }

    #[test]
    fn test_green_color() {
        // 純緑色のテスト
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([0, 255, 0, 255])); // 緑
        let dynamic_img = DynamicImage::ImageRgba8(img);
        
        let rgb565_data = to_rgb565(&dynamic_img);
        // 緑色のRGB565値確認（R=0, G=63, B=0 -> 0x07E0）
        assert_eq!(rgb565_data[0], 0x07E0);
    }

    #[test]
    fn test_blue_color() {
        // 純青色のテスト
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([0, 0, 255, 255])); // 青
        let dynamic_img = DynamicImage::ImageRgba8(img);
        
        let rgb565_data = to_rgb565(&dynamic_img);
        // 青色のRGB565値確認（R=0, G=0, B=31 -> 0x001F）
        assert_eq!(rgb565_data[0], 0x001F);
    }

    #[test]
    fn test_alpha_channel_ignored() {
        // アルファチャンネルが無視されることを確認
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 128])); // 半透明赤
        let dynamic_img = DynamicImage::ImageRgba8(img);
        
        let rgb565_data = to_rgb565(&dynamic_img);
        // 透明度に関係なく赤色として処理される
        assert_eq!(rgb565_data[0], 0xF800);
    }
}