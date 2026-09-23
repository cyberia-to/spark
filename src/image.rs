//! Image dimension extraction from raw encoded bytes — PNG, GIF, JPEG.
//!
//! Reads only the fixed-position header fields each format's spec
//! guarantees; never decodes pixels. Self-contained on raw bytes, the
//! same shape as [`crate::pdf::page_count`]: no dependency on `file::Kind`.
//!
//! WebP's dimensions live inside a RIFF chunk (`VP8 `, `VP8L` or `VP8X`,
//! three different layouts) and are not parsed here yet — a documented
//! gap, not a silent wrong answer.

/// Width and height in pixels, or `None` if the bytes are not a
/// recognized PNG, GIF or JPEG, or the format's dimension field could
/// not be located.
pub fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    png_dimensions(bytes)
        .or_else(|| gif_dimensions(bytes))
        .or_else(|| jpeg_dimensions(bytes))
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes.len() < 24 || bytes[0..8] != SIG {
        return None;
    }
    if &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((width, height))
}

fn gif_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 10 || (&bytes[0..6] != b"GIF87a" && &bytes[0..6] != b"GIF89a") {
        return None;
    }
    let width = u16::from_le_bytes(bytes[6..8].try_into().ok()?) as u32;
    let height = u16::from_le_bytes(bytes[8..10].try_into().ok()?) as u32;
    Some((width, height))
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut i = 2usize;
    while i + 1 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        // markers with no length/payload: TEM, RSTn, SOI, EOI
        if marker == 0x01 || (0xD0..=0xD9).contains(&marker) {
            i += 2;
            continue;
        }
        if i + 4 > bytes.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        let is_sof = (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC;
        if is_sof {
            if i + 9 > bytes.len() {
                return None;
            }
            let height = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
            let width = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
            return Some((width, height));
        }
        if seg_len < 2 {
            return None;
        }
        i += 2 + seg_len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        v.extend_from_slice(&[0, 0, 0, 13]);
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&width.to_be_bytes());
        v.extend_from_slice(&height.to_be_bytes());
        v.extend_from_slice(&[0; 5]);
        v
    }

    fn gif_bytes(width: u16, height: u16) -> Vec<u8> {
        let mut v = b"GIF89a".to_vec();
        v.extend_from_slice(&width.to_le_bytes());
        v.extend_from_slice(&height.to_le_bytes());
        v.extend_from_slice(&[0; 3]);
        v
    }

    #[test]
    fn png_reads_width_and_height_from_ihdr() {
        let bytes = png_bytes(800, 600);
        assert_eq!(dimensions(&bytes), Some((800, 600)));
    }

    #[test]
    fn png_too_short_is_none() {
        assert_eq!(dimensions(&[0x89, b'P', b'N', b'G']), None);
    }

    #[test]
    fn gif_reads_logical_screen_size() {
        let bytes = gif_bytes(320, 240);
        assert_eq!(dimensions(&bytes), Some((320, 240)));
    }

    #[test]
    fn jpeg_reads_sof0_dimensions_past_an_app0_segment() {
        let mut v = vec![0xFF, 0xD8];
        v.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00]);
        v.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
        v.extend_from_slice(&100u16.to_be_bytes());
        v.extend_from_slice(&200u16.to_be_bytes());
        v.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);
        assert_eq!(dimensions(&v), Some((200, 100)));
    }

    #[test]
    fn jpeg_skips_a_comment_segment_by_its_length_to_find_sof() {
        let mut v = vec![0xFF, 0xD8];
        v.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x08]);
        v.extend_from_slice(b"hello!");
        v.extend_from_slice(&[0xFF, 0xC2, 0x00, 0x11, 0x08]);
        v.extend_from_slice(&50u16.to_be_bytes());
        v.extend_from_slice(&64u16.to_be_bytes());
        v.extend_from_slice(&[0x03, 0x01, 0x11, 0x00]);
        assert_eq!(dimensions(&v), Some((64, 50)));
    }

    #[test]
    fn jpeg_with_no_sof_marker_is_none() {
        let v = vec![0xFF, 0xD8, 0xFF, 0xD9];
        assert_eq!(dimensions(&v), None);
    }

    #[test]
    fn non_image_bytes_have_no_dimensions() {
        assert_eq!(dimensions(b"just some text, not an image"), None);
    }

    #[test]
    fn webp_is_not_parsed_yet() {
        let mut v = b"RIFF".to_vec();
        v.extend_from_slice(&[0; 4]);
        v.extend_from_slice(b"WEBPVP8 ");
        assert_eq!(dimensions(&v), None);
    }
}
