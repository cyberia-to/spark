//! Video duration from raw encoded bytes — MP4/QuickTime (ISO-BMFF) today.
//!
//! Reads only the box structure and the `mvhd` header fields the ISO-BMFF
//! spec guarantees; never decodes frames. Self-contained on raw bytes, the
//! same shape as [`crate::audio::duration_ms`]: no dependency on
//! `file::Kind`.
//!
//! WebM/Matroska duration is not parsed here yet — a documented gap, not
//! a silent wrong answer.

/// Duration in whole milliseconds, or `None` if the bytes are not a
/// recognized MP4/QuickTime container, or the `moov`/`mvhd` boxes could
/// not be located.
pub fn duration_ms(bytes: &[u8]) -> Option<u64> {
    let moov = find_box(bytes, b"moov")?;
    let mvhd = find_box(moov, b"mvhd")?;
    parse_mvhd(mvhd)
}

/// One ISO-BMFF box: 4-byte size, 4-byte type, then payload. `size == 1`
/// means a 64-bit extended size follows the type; `size == 0` means the
/// box runs to the end of the buffer it was found in.
fn find_box<'a>(bytes: &'a [u8], want: &[u8; 4]) -> Option<&'a [u8]> {
    let mut pos = 0usize;
    while pos + 8 <= bytes.len() {
        let size32 = u32::from_be_bytes(bytes[pos..pos + 4].try_into().ok()?);
        let box_type = &bytes[pos + 4..pos + 8];

        let (header_len, box_len) = if size32 == 1 {
            if pos + 16 > bytes.len() {
                return None;
            }
            let size64 = u64::from_be_bytes(bytes[pos + 8..pos + 16].try_into().ok()?);
            (16usize, size64 as usize)
        } else if size32 == 0 {
            (8usize, bytes.len() - pos)
        } else {
            (8usize, size32 as usize)
        };

        if box_len < header_len || pos + box_len > bytes.len() {
            return None;
        }

        if box_type == want {
            return Some(&bytes[pos + header_len..pos + box_len]);
        }

        pos += box_len;
    }
    None
}

fn parse_mvhd(body: &[u8]) -> Option<u64> {
    if body.is_empty() {
        return None;
    }
    let version = body[0];
    let (timescale, duration) = if version == 1 {
        // full box header (4) + creation(8) + modification(8) = 20, then
        // timescale(4) + duration(8)
        if body.len() < 32 {
            return None;
        }
        let timescale = u32::from_be_bytes(body[20..24].try_into().ok()?);
        let duration = u64::from_be_bytes(body[24..32].try_into().ok()?);
        (timescale, duration)
    } else {
        // full box header (4) + creation(4) + modification(4) = 12, then
        // timescale(4) + duration(4)
        if body.len() < 20 {
            return None;
        }
        let timescale = u32::from_be_bytes(body[12..16].try_into().ok()?);
        let duration = u32::from_be_bytes(body[16..20].try_into().ok()?) as u64;
        (timescale, duration)
    };
    if timescale == 0 {
        return None;
    }
    Some((duration * 1000) / timescale as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_box(box_type: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&((8 + payload.len()) as u32).to_be_bytes());
        v.extend_from_slice(box_type);
        v.extend_from_slice(payload);
        v
    }

    fn mvhd_v0(timescale: u32, duration: u32) -> Vec<u8> {
        let mut body = vec![0u8]; // version
        body.extend_from_slice(&[0, 0, 0]); // flags
        body.extend_from_slice(&0u32.to_be_bytes()); // creation_time
        body.extend_from_slice(&0u32.to_be_bytes()); // modification_time
        body.extend_from_slice(&timescale.to_be_bytes());
        body.extend_from_slice(&duration.to_be_bytes());
        make_box(b"mvhd", &body)
    }

    fn mvhd_v1(timescale: u32, duration: u64) -> Vec<u8> {
        let mut body = vec![1u8]; // version
        body.extend_from_slice(&[0, 0, 0]); // flags
        body.extend_from_slice(&0u64.to_be_bytes()); // creation_time
        body.extend_from_slice(&0u64.to_be_bytes()); // modification_time
        body.extend_from_slice(&timescale.to_be_bytes());
        body.extend_from_slice(&duration.to_be_bytes());
        make_box(b"mvhd", &body)
    }

    #[test]
    fn ftyp_then_moov_mvhd_v0_two_seconds() {
        let ftyp = make_box(b"ftyp", b"isom\0\0\x02\0isomiso2mp41");
        let mvhd = mvhd_v0(1000, 2000);
        let moov = make_box(b"moov", &mvhd);
        let mut file = ftyp;
        file.extend_from_slice(&moov);
        assert_eq!(duration_ms(&file), Some(2000));
    }

    #[test]
    fn mvhd_v1_wide_fields_half_second() {
        let mvhd = mvhd_v1(48000, 24000);
        let moov = make_box(b"moov", &mvhd);
        assert_eq!(duration_ms(&moov), Some(500));
    }

    #[test]
    fn skips_sibling_boxes_before_and_after_mvhd_inside_moov() {
        let udta = make_box(b"free", b"padding");
        let mvhd = mvhd_v0(600, 300);
        let trak = make_box(b"trak", b"track data here");
        let mut moov_body = udta;
        moov_body.extend_from_slice(&mvhd);
        moov_body.extend_from_slice(&trak);
        let moov = make_box(b"moov", &moov_body);
        assert_eq!(duration_ms(&moov), Some(500));
    }

    #[test]
    fn no_moov_box_is_none() {
        let ftyp = make_box(b"ftyp", b"isom");
        assert_eq!(duration_ms(&ftyp), None);
    }

    #[test]
    fn moov_without_mvhd_is_none() {
        let moov = make_box(b"moov", b"not an mvhd box in here");
        assert_eq!(duration_ms(&moov), None);
    }

    #[test]
    fn truncated_box_size_is_none() {
        let mut v = 100u32.to_be_bytes().to_vec();
        v.extend_from_slice(b"moov");
        assert_eq!(duration_ms(&v), None);
    }
}
