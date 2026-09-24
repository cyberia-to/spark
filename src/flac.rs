//! Audio duration for FLAC — read straight from the STREAMINFO block.
//!
//! Reads only the mandatory first metadata block the FLAC spec
//! guarantees; never decodes frames. Self-contained on raw bytes, the
//! same shape as [`crate::audio::duration_ms`].

/// Duration in whole milliseconds, or `None` if the bytes are not a
/// recognized FLAC stream, or `STREAMINFO` could not be read.
pub fn duration_ms(bytes: &[u8]) -> Option<u64> {
    if bytes.len() < 4 || &bytes[0..4] != b"fLaC" {
        return None;
    }
    // The first metadata block is always STREAMINFO (type 0): 1-byte
    // header (last-flag + 7-bit type), 3-byte big-endian body length,
    // then a 34-byte body.
    if bytes.len() < 4 + 4 + 34 {
        return None;
    }
    let header = bytes[4];
    let block_type = header & 0x7F;
    if block_type != 0 {
        return None;
    }
    let body_len = u32::from_be_bytes([0, bytes[5], bytes[6], bytes[7]]) as usize;
    if body_len < 34 {
        return None;
    }
    let body = &bytes[8..8 + 34];

    // Bytes 10..18 of STREAMINFO pack, MSB first: sample_rate (20 bits),
    // channels - 1 (3 bits), bits_per_sample - 1 (5 bits), total_samples
    // (36 bits) — 64 bits exactly.
    let packed = u64::from_be_bytes(body[10..18].try_into().ok()?);
    let sample_rate = (packed >> 44) & 0xF_FFFF;
    let total_samples = packed & 0xF_FFFF_FFFF;

    if sample_rate == 0 {
        return None;
    }
    Some((total_samples * 1000) / sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn streaminfo_body(sample_rate: u32, channels: u8, bits_per_sample: u8, total_samples: u64) -> [u8; 34] {
        let mut body = [0u8; 34];
        body[0..2].copy_from_slice(&4096u16.to_be_bytes()); // min block size
        body[2..4].copy_from_slice(&4096u16.to_be_bytes()); // max block size
        // min/max frame size (24 bits each) left at 0

        let packed: u64 = ((sample_rate as u64 & 0xF_FFFF) << 44)
            | (((channels as u64 - 1) & 0x7) << 41)
            | (((bits_per_sample as u64 - 1) & 0x1F) << 36)
            | (total_samples & 0xF_FFFF_FFFF);
        body[10..18].copy_from_slice(&packed.to_be_bytes());
        body
    }

    fn flac_bytes(sample_rate: u32, channels: u8, bits_per_sample: u8, total_samples: u64) -> Vec<u8> {
        let mut v = b"fLaC".to_vec();
        v.push(0x80); // last-metadata-block flag set, type 0 (STREAMINFO)
        let body = streaminfo_body(sample_rate, channels, bits_per_sample, total_samples);
        v.extend_from_slice(&[0, 0, body.len() as u8]); // 24-bit body length
        v.extend_from_slice(&body);
        v
    }

    #[test]
    fn one_second_at_44100() {
        let bytes = flac_bytes(44100, 2, 16, 44100);
        assert_eq!(duration_ms(&bytes), Some(1000));
    }

    #[test]
    fn two_and_a_half_seconds_at_48000() {
        let bytes = flac_bytes(48000, 1, 24, 120_000);
        assert_eq!(duration_ms(&bytes), Some(2500));
    }

    #[test]
    fn not_flac_is_none() {
        assert_eq!(duration_ms(b"not a flac stream"), None);
    }

    #[test]
    fn wrong_first_block_type_is_none() {
        let mut v = b"fLaC".to_vec();
        v.push(0x84); // type 4 (VORBIS_COMMENT), not STREAMINFO
        v.extend_from_slice(&[0, 0, 34]);
        v.extend_from_slice(&[0u8; 34]);
        assert_eq!(duration_ms(&v), None);
    }

    #[test]
    fn truncated_streaminfo_is_none() {
        let mut v = b"fLaC".to_vec();
        v.push(0x80);
        v.extend_from_slice(&[0, 0, 34]);
        v.extend_from_slice(&[0u8; 10]); // short body
        assert_eq!(duration_ms(&v), None);
    }
}
