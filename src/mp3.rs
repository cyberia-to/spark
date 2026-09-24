//! MP3 duration from raw encoded bytes — MPEG-1 Layer III only.
//!
//! Walks every frame rather than reading one header and extrapolating: a
//! constant-bitrate file has one bitrate to read, but a variable-bitrate
//! file (the common case for encoders like LAME) changes bitrate per frame,
//! so only summing each frame's own sample count gives a correct duration
//! either way. No dependency on `file::Kind` — same shape as
//! [`crate::audio::duration_ms`] and [`crate::image::dimensions`].
//!
//! MPEG-2 and MPEG-2.5 Layer III (used for very low sample rates) and
//! Layers I/II are not parsed — a documented gap, not a silent wrong
//! answer: the first frame is required to be MPEG-1 Layer III, or this
//! returns `None`.

const SAMPLES_PER_FRAME: u64 = 1152;

// index 0..=15; 0 (free) and 15 (bad) are not usable frame bitrates.
const BITRATE_KBPS: [u16; 16] =
    [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0];

// index 0..=2; 3 is reserved.
const SAMPLE_RATE_HZ: [u32; 4] = [44100, 48000, 32000, 0];

struct FrameHeader {
    bitrate_bps: u32,
    sample_rate_hz: u32,
    padding: u32,
}

/// The byte length of an ID3v2 tag at the start of `bytes`, or `0` if none.
fn id3v2_len(bytes: &[u8]) -> usize {
    if bytes.len() < 10 || &bytes[0..3] != b"ID3" {
        return 0;
    }
    let synchsafe = &bytes[6..10];
    if synchsafe.iter().any(|b| b & 0x80 != 0) {
        return 0;
    }
    let size = (synchsafe[0] as usize) << 21
        | (synchsafe[1] as usize) << 14
        | (synchsafe[2] as usize) << 7
        | (synchsafe[3] as usize);
    10 + size
}

/// Parse a 4-byte MPEG-1 Layer III frame header at `bytes[pos..]`.
/// Returns `None` if it is not a sync, is a different version/layer, or
/// names a reserved bitrate/sample-rate slot.
fn parse_frame_header(bytes: &[u8], pos: usize) -> Option<FrameHeader> {
    let b = bytes.get(pos..pos + 4)?;
    if b[0] != 0xFF || b[1] & 0xE0 != 0xE0 {
        return None;
    }
    let version = (b[1] >> 3) & 0x03;
    let layer = (b[1] >> 1) & 0x03;
    if version != 0b11 || layer != 0b01 {
        return None; // not MPEG-1 Layer III
    }
    let bitrate_index = (b[2] >> 4) & 0x0F;
    let sample_rate_index = (b[2] >> 2) & 0x03;
    let padding = (b[2] >> 1) & 0x01;

    let kbps = BITRATE_KBPS[bitrate_index as usize];
    let hz = SAMPLE_RATE_HZ[sample_rate_index as usize];
    if kbps == 0 || hz == 0 {
        return None;
    }
    Some(FrameHeader {
        bitrate_bps: kbps as u32 * 1000,
        sample_rate_hz: hz,
        padding: padding as u32,
    })
}

fn frame_len(h: &FrameHeader) -> usize {
    (144 * h.bitrate_bps / h.sample_rate_hz + h.padding) as usize
}

/// Duration in whole milliseconds, or `None` if the bytes do not start
/// (after any ID3v2 tag) with a valid MPEG-1 Layer III frame.
pub fn duration_ms(bytes: &[u8]) -> Option<u64> {
    let mut pos = id3v2_len(bytes);
    // An ID3v1 tag is a fixed 128-byte trailer, never audio data.
    let end = if bytes.len() >= 128 && &bytes[bytes.len() - 128..bytes.len() - 125] == b"TAG" {
        bytes.len() - 128
    } else {
        bytes.len()
    };

    let first = parse_frame_header(bytes, pos)?;
    let sample_rate_hz = first.sample_rate_hz;

    let mut total_samples: u64 = 0;
    let mut frames: u64 = 0;
    loop {
        let Some(h) = parse_frame_header(bytes, pos) else { break };
        let len = frame_len(&h);
        if len == 0 || pos + len > end {
            break;
        }
        total_samples += SAMPLES_PER_FRAME;
        frames += 1;
        pos += len;
    }
    if frames == 0 {
        return None;
    }
    Some(total_samples * 1000 / sample_rate_hz as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_bytes(kbps_index: u8, rate_index: u8, padding: bool) -> Vec<u8> {
        let h = FrameHeader {
            bitrate_bps: BITRATE_KBPS[kbps_index as usize] as u32 * 1000,
            sample_rate_hz: SAMPLE_RATE_HZ[rate_index as usize],
            padding: padding as u32,
        };
        let len = frame_len(&h);
        let mut v = vec![0u8; len];
        v[0] = 0xFF;
        v[1] = 0xE0 | (0b11 << 3) | (0b01 << 1); // MPEG-1, Layer III
        v[2] = (kbps_index << 4) | (rate_index << 2) | ((padding as u8) << 1);
        v[3] = 0x00;
        v
    }

    #[test]
    fn ten_frames_at_44100_128kbps() {
        let mut data = Vec::new();
        for _ in 0..10 {
            data.extend(frame_bytes(9, 0, false)); // 128kbps, 44100Hz
        }
        // 10 frames * 1152 samples / 44100 Hz = 261.22...ms -> 261ms
        assert_eq!(duration_ms(&data), Some(10 * 1152 * 1000 / 44100));
    }

    #[test]
    fn skips_id3v2_header() {
        let mut data = b"ID3".to_vec();
        data.push(3); // version
        data.push(0); // revision
        data.push(0); // flags
        let tag_body_len: u32 = 20;
        data.extend_from_slice(&[
            ((tag_body_len >> 21) & 0x7F) as u8,
            ((tag_body_len >> 14) & 0x7F) as u8,
            ((tag_body_len >> 7) & 0x7F) as u8,
            (tag_body_len & 0x7F) as u8,
        ]);
        data.extend(vec![0u8; tag_body_len as usize]);
        data.extend(frame_bytes(4, 0, false)); // 56kbps, 44100Hz
        data.extend(frame_bytes(4, 0, false));
        assert_eq!(duration_ms(&data), Some(2 * 1152 * 1000 / 44100));
    }

    #[test]
    fn excludes_trailing_id3v1_tag() {
        let mut data = Vec::new();
        data.extend(frame_bytes(9, 0, false));
        data.extend(frame_bytes(9, 0, false));
        data.extend(b"TAG");
        data.extend(vec![0u8; 125]);
        assert_eq!(duration_ms(&data), Some(2 * 1152 * 1000 / 44100));
    }

    #[test]
    fn variable_bitrate_sums_each_frames_own_samples() {
        let mut data = Vec::new();
        data.extend(frame_bytes(2, 1, false)); // 40kbps, 48000Hz
        data.extend(frame_bytes(14, 1, false)); // 320kbps, 48000Hz
        data.extend(frame_bytes(6, 1, false)); // 80kbps, 48000Hz
        assert_eq!(duration_ms(&data), Some(3 * 1152 * 1000 / 48000));
    }

    #[test]
    fn padding_bit_extends_frame_by_one_byte() {
        let padded = frame_bytes(9, 0, true);
        let unpadded = frame_bytes(9, 0, false);
        assert_eq!(padded.len(), unpadded.len() + 1);
    }

    #[test]
    fn not_mp3_is_none() {
        assert_eq!(duration_ms(b"not an mp3 file at all, no sync here"), None);
    }

    #[test]
    fn reserved_bitrate_index_is_none() {
        let mut data = frame_bytes(9, 0, false);
        data[2] = (0x0F << 4) | (0 << 2); // bitrate index 15 = bad
        assert_eq!(duration_ms(&data), None);
    }

    #[test]
    fn mpeg2_version_is_not_parsed() {
        let mut data = frame_bytes(9, 0, false);
        data[1] = 0xE0 | (0b10 << 3) | (0b01 << 1); // MPEG-2, Layer III
        assert_eq!(duration_ms(&data), None);
    }

    #[test]
    fn short_buffer_does_not_panic() {
        assert_eq!(duration_ms(&[0xFF, 0xE0]), None);
    }
}
