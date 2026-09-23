//! WebP dimensions from raw RIFF container bytes, no pixel decode.
//!
//! spark#5 parsed PNG/GIF/JPEG headers for `dimensions` and left WebP
//! unparsed, since its size lives inside one of three RIFF chunk layouts
//! (`VP8 ` lossy, `VP8L` lossless, `VP8X` extended) instead of one fixed
//! header. This module reads whichever layout the file carries.

/// Width and height from a WebP file's RIFF/VP8 headers, or `None` if the
/// bytes are not a WebP file or the chunk layout is not recognized.
pub fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 20 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    let fourcc = &bytes[12..16];
    let data = bytes.get(20..)?;
    match fourcc {
        b"VP8 " => vp8_dimensions(data),
        b"VP8L" => vp8l_dimensions(data),
        b"VP8X" => vp8x_dimensions(data),
        _ => None,
    }
}

/// Lossy `VP8 ` chunk: 3-byte frame tag, 3-byte start code `9d 01 2a`,
/// then 14-bit width and 14-bit height, each in a little-endian u16 whose
/// top 2 bits are an unrelated scale factor.
fn vp8_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 10 || data[3] != 0x9d || data[4] != 0x01 || data[5] != 0x2a {
        return None;
    }
    let w = u16::from_le_bytes([data[6], data[7]]) & 0x3fff;
    let h = u16::from_le_bytes([data[8], data[9]]) & 0x3fff;
    Some((w as u32, h as u32))
}

/// Lossless `VP8L` chunk: 1-byte signature `0x2f`, then a little-endian
/// 32-bit field packing 14-bit (width-1), 14-bit (height-1), a 1-bit alpha
/// flag and a 3-bit version, low bits first.
fn vp8l_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 5 || data[0] != 0x2f {
        return None;
    }
    let bits = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
    let width = (bits & 0x3fff) + 1;
    let height = ((bits >> 14) & 0x3fff) + 1;
    Some((width, height))
}

/// Extended `VP8X` chunk: 1-byte flags, 3 reserved bytes, then 24-bit
/// little-endian (canvas width - 1) and 24-bit little-endian
/// (canvas height - 1). Present for animated, ICC, EXIF or XMP WebP.
fn vp8x_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 10 {
        return None;
    }
    let width = 1 + (data[4] as u32 | (data[5] as u32) << 8 | (data[6] as u32) << 16);
    let height = 1 + (data[7] as u32 | (data[8] as u32) << 8 | (data[9] as u32) << 16);
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn riff_webp(fourcc: &[u8; 4], chunk_data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(4 + 8 + chunk_data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(b"WEBP");
        bytes.extend_from_slice(fourcc);
        bytes.extend_from_slice(&(chunk_data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(chunk_data);
        bytes
    }

    #[test]
    fn vp8_lossy_reads_width_and_height() {
        let chunk = [
            0x10, 0x00, 0x00, // frame tag
            0x9d, 0x01, 0x2a, // start code
            0xc8, 0x00, // width = 200
            0x64, 0x00, // height = 100
        ];
        let bytes = riff_webp(b"VP8 ", &chunk);
        assert_eq!(dimensions(&bytes), Some((200, 100)));
    }

    #[test]
    fn vp8_lossy_rejects_wrong_start_code() {
        let chunk = [0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc8, 0x00, 0x64, 0x00];
        let bytes = riff_webp(b"VP8 ", &chunk);
        assert_eq!(dimensions(&bytes), None);
    }

    #[test]
    fn vp8l_lossless_reads_width_minus_one_height_minus_one() {
        // width = 7 (6), height = 3 (2): bits = 6 | (2 << 14)
        let bits: u32 = 6 | (2 << 14);
        let mut chunk = vec![0x2f];
        chunk.extend_from_slice(&bits.to_le_bytes());
        let bytes = riff_webp(b"VP8L", &chunk);
        assert_eq!(dimensions(&bytes), Some((7, 3)));
    }

    #[test]
    fn vp8l_rejects_wrong_signature() {
        let mut chunk = vec![0x00];
        chunk.extend_from_slice(&0u32.to_le_bytes());
        let bytes = riff_webp(b"VP8L", &chunk);
        assert_eq!(dimensions(&bytes), None);
    }

    #[test]
    fn vp8x_extended_reads_canvas_width_minus_one_height_minus_one() {
        // canvas width = 100 (99), canvas height = 50 (49)
        let chunk = [
            0x00, 0x00, 0x00, 0x00, // flags + reserved
            99, 0x00, 0x00, // width - 1, 24-bit LE
            49, 0x00, 0x00, // height - 1, 24-bit LE
        ];
        let bytes = riff_webp(b"VP8X", &chunk);
        assert_eq!(dimensions(&bytes), Some((100, 50)));
    }

    #[test]
    fn vp8x_handles_dimensions_past_16_bits() {
        // canvas width = 16777216 - 1 max representable, use a smaller
        // but still > u16::MAX value to exercise the 24-bit path: 70000
        let w_minus_1 = 70000u32 - 1;
        let h_minus_1 = 40000u32 - 1;
        let chunk = [
            0x00, 0x00, 0x00, 0x00,
            (w_minus_1 & 0xff) as u8,
            ((w_minus_1 >> 8) & 0xff) as u8,
            ((w_minus_1 >> 16) & 0xff) as u8,
            (h_minus_1 & 0xff) as u8,
            ((h_minus_1 >> 8) & 0xff) as u8,
            ((h_minus_1 >> 16) & 0xff) as u8,
        ];
        let bytes = riff_webp(b"VP8X", &chunk);
        assert_eq!(dimensions(&bytes), Some((70000, 40000)));
    }

    #[test]
    fn unknown_fourcc_is_none() {
        let bytes = riff_webp(b"VP8?", &[0; 10]);
        assert_eq!(dimensions(&bytes), None);
    }

    #[test]
    fn non_riff_bytes_have_no_dimensions() {
        assert_eq!(dimensions(b"not a webp file at all"), None);
    }

    #[test]
    fn riff_without_webp_tag_is_none() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(b"AVI ");
        bytes.extend_from_slice(&[0; 20]);
        assert_eq!(dimensions(&bytes), None);
    }

    #[test]
    fn too_short_is_none() {
        assert_eq!(dimensions(b"RIFF\x00\x00\x00\x00WEBP"), None);
    }
}
