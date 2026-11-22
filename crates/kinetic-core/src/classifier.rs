use serde::{Deserialize, Serialize};
use crate::pipeline::{estimate_shannon_entropy, BlockCodec};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentCategory {
    IncompressibleMedia,
    ExecutableBinary,
    StructuredNumeric,
    DeflateContainer,
    HighRedundancyText,
    GeneralBinary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentProfile {
    pub category: ContentCategory,
    pub shannon_entropy: f64,
    pub recommended_codec: BlockCodec,
    pub recommended_filter_mask: u8,
}

pub fn classify_content(path_hint: &str, data: &[u8]) -> ContentProfile {
    let entropy = estimate_shannon_entropy(data);
    let lower = path_hint.to_ascii_lowercase();

    let is_media_magic = (data.len() >= 4 && &data[0..4] == b"\x89PNG")
        || (data.len() >= 3 && &data[0..3] == b"\xFF\xD8\xFF")
        || (data.len() >= 4 && &data[0..4] == b"GIF8")
        || (data.len() >= 12 && &data[4..8] == b"ftyp")
        || lower.ends_with(".mp4")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".webm")
        || lower.ends_with(".mkv");

    if is_media_magic || entropy > 7.85 {
        return ContentProfile {
            category: ContentCategory::IncompressibleMedia,
            shannon_entropy: entropy,
            recommended_codec: BlockCodec::Store,
            recommended_filter_mask: 0x00,
        };
    }

    if (data.len() >= 2 && &data[0..2] == b"MZ")
        || (data.len() >= 4 && &data[0..4] == b"\x7FELF")
        || lower.ends_with(".exe")
        || lower.ends_with(".dll")
        || lower.ends_with(".sys")
    {
        return ContentProfile {
            category: ContentCategory::ExecutableBinary,
            shannon_entropy: entropy,
            recommended_codec: BlockCodec::DeflateHigh,
            recommended_filter_mask: 0x01,
        };
    }

    if (data.len() >= 4 && &data[0..4] == b"PK\x03\x04")
        && (lower.ends_with(".docx")
            || lower.ends_with(".xlsx")
            || lower.ends_with(".pptx")
            || lower.ends_with(".jar")
            || lower.ends_with(".apk"))
    {
        return ContentProfile {
            category: ContentCategory::DeflateContainer,
            shannon_entropy: entropy,
            recommended_codec: BlockCodec::BrotliUltra,
            recommended_filter_mask: 0x00,
        };
    }

    if lower.ends_with(".wav") || lower.ends_with(".bmp") || lower.ends_with(".raw") {
        return ContentProfile {
            category: ContentCategory::StructuredNumeric,
            shannon_entropy: entropy,
            recommended_codec: BlockCodec::DeflateHigh,
            recommended_filter_mask: 0x04,
        };
    }

    if (lower.ends_with(".bin") || lower.ends_with(".dat")) && data.len() >= 64 {
        return ContentProfile {
            category: ContentCategory::StructuredNumeric,
            shannon_entropy: entropy,
            recommended_codec: BlockCodec::DeflateHigh,
            recommended_filter_mask: 0x08,
        };
    }

    let is_text_ext = lower.ends_with(".txt")
        || lower.ends_with(".md")
        || lower.ends_with(".rs")
        || lower.ends_with(".ts")
        || lower.ends_with(".tsx")
        || lower.ends_with(".js")
        || lower.ends_with(".json")
        || lower.ends_with(".toml")
        || lower.ends_with(".html")
        || lower.ends_with(".css")
        || lower.ends_with(".xml")
        || lower.ends_with(".csv");

    if is_text_ext || entropy <= 5.4 {
        return ContentProfile {
            category: ContentCategory::HighRedundancyText,
            shannon_entropy: entropy,
            recommended_codec: BlockCodec::BrotliUltra,
            recommended_filter_mask: 0x00,
        };
    }

    ContentProfile {
        category: ContentCategory::GeneralBinary,
        shannon_entropy: entropy,
        recommended_codec: BlockCodec::Lz4,
        recommended_filter_mask: 0x00,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classifier_routes_media_exec_and_text() {
        let png_header = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        let media_prof = classify_content("photo.png", png_header);
        assert_eq!(media_prof.category, ContentCategory::IncompressibleMedia);
        assert_eq!(media_prof.recommended_codec, BlockCodec::Store);

        let exe_header = b"MZ\x90\x00\x03\x00\x00\x00\x04\x00\x00\x00\xFF\xFF";
        let exe_prof = classify_content("app.exe", exe_header);
        assert_eq!(exe_prof.category, ContentCategory::ExecutableBinary);
        assert_eq!(exe_prof.recommended_filter_mask, 0x01);

        let src_code = b"pub fn main() { println!(\"Hello Kinetic Engine!\"); }\n".repeat(32);
        let text_prof = classify_content("main.rs", &src_code);
        assert_eq!(text_prof.category, ContentCategory::HighRedundancyText);
        assert_eq!(text_prof.recommended_codec, BlockCodec::BrotliUltra);
    }
}