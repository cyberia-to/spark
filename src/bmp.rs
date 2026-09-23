//! BMP dimensions from raw bitmap header bytes, no pixel decode.
//!
//! spark#5/#6 covered PNG/GIF/JPEG and WebP; BMP is the other image format
//! a legacy web-era file corpus (like the bootloader burial) can carry.
//! The size lives in the DIB header that follows the 14-byte file header,
//! not in the file header itself, so this reads past it.

/// Width and height from a BMP file's file header and DIB header, or
/// `None` if the bytes are not a BMP file or the DIB header is truncated.
///
/// The DIB header's height field is a signed `i32`: positive means the
/// image is stored bottom-up (the common case), negative means top-down.
/// Either way the drawn image is `|height|` pixels tall, so this returns
/// the absolute value.
pub fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 14 || &bytes[0..2] != b"BM" {
        return None;
    }
    let dib = bytes.get(14..)?;
    let dib_size = u32::from_le_bytes(dib.get(0..4)?.try_into().ok()?);
    if dib_size < 12 {
        return None;
    }
    if dib_size == 12 {
        // BITMAPCOREHEADER: 16-bit width and height, always unsigned/top-down-never.
        let w = u16::from_le_bytes(dib.get(4..6)?.try_into().ok()?);
        let h = u16::from_le_bytes(dib.get(6..8)?.try_into().ok()?);
        return Some((w as u32, h as u32));
    }
    // BITMAPINFOHEADER and every later variant (V4/V5) keep width/height
    // as signed 32-bit values at the same offsets 4 and 8 into the DIB header.
    let w = i32::from_le_bytes(dib.get(4..8)?.try_into().ok()?);
    let h = i32::from_le_bytes(dib.get(8..12)?.try_into().ok()?);
    Some((w.unsigned_abs(), h.unsigned_abs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bmp_info_header(width: i32, height: i32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"BM");
        bytes.extend_from_slice(&0u32.to_le_bytes()); // file size, unchecked
        bytes.extend_from_slice(&[0u8; 4]); // reserved
        bytes.extend_from_slice(&0u32.to_le_bytes()); // pixel data offset, unchecked
        bytes.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER size
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes
    }

    fn bmp_core_header(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"BM");
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 4]);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&12u32.to_le_bytes()); // BITMAPCOREHEADER size
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes
    }

    #[test]
    fn info_header_reads_bottom_up_dimensions() {
        let bytes = bmp_info_header(640, 480);
        assert_eq!(dimensions(&bytes), Some((640, 480)));
    }

    #[test]
    fn info_header_negative_height_is_top_down_same_magnitude() {
        let bytes = bmp_info_header(640, -480);
        assert_eq!(dimensions(&bytes), Some((640, 480)));
    }

    #[test]
    fn core_header_reads_dimensions() {
        let bytes = bmp_core_header(16, 16);
        assert_eq!(dimensions(&bytes), Some((16, 16)));
    }

    #[test]
    fn wrong_signature_is_none() {
        let mut bytes = bmp_info_header(640, 480);
        bytes[0] = b'X';
        assert_eq!(dimensions(&bytes), None);
    }

    #[test]
    fn too_short_for_file_header_is_none() {
        assert_eq!(dimensions(b"BM\x00\x00"), None);
    }

    #[test]
    fn truncated_dib_header_is_none() {
        let mut bytes = bmp_info_header(640, 480);
        bytes.truncate(20); // file header (14) + dib size field (4) + 2 bytes, no width/height
        assert_eq!(dimensions(&bytes), None);
    }

    #[test]
    fn dib_size_below_core_header_is_none() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"BM");
        bytes.extend_from_slice(&[0u8; 12]); // file size, reserved, pixel data offset
        bytes.extend_from_slice(&4u32.to_le_bytes()); // smaller than any real DIB header
        assert_eq!(dimensions(&bytes), None);
    }

    #[test]
    fn v4_header_size_still_reads_offsets_4_and_8() {
        // BITMAPV4HEADER (108 bytes) keeps width/height at the same offsets
        // as BITMAPINFOHEADER; only the DIB size field itself differs.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"BM");
        bytes.extend_from_slice(&0u32.to_le_bytes()); // file size, unchecked
        bytes.extend_from_slice(&[0u8; 4]); // reserved
        bytes.extend_from_slice(&0u32.to_le_bytes()); // pixel data offset, unchecked
        bytes.extend_from_slice(&108u32.to_le_bytes());
        bytes.extend_from_slice(&1920i32.to_le_bytes());
        bytes.extend_from_slice(&1080i32.to_le_bytes());
        assert_eq!(dimensions(&bytes), Some((1920, 1080)));
    }
}
