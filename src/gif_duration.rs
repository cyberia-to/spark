//! Animated GIF duration from the Graphic Control Extension delay fields.
//!
//! A GIF's dimensions come from the Logical Screen Descriptor; its playback
//! length does not live there. Each frame's delay sits in its own Graphic
//! Control Extension, so total duration means walking every block.

/// Sum of every frame's delay time, in milliseconds. `None` if `data` is
/// not a GIF, or the block structure runs out before a trailer.
pub fn duration_ms(data: &[u8]) -> Option<u32> {
    if !(data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a")) {
        return None;
    }
    // Logical Screen Descriptor: width(2) height(2) packed(1) bg(1) aspect(1)
    let packed = *data.get(10)?;
    let mut pos = 13;
    if packed & 0x80 != 0 {
        pos += global_color_table_len(packed);
    }

    let mut total_ms: u32 = 0;
    loop {
        match *data.get(pos)? {
            0x3B => return Some(total_ms), // trailer
            0x21 => {
                let label = *data.get(pos + 1)?;
                if label == 0xF9 && *data.get(pos + 2)? >= 4 {
                    let lo = *data.get(pos + 4)?;
                    let hi = *data.get(pos + 5)?;
                    total_ms += u16::from_le_bytes([lo, hi]) as u32 * 10;
                }
                pos = skip_sub_blocks(data, pos + 2)?;
            }
            0x2C => {
                let img_packed = *data.get(pos + 9)?;
                pos += 10;
                if img_packed & 0x80 != 0 {
                    pos += global_color_table_len(img_packed);
                }
                pos += 1; // LZW minimum code size
                pos = skip_sub_blocks(data, pos)?;
            }
            _ => return Some(total_ms), // unrecognized block: stop, keep what we found
        }
    }
}

fn global_color_table_len(packed: u8) -> usize {
    3 << ((packed & 0x07) as usize + 1)
}

/// Walk a sub-block chain (size byte, that many data bytes, repeat) to its
/// zero-size terminator. Returns the position just past the terminator.
fn skip_sub_blocks(data: &[u8], mut pos: usize) -> Option<usize> {
    loop {
        let size = *data.get(pos)? as usize;
        pos += 1;
        if size == 0 {
            return Some(pos);
        }
        pos += size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_no_global_table() -> Vec<u8> {
        // "GIF89a" + LSD: 1x1, packed=0 (no global color table), bg=0, aspect=0
        let mut v = b"GIF89a".to_vec();
        v.extend_from_slice(&[0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00]);
        v
    }

    fn graphic_control_extension(delay_1_100s: u16) -> Vec<u8> {
        let [lo, hi] = delay_1_100s.to_le_bytes();
        vec![0x21, 0xF9, 0x04, 0x00, lo, hi, 0x00, 0x00]
    }

    fn image_descriptor_1x1() -> Vec<u8> {
        let mut v = vec![0x2C, 0, 0, 0, 0, 0x01, 0x00, 0x01, 0x00, 0x00]; // no local table
        v.push(0x02); // LZW minimum code size
        v.extend_from_slice(&[0x01, 0x00, 0x00]); // one 1-byte data sub-block, then terminator
        v
    }

    #[test]
    fn sums_two_frame_delays() {
        let mut data = header_no_global_table();
        data.extend(graphic_control_extension(10)); // 100ms
        data.extend(image_descriptor_1x1());
        data.extend(graphic_control_extension(50)); // 500ms
        data.extend(image_descriptor_1x1());
        data.push(0x3B); // trailer
        assert_eq!(duration_ms(&data), Some(600));
    }

    #[test]
    fn static_gif_with_no_extension_is_zero() {
        let mut data = header_no_global_table();
        data.extend(image_descriptor_1x1());
        data.push(0x3B);
        assert_eq!(duration_ms(&data), Some(0));
    }

    #[test]
    fn non_gif_is_none() {
        assert_eq!(duration_ms(b"not a gif"), None);
        assert_eq!(duration_ms(&[]), None);
    }

    #[test]
    fn truncated_before_trailer_is_none() {
        let mut data = header_no_global_table();
        data.extend(graphic_control_extension(10));
        assert_eq!(duration_ms(&data), None);
    }
}
