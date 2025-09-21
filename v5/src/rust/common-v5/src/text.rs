//! テキスト処理ユーティリティ
//! 
//! 絵文字とテキストの分離、折り返し処理など

use crate::{Command, Size, RGB};
use std::collections::HashSet;

/// テキストセグメント（テキストまたは絵文字）
#[derive(Debug, Clone)]
pub enum TextSegment {
    Text(String),
    Emoji(u32),
}

/// サポートされている絵文字のセット
pub fn supported_emojis() -> HashSet<u32> {
    let mut emojis = HashSet::new();
    
    // 基本的な絵文字範囲
    // Emoticons (顔文字)
    for code in 0x1F600..=0x1F64F {
        emojis.insert(code);
    }
    
    // Miscellaneous Symbols and Pictographs (その他記号と絵文字)
    for code in 0x1F300..=0x1F5FF {
        emojis.insert(code);
    }
    
    // Transport and Map Symbols (交通と地図記号)
    for code in 0x1F680..=0x1F6FF {
        emojis.insert(code);
    }
    
    // Supplemental Symbols and Pictographs (追加記号と絵文字)
    for code in 0x1F900..=0x1F9FF {
        emojis.insert(code);
    }
    
    // Miscellaneous Symbols (その他記号)
    for code in 0x2600..=0x26FF {
        emojis.insert(code);
    }
    
    // Dingbats
    for code in 0x2700..=0x27BF {
        emojis.insert(code);
    }
    
    // CJK Symbols and Punctuation (一部の記号)
    emojis.insert(0x3030);
    emojis.insert(0x303D);
    
    // Letterlike Symbols (一部の記号)
    emojis.insert(0x2122);
    emojis.insert(0x2139);
    
    // Arrows (矢印)
    for code in 0x2190..=0x21FF {
        emojis.insert(code);
    }
    
    // Enclosed Alphanumerics (囲み文字)
    for code in 0x2460..=0x24FF {
        emojis.insert(code);
    }
    
    // 特定の絵文字（❤️など）
    // Note: 0x2764、0x2665、0x2660-0x2667 are already included in 0x2600..=0x26FF and 0x2700..=0x27BF
    
    emojis
}

/// 文字が絵文字かどうか判定
pub fn is_emoji(c: char) -> bool {
    let code = c as u32;
    
    // 基本的な絵文字範囲のチェック（高速化のため）
    matches!(code,
        // Emoticons (顔文字)
        0x1F600..=0x1F64F |
        // Miscellaneous Symbols and Pictographs (その他記号と絵文字)
        0x1F300..=0x1F5FF |
        // Transport and Map Symbols (交通と地図記号)
        0x1F680..=0x1F6FF |
        // Supplemental Symbols and Pictographs (追加記号と絵文字)
        0x1F900..=0x1F9FF |
        // Miscellaneous Symbols (その他記号)
        0x2600..=0x26FF |
        // Dingbats
        0x2700..=0x27BF |
        // CJK Symbols and Punctuation (一部の記号)
        0x3030 | 0x303D |
        // Letterlike Symbols (一部の記号)
        0x2122 | 0x2139 |
        // Arrows (矢印)
        0x2190..=0x21FF |
        // Enclosed Alphanumerics (囲み文字)
        0x2460..=0x24FF
    )
}

/// テキストを絵文字とテキストのセグメントに分割
pub fn parse_text_with_emoji(text: &str) -> Vec<TextSegment> {
    use tracing::info;
    
    let mut segments = Vec::new();
    let mut current_text = String::new();
    let supported = supported_emojis();
    
    info!("parse_text_with_emoji: Input text: {:?}", text);
    
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    
    while i < chars.len() {
        let ch = chars[i];
        let code = ch as u32;
        
        // デバッグ: すべての文字とそのコードを表示（😊認識確認のため）
        info!("  Char '{}' code: U+{:04X}, supported: {}", ch, code, supported.contains(&code));
        
        // サポートされている絵文字かチェック
        if supported.contains(&code) {
            // 現在のテキストをセグメントに追加
            if !current_text.is_empty() {
                segments.push(TextSegment::Text(current_text.clone()));
                current_text.clear();
            }
            
            // 絵文字セグメントを追加
            info!("  Adding emoji segment: U+{:04X}", code);
            segments.push(TextSegment::Emoji(code));
        } else {
            // 通常の文字を現在のテキストに追加
            current_text.push(ch);
        }
        
        i += 1;
    }
    
    // 残りのテキストをセグメントに追加
    if !current_text.is_empty() {
        segments.push(TextSegment::Text(current_text));
    }
    
    segments
}

/// 各行を絵文字とテキストのコマンドに変換（v3用に調整）
pub fn process_line_with_emoji(
    line: &str,
    x: i32,
    y: i32,
    size: Size,
    color: RGB,
    font_size: u8,
) -> Vec<Command> {
    let mut commands = Vec::new();
    let segments = parse_text_with_emoji(line);
    let mut current_x = x;
    
    // 文字幅の計算（ピクセル単位で調整）
    let ascii_width = match font_size {
        1 => 8,   // fs=1: ASCII 8ピクセル
        2 => 12,  // fs=2: ASCII 12ピクセル  
        3 => 16,  // fs=3: ASCII 16ピクセル
        4 => 20,  // fs=4: ASCII 20ピクセル
        _ => 12,  // デフォルト
    };
    
    for segment in segments {
        match segment {
            TextSegment::Text(txt) => {
                if !txt.is_empty() {
                    commands.push(Command::Text {
                        x: (current_x as i32).clamp(0, 255) as u8,
                        y: (y as i32).clamp(0, 255) as u8,
                        size,
                        color,
                        text: txt.clone(),
                    });
                    
                    // テキストの幅を計算して位置を更新
                    for ch in txt.chars() {
                        let char_width = if ch.is_ascii() {
                            ascii_width
                        } else {
                            ascii_width * 2  // 全角文字は2倍
                        };
                        current_x = current_x.saturating_add(char_width);
                    }
                }
            }
            TextSegment::Emoji(code) => {
                // v3のEmoji構造に合わせて変換（u32コードポイント直接使用）
                commands.push(Command::Emoji {
                    x: (current_x as i32).clamp(0, 255) as u8,
                    y: (y as i32).clamp(0, 255) as u8,
                    size: size.to_byte(),
                    code,
                });
                
                // 絵文字の幅（全角文字と同じ）
                let emoji_width = ascii_width * 2;
                current_x = current_x.saturating_add(emoji_width);
            }
        }
    }
    
    commands
}

/// テキストを領域内で折り返す（絵文字対応版）
pub fn wrap_text_with_emoji(text: &str, area_width: i32, font_size: u8) -> Vec<String> {
    use tracing::info;
    
    // 文字幅の計算（グリッド単位）
    let ascii_width = match font_size {
        1 => 2,   // fs=1: ASCII 2グリッド（8ピクセル相当）
        2 => 3,   // fs=2: ASCII 3グリッド（12ピクセル相当）
        3 => 4,   // fs=3: ASCII 4グリッド（16ピクセル相当）
        4 => 5,   // fs=4: ASCII 5グリッド（20ピクセル相当）
        _ => 3,   // デフォルト
    };
    
    info!("wrap_text_with_emoji: area_width={}, font_size={}, ascii_width={}", area_width, font_size, ascii_width);
    
    let mut lines = Vec::new();
    
    // 既存の改行で分割
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        
        // 各段落を幅で折り返す
        let mut current_line = String::new();
        let mut current_width = 0;
        
        for c in paragraph.chars() {
            // 文字幅計算（絵文字対応）
            let c_width = if is_emoji(c) {
                // 絵文字は全角文字と同じ幅（ASCII文字の2倍）
                ascii_width * 2
            } else if c.is_ascii() {
                ascii_width      // ASCII文字の幅
            } else {
                ascii_width * 2  // 全角文字は2倍
            };
            
            // 行幅を超える場合は改行
            if current_width + c_width > area_width {
                if !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = String::new();
                    current_width = 0;
                }
            }
            
            current_line.push(c);
            current_width += c_width;
        }
        
        // 残りの文字を追加
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    
    lines
}

/// 絵文字文字列をコードポイントに変換（v3のString→u32変換用）
pub fn emoji_string_to_codepoint(emoji: &str) -> Option<u32> {
    let chars: Vec<char> = emoji.chars().collect();
    if chars.len() == 1 {
        Some(chars[0] as u32)
    } else if chars.is_empty() {
        None
    } else {
        // 複合絵文字の場合は最初の文字のコードポイントを返す
        Some(chars[0] as u32)
    }
}

/// コードポイントを絵文字文字列に変換（u32→String変換用）
pub fn codepoint_to_emoji_string(code: u32) -> String {
    char::from_u32(code)
        .map(|c| c.to_string())
        .unwrap_or_else(|| "?".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_emoji() {
        assert!(is_emoji('😀'));
        assert!(is_emoji('❤'));
        assert!(is_emoji('♠'));
        assert!(!is_emoji('A'));
        assert!(!is_emoji('あ'));
    }
    
    #[test]
    fn test_parse_text_with_emoji() {
        let text = "Hello 😀 World ❤";  // ❤ without variation selector
        let segments = parse_text_with_emoji(text);
        assert_eq!(segments.len(), 4);
        
        match &segments[0] {
            TextSegment::Text(t) => assert_eq!(t, "Hello "),
            _ => panic!("Expected text segment"),
        }
        
        match &segments[1] {
            TextSegment::Emoji(code) => assert_eq!(*code, 0x1F600),
            _ => panic!("Expected emoji segment"),
        }
        
        match &segments[2] {
            TextSegment::Text(t) => assert_eq!(t, " World "),
            _ => panic!("Expected text segment"),
        }
        
        match &segments[3] {
            TextSegment::Emoji(code) => assert_eq!(*code, 0x2764),
            _ => panic!("Expected emoji segment"),
        }
    }
    
    #[test]
    fn test_emoji_conversion() {
        let emoji = "😀";
        let code = emoji_string_to_codepoint(emoji);
        assert_eq!(code, Some(0x1F600));
        
        let converted = codepoint_to_emoji_string(0x1F600);
        assert_eq!(converted, "😀");
    }
}