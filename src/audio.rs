//! Audio duration from raw encoded bytes — WAV today.
//!
//! Reads only the fixed-position and chunk-walked fields the RIFF/WAVE
//! spec guarantees; never decodes samples. Self-contained on raw bytes,
//! the same shape as [`crate::image::dimensions`]: no dependency on
//! `file::Kind`.
//!
//! MP3, OGG and FLAC durations are not parsed here yet — a documented
//! gap, not a silent wrong answer.

/// Duration in whole milliseconds, or `None` if the bytes are not a
/// recognized WAV file, or the `fmt ` / `data` chunks could not be
/// located.
pub fn duration_ms(bytes: &[u8]) -> Option<u64> {
    wav_duration_ms(bytes)
}

fn wav_duration_ms(bytes: &[u8]) -> Option<u64> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }

    let mut byte_rate: Option<u32> = None;
    let mut data_size: Option<u32> = None;
    let mut pos = 12usize;

    while pos + 8 <= bytes.len() {
        let chunk_id = &bytes[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().ok()?);
        let body_start = pos + 8;

        if chunk_id == b"fmt " {
            if body_start + 16 > bytes.len() {
                return None;
            }
            byte_rate = Some(u32::from_le_bytes(
                bytes[body_start + 8..body_start + 12].try_into().ok()?,
            ));
        } else if chunk_id == b"data" {
            data_size = Some(chunk_size);
        }

        if byte_rate.is_some() && data_size.is_some() {
            break;
        }

        // Chunks are padded to an even byte count.
        let advance = chunk_size as usize + (chunk_size as usize % 2);
        pos = body_start.checked_add(advance)?;
    }

    let byte_rate = byte_rate?;
    let data_size = data_size?;
    if byte_rate == 0 {
        return None;
    }
    Some((data_size as u64 * 1000) / byte_rate as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav_bytes(sample_rate: u32, channels: u16, bits_per_sample: u16, data: &[u8]) -> Vec<u8> {
        let block_align = channels * (bits_per_sample / 8);
        let byte_rate = sample_rate * block_align as u32;
        let mut fmt_body = Vec::new();
        fmt_body.extend_from_slice(&1u16.to_le_bytes()); // PCM
        fmt_body.extend_from_slice(&channels.to_le_bytes());
        fmt_body.extend_from_slice(&sample_rate.to_le_bytes());
        fmt_body.extend_from_slice(&byte_rate.to_le_bytes());
        fmt_body.extend_from_slice(&block_align.to_le_bytes());
        fmt_body.extend_from_slice(&bits_per_sample.to_le_bytes());

        let mut v = b"RIFF".to_vec();
        let riff_size = 4 + (8 + fmt_body.len()) + (8 + data.len());
        v.extend_from_slice(&(riff_size as u32).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&(fmt_body.len() as u32).to_le_bytes());
        v.extend_from_slice(&fmt_body);
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(data);
        v
    }

    #[test]
    fn one_second_of_mono_16bit_44100() {
        let data = vec![0u8; 44100 * 2];
        let bytes = wav_bytes(44100, 1, 16, &data);
        assert_eq!(duration_ms(&bytes), Some(1000));
    }

    #[test]
    fn half_second_of_stereo_16bit_48000() {
        let data = vec![0u8; 48000 * 4 / 2];
        let bytes = wav_bytes(48000, 2, 16, &data);
        assert_eq!(duration_ms(&bytes), Some(500));
    }

    #[test]
    fn skips_a_leading_list_chunk_to_find_fmt_and_data() {
        let mut v = b"RIFF".to_vec();
        let list_body = b"INFOsome metadata.".to_vec(); // even length, no pad byte needed
        let fmt_body_len = 16u32;
        let data = vec![0u8; 22050 * 2];
        let riff_size = 4 + (8 + list_body.len()) + (8 + fmt_body_len as usize) + (8 + data.len());
        v.extend_from_slice(&(riff_size as u32).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"LIST");
        v.extend_from_slice(&(list_body.len() as u32).to_le_bytes());
        v.extend_from_slice(&list_body);
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&fmt_body_len.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&22050u32.to_le_bytes());
        v.extend_from_slice(&44100u32.to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(&data);
        assert_eq!(duration_ms(&v), Some(1000));
    }

    #[test]
    fn pads_an_odd_sized_chunk_before_the_next_one() {
        let mut v = b"RIFF".to_vec();
        let odd_body = b"odd".to_vec(); // 3 bytes: odd, needs 1 pad byte
        let fmt_body_len = 16u32;
        let data = vec![0u8; 100];
        let riff_size = 4 + (8 + odd_body.len() + 1) + (8 + fmt_body_len as usize) + (8 + data.len());
        v.extend_from_slice(&(riff_size as u32).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"JUNK");
        v.extend_from_slice(&(odd_body.len() as u32).to_le_bytes());
        v.extend_from_slice(&odd_body);
        v.push(0); // pad byte, not counted in JUNK's own size
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&fmt_body_len.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&8000u32.to_le_bytes());
        v.extend_from_slice(&8000u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&8u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(&data);
        assert_eq!(duration_ms(&v), Some(12));
    }

    #[test]
    fn not_riff_is_none() {
        assert_eq!(duration_ms(b"not a wav file at all"), None);
    }

    #[test]
    fn riff_without_wave_is_none() {
        let mut v = b"RIFF".to_vec();
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(b"AVI ");
        assert_eq!(duration_ms(&v), None);
    }

    #[test]
    fn missing_data_chunk_is_none() {
        let mut v = b"RIFF".to_vec();
        let fmt_body_len = 16u32;
        let riff_size = 4 + (8 + fmt_body_len as usize);
        v.extend_from_slice(&(riff_size as u32).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&fmt_body_len.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&44100u32.to_le_bytes());
        v.extend_from_slice(&44100u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&8u16.to_le_bytes());
        assert_eq!(duration_ms(&v), None);
    }
}
