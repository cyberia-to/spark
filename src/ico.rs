//! ICO (Windows icon) dimensions from the directory header, no image decode.
//!
//! Unlike PNG/GIF/JPEG/WebP/BMP, an ICO file is a small directory of one
//! or more images at different sizes. This reads the first directory
//! entry's width and height — the size a chrome favicon or file-preview
//! surface would draw by default.

/// Width and height of an ICO file's first directory entry, or `None` if
/// the bytes are not an ICO file or the directory is truncated.
///
/// Each dimension byte is `0` to mean `256` (the format's own encoding
/// for the maximum icon size, since a byte cannot hold 256).
pub fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    // ICONDIR: reserved (u16, must be 0), type (u16, must be 1 for icons),
    // count (u16, must be at least 1 to have a first entry).
    if bytes.len() < 22 {
        return None;
    }
    let reserved = u16::from_le_bytes([bytes[0], bytes[1]]);
    let kind = u16::from_le_bytes([bytes[2], bytes[3]]);
    let count = u16::from_le_bytes([bytes[4], bytes[5]]);
    if reserved != 0 || kind != 1 || count == 0 {
        return None;
    }
    // First ICONDIRENTRY starts right after the 6-byte ICONDIR: width and
    // height are single bytes, 0 meaning 256.
    let width = if bytes[6] == 0 { 256 } else { bytes[6] as u32 };
    let height = if bytes[7] == 0 { 256 } else { bytes[7] as u32 };
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icondir(count: u16, width: u8, height: u8) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0u16.to_le_bytes()); // reserved
        bytes.extend_from_slice(&1u16.to_le_bytes()); // type = icon
        bytes.extend_from_slice(&count.to_le_bytes());
        bytes.push(width);
        bytes.push(height);
        bytes.extend_from_slice(&[0u8; 14]); // rest of ICONDIRENTRY, unchecked
        bytes
    }

    #[test]
    fn reads_first_entry_dimensions() {
        let bytes = icondir(1, 32, 32);
        assert_eq!(dimensions(&bytes), Some((32, 32)));
    }

    #[test]
    fn zero_byte_means_256() {
        let bytes = icondir(1, 0, 0);
        assert_eq!(dimensions(&bytes), Some((256, 256)));
    }

    #[test]
    fn reads_first_of_several_entries() {
        let mut bytes = icondir(3, 16, 16);
        bytes.extend_from_slice(&[0xff; 16]); // a second entry, ignored
        assert_eq!(dimensions(&bytes), Some((16, 16)));
    }

    #[test]
    fn zero_count_is_none() {
        let bytes = icondir(0, 32, 32);
        assert_eq!(dimensions(&bytes), None);
    }

    #[test]
    fn wrong_type_is_none() {
        let mut bytes = icondir(1, 32, 32);
        bytes[2] = 2; // type = cursor, not icon
        assert_eq!(dimensions(&bytes), None);
    }

    #[test]
    fn nonzero_reserved_is_none() {
        let mut bytes = icondir(1, 32, 32);
        bytes[0] = 1;
        assert_eq!(dimensions(&bytes), None);
    }

    #[test]
    fn too_short_is_none() {
        assert_eq!(dimensions(&[0, 0, 1, 0]), None);
    }
}
