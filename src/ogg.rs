//! Audio duration for Ogg Vorbis — walk pages to the last granule position.
//!
//! Reads only the fields the Ogg container and Vorbis identification
//! header spec guarantee: the first page's sample rate, and the highest
//! granule position (a sample count) carried by any later page. Never
//! decodes a Vorbis packet. Self-contained on raw bytes, the same shape
//! as [`crate::audio::duration_ms`] and [`crate::flac::duration_ms`].

const CAPTURE_PATTERN: &[u8; 4] = b"OggS";
const PAGE_HEADER_LEN: usize = 27;
/// Granule position sentinel meaning "no packet finishes on this page".
const NO_GRANULE: u64 = u64::MAX;

/// Duration in whole milliseconds, or `None` if the bytes are not a
/// recognized Ogg Vorbis stream, or no page carries a valid granule
/// position.
pub fn duration_ms(bytes: &[u8]) -> Option<u64> {
    let sample_rate = vorbis_sample_rate(bytes)?;
    if sample_rate == 0 {
        return None;
    }
    let total_samples = last_granule_position(bytes)?;
    Some((total_samples * 1000) / sample_rate as u64)
}

/// The Vorbis identification header is the first packet of the first
/// page: `\x01vorbis` then version(4) channels(1) sample_rate(4) ...
fn vorbis_sample_rate(bytes: &[u8]) -> Option<u32> {
    let (header_len, body_len, _granule) = page_at(bytes, 0)?;
    let body = bytes.get(header_len..header_len + body_len)?;
    // 1 byte packet_type + 6 "vorbis" + 4 vorbis_version + 1 channels,
    // then the 4-byte little-endian sample rate at offset 12.
    if body.len() < 16 || body[0] != 0x01 || &body[1..7] != b"vorbis" {
        return None;
    }
    Some(u32::from_le_bytes(body[12..16].try_into().ok()?))
}

/// Scan every page in order, returning the granule position of the last
/// page whose granule position is not the "no packet finishes here"
/// sentinel.
fn last_granule_position(bytes: &[u8]) -> Option<u64> {
    let mut pos = 0usize;
    let mut last = None;
    while pos < bytes.len() {
        let (header_len, body_len, granule) = page_at(bytes, pos)?;
        if granule != NO_GRANULE {
            last = Some(granule);
        }
        pos = pos.checked_add(header_len)?.checked_add(body_len)?;
    }
    last
}

/// Parse the page header at `pos`, returning `(header_len, body_len,
/// granule_position)`. `header_len` includes the segment table.
fn page_at(bytes: &[u8], pos: usize) -> Option<(usize, usize, u64)> {
    let header = bytes.get(pos..pos + PAGE_HEADER_LEN)?;
    if &header[0..4] != CAPTURE_PATTERN {
        return None;
    }
    let granule = u64::from_le_bytes(header[6..14].try_into().ok()?);
    let page_segments = header[26] as usize;
    let segment_table = bytes.get(pos + PAGE_HEADER_LEN..pos + PAGE_HEADER_LEN + page_segments)?;
    let body_len: usize = segment_table.iter().map(|&b| b as usize).sum();
    Some((PAGE_HEADER_LEN + page_segments, body_len, granule))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vorbis_ident_packet(sample_rate: u32, channels: u8) -> Vec<u8> {
        let mut p = vec![0x01];
        p.extend_from_slice(b"vorbis");
        p.extend_from_slice(&0u32.to_le_bytes()); // vorbis_version
        p.push(channels);
        p.extend_from_slice(&sample_rate.to_le_bytes());
        p.extend_from_slice(&0i32.to_le_bytes()); // bitrate_maximum
        p.extend_from_slice(&0i32.to_le_bytes()); // bitrate_nominal
        p.extend_from_slice(&0i32.to_le_bytes()); // bitrate_minimum
        p.push(0); // blocksize_0/1
        p.push(1); // framing_flag
        p
    }

    fn page(granule: u64, serial: u32, seq: u32, body: &[u8]) -> Vec<u8> {
        let mut v = b"OggS".to_vec();
        v.push(0); // version
        v.push(0); // header_type_flag
        v.extend_from_slice(&granule.to_le_bytes());
        v.extend_from_slice(&serial.to_le_bytes());
        v.extend_from_slice(&seq.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes()); // CRC, unchecked here
        // Lace the whole body into 255-byte segments, as a real
        // encoder would; a final segment shorter than 255 terminates
        // the packet.
        let mut segments = Vec::new();
        let mut remaining = body.len();
        loop {
            if remaining >= 255 {
                segments.push(255u8);
                remaining -= 255;
            } else {
                segments.push(remaining as u8);
                break;
            }
        }
        v.push(segments.len() as u8);
        v.extend_from_slice(&segments);
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn two_second_stream_at_48000() {
        let ident = vorbis_ident_packet(48000, 2);
        let mut v = page(0, 1, 0, &ident);
        v.extend_from_slice(&page(96000, 1, 1, &[0u8; 10]));
        assert_eq!(duration_ms(&v), Some(2000));
    }

    #[test]
    fn takes_the_last_real_granule_skipping_trailing_sentinel() {
        let ident = vorbis_ident_packet(44100, 1);
        let mut v = page(0, 1, 0, &ident);
        v.extend_from_slice(&page(44100, 1, 1, &[0u8; 5]));
        // A trailing page whose packet does not finish here.
        v.extend_from_slice(&page(NO_GRANULE, 1, 2, &[0u8; 5]));
        assert_eq!(duration_ms(&v), Some(1000));
    }

    #[test]
    fn body_spanning_more_than_255_bytes_is_laced_correctly() {
        let ident = vorbis_ident_packet(8000, 1);
        let mut v = page(0, 1, 0, &ident);
        let body = vec![0u8; 300];
        v.extend_from_slice(&page(4000, 1, 1, &body));
        assert_eq!(duration_ms(&v), Some(500));
    }

    #[test]
    fn not_ogg_is_none() {
        assert_eq!(duration_ms(b"not an ogg file at all, no capture pattern"), None);
    }

    #[test]
    fn ogg_without_vorbis_signature_is_none() {
        let mut not_vorbis = vec![0x7f];
        not_vorbis.extend_from_slice(b"FISHEAD");
        not_vorbis.extend_from_slice(&[0u8; 20]);
        let v = page(0, 1, 0, &not_vorbis);
        assert_eq!(duration_ms(&v), None);
    }

    #[test]
    fn truncated_header_is_none_not_a_panic() {
        let mut v = b"OggS".to_vec();
        v.extend_from_slice(&[0u8; 5]); // short of a full 27-byte header
        assert_eq!(duration_ms(&v), None);
    }

    #[test]
    fn truncated_segment_table_is_none_not_a_panic() {
        let ident = vorbis_ident_packet(44100, 2);
        let mut v = page(0, 1, 0, &ident);
        // A second page whose header claims segments but whose segment
        // table (and body) is cut off before those bytes arrive.
        let mut short_second = page(44100, 1, 1, &[0u8; 5]);
        short_second.truncate(PAGE_HEADER_LEN);
        v.extend_from_slice(&short_second);
        assert_eq!(duration_ms(&v), None);
    }
}
