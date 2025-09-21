use std::fs;
use image::{Rgba, RgbaImage, DynamicImage};

fn to_rgb565(img: &DynamicImage) -> Vec<u16> {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut result = Vec::with_capacity((width * height) as usize);
    
    println!("Image dimensions: {}x{}", width, height);
    
    for (i, pixel) in rgba.pixels().enumerate() {
        let r = pixel[0];
        let g = pixel[1]; 
        let b = pixel[2];
        let _a = pixel[3];
        
        // 高精度変換（丸め処理込み）
        let r5 = ((r as u16 * 31 + 127) / 255) as u16; 
        let g6 = ((g as u16 * 63 + 127) / 255) as u16; 
        let b5 = ((b as u16 * 31 + 127) / 255) as u16; 
        
        let rgb565 = (r5 << 11) | (g6 << 5) | b5;
        
        if i < 10 { // Print first 10 pixels for debugging
            println!("Pixel {}: RGB({}, {}, {}) → R5={}, G6={}, B5={} → RGB565=0x{:04X}", 
                     i, r, g, b, r5, g6, b5, rgb565);
        }
        
        result.push(rgb565);
    }
    
    result
}

fn rgb565_to_bytes(rgb565_data: &[u16]) -> Vec<u8> {
    let mut result = Vec::with_capacity(rgb565_data.len() * 2);
    
    for &pixel in rgb565_data {
        // リトルエンディアン形式
        result.push((pixel & 0xFF) as u8);      // 下位バイト
        result.push((pixel >> 8) as u8);        // 上位バイト
    }
    
    result
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create a pure red PNG (128x128)
    let mut img = RgbaImage::new(128, 128);
    
    // Fill with pure red
    for y in 0..128 {
        for x in 0..128 {
            img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
        }
    }
    
    let dynamic_img = DynamicImage::ImageRgba8(img);
    
    // Save the test PNG
    dynamic_img.save("test_red.png")?;
    println!("Created test_red.png (128x128 pure red #FF0000)");
    
    // 2. Convert to RGB565
    let rgb565_data = to_rgb565(&dynamic_img);
    
    println!("\nTotal pixels: {}", rgb565_data.len());
    
    // Check if all pixels are the expected red value
    let expected_red = 0xF800;
    let mut different_count = 0;
    for (i, &pixel) in rgb565_data.iter().enumerate() {
        if pixel != expected_red {
            if different_count < 10 {
                println!("Unexpected pixel {} value: 0x{:04X} (expected 0x{:04X})", i, pixel, expected_red);
            }
            different_count += 1;
        }
    }
    
    if different_count == 0 {
        println!("✓ All pixels correctly converted to 0xF800 (pure red)");
    } else {
        println!("✗ {} pixels have unexpected values", different_count);
    }
    
    // 3. Convert to bytes and create an 8x8 tile (similar to BLE transmission)
    let tile_size = 8;
    let mut tile_data = Vec::new();
    
    println!("\nExtracting first 8x8 tile:");
    for y in 0..tile_size {
        for x in 0..tile_size {
            let idx = y * 128 + x;
            tile_data.push(rgb565_data[idx]);
        }
    }
    
    println!("Tile pixels: {}", tile_data.len());
    for (i, &pixel) in tile_data.iter().enumerate() {
        if i < 10 {
            println!("Tile pixel {}: 0x{:04X}", i, pixel);
        }
    }
    
    // Convert tile to bytes (as sent via BLE)
    let tile_bytes = rgb565_to_bytes(&tile_data);
    println!("\nTile bytes (first 20): {:02X?}", &tile_bytes[0..20.min(tile_bytes.len())]);
    
    // Check what pattern we get
    if tile_bytes.len() >= 4 {
        let pattern1 = u16::from_le_bytes([tile_bytes[0], tile_bytes[1]]);
        let pattern2 = u16::from_le_bytes([tile_bytes[2], tile_bytes[3]]);
        
        println!("First pixel as little-endian u16: 0x{:04X}", pattern1);
        println!("Second pixel as little-endian u16: 0x{:04X}", pattern2);
        
        if pattern1 == 0xE4E8 || pattern2 == 0xE4E8 {
            println!("🔍 Found the 0xE4E8 pattern! This suggests endianness issues.");
        }
        
        // Analyze the pattern
        println!("\nPattern analysis:");
        println!("Expected: 0xF800 (binary: 11111000 00000000)");
        println!("Received bytes: [0x{:02X}, 0x{:02X}]", tile_bytes[0], tile_bytes[1]);
        println!("As big-endian u16: 0x{:04X}", u16::from_be_bytes([tile_bytes[0], tile_bytes[1]]));
        println!("As little-endian u16: 0x{:04X}", u16::from_le_bytes([tile_bytes[0], tile_bytes[1]]));
    }
    
    Ok(())
}