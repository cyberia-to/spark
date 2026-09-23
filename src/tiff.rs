//! TIFF dimensions from the IFD, no pixel decode.
//!
//! file#7 added `Kind::ImageTiff`; TIFF has no fixed-offset size field like
//! PNG/BMP/ICO do. Width and height live as tagged entries (256, 257) in
//! the first Image File Directory, whose own offset and entry layout are
//! encoded in the byte order the file's own header declares.

/// Width and height read from the first IFD's `ImageWidth`/`ImageLength`
/// tags, or `None` if the bytes are not TIFF, the IFD is truncated, or
/// either tag is missing.
pub fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let little = match bytes.get(0..4)? {
        [b'I', b'I', 0x2a, 0x00] => true,
        [b'M', b'M', 0x00, 0x2a] => false,
        _ => return None,
    };
    let ifd_offset = read_u32(bytes, 4, little)? as usize;
    let count = read_u16(bytes, ifd_offset, little)? as usize;
    let entries_start = ifd_offset + 2;

    let mut width = None;
    let mut height = None;
    for i in 0..count {
        let entry = entries_start + i * 12;
        let tag = read_u16(bytes, entry, little)?;
        if tag != 256 && tag != 257 {
            continue;
        }
        let field_type = read_u16(bytes, entry + 2, little)?;
        let value = read_tag_value(bytes, entry + 8, field_type, little)?;
        if tag == 256 {
            width = Some(value);
        } else {
            height = Some(value);
        }
    }
    Some((width?, height?))
}

/// A SHORT (type 3) or LONG (type 4) tag value stored inline in the
/// entry's four-byte value field. Any other type (RATIONAL, ASCII, ...)
/// is not a size this reader understands.
fn read_tag_value(bytes: &[u8], offset: usize, field_type: u16, little: bool) -> Option<u32> {
    match field_type {
        3 => read_u16(bytes, offset, little).map(u32::from),
        4 => read_u32(bytes, offset, little),
        _ => None,
    }
}

fn read_u16(bytes: &[u8], offset: usize, little: bool) -> Option<u16> {
    let b: [u8; 2] = bytes.get(offset..offset + 2)?.try_into().ok()?;
    Some(if little { u16::from_le_bytes(b) } else { u16::from_be_bytes(b) })
}

fn read_u32(bytes: &[u8], offset: usize, little: bool) -> Option<u32> {
    let b: [u8; 4] = bytes.get(offset..offset + 4)?.try_into().ok()?;
    Some(if little { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn little_endian_tiff(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"II\x2a\x00");
        bytes.extend_from_slice(&8u32.to_le_bytes()); // IFD right after the header
        bytes.extend_from_slice(&2u16.to_le_bytes()); // two entries
        // ImageWidth, SHORT, count 1, value in the low two bytes
        bytes.extend_from_slice(&256u16.to_le_bytes());
        bytes.extend_from_slice(&3u16.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 2]);
        // ImageLength, SHORT, count 1
        bytes.extend_from_slice(&257u16.to_le_bytes());
        bytes.extend_from_slice(&3u16.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 2]);
        bytes.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
        bytes
    }

    fn big_endian_tiff(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"MM\x00\x2a");
        bytes.extend_from_slice(&8u32.to_be_bytes());
        bytes.extend_from_slice(&2u16.to_be_bytes());
        // ImageWidth, LONG, count 1
        bytes.extend_from_slice(&256u16.to_be_bytes());
        bytes.extend_from_slice(&4u16.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&width.to_be_bytes());
        // ImageLength, LONG, count 1
        bytes.extend_from_slice(&257u16.to_be_bytes());
        bytes.extend_from_slice(&4u16.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes
    }

    #[test]
    fn little_endian_short_tags_read_dimensions() {
        assert_eq!(dimensions(&little_endian_tiff(640, 480)), Some((640, 480)));
    }

    #[test]
    fn big_endian_long_tags_read_dimensions() {
        assert_eq!(dimensions(&big_endian_tiff(1920, 1080)), Some((1920, 1080)));
    }

    #[test]
    fn missing_height_tag_is_none() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"II\x2a\x00");
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // one entry only
        bytes.extend_from_slice(&256u16.to_le_bytes());
        bytes.extend_from_slice(&3u16.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&100u16.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 2]);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(dimensions(&bytes), None);
    }

    #[test]
    fn not_tiff_is_none() {
        assert_eq!(dimensions(b"GIF89a"), None);
    }

    #[test]
    fn truncated_header_is_none() {
        assert_eq!(dimensions(b"II\x2a"), None);
    }
}
