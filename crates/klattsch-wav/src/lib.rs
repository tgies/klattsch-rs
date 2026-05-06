//! RIFF/WAVE encoder.
//!
//! Encodes mono `f32` PCM as 16-bit signed PCM with optional peak
//! normalization and `LIST INFO` metadata.

/// Optional metadata embedded in a `LIST INFO` chunk after the data chunk.
/// `software` populates `ISFT` (RIFF info software identifier);
/// `comment` populates `ICMT` (free-form comment).
#[derive(Clone, Debug, Default)]
pub struct WavMetadata<'a> {
    pub software: Option<&'a str>,
    pub comment: Option<&'a str>,
}

/// Options for [`encode_wav`].
#[derive(Clone, Debug)]
pub struct WavOptions<'a> {
    /// Peak-normalize so the loudest sample sits at this absolute amplitude
    /// (0..=1). Default 0.95. Set to 0 to disable normalization.
    pub peak_normalize: f32,
    pub metadata: Option<WavMetadata<'a>>,
}

impl Default for WavOptions<'_> {
    fn default() -> Self {
        Self {
            peak_normalize: 0.95,
            metadata: None,
        }
    }
}

/// Result of [`encode_wav`]: the encoded byte stream and the gain factor that
/// was applied for peak normalization (1.0 if disabled or all-zero input).
#[derive(Clone, Debug)]
pub struct EncodedWav {
    pub bytes: Vec<u8>,
    pub gain: f32,
}

/// Encode mono `f32` PCM as a 16-bit signed-PCM RIFF/WAVE file.
pub fn encode_wav(samples: &[f32], sample_rate: u32, opts: &WavOptions<'_>) -> EncodedWav {
    let mut gain = 1.0f32;
    if opts.peak_normalize > 0.0 {
        let mut peak = 0.0f32;
        for s in samples {
            let a = s.abs();
            if a > peak {
                peak = a;
            }
        }
        if peak > 0.0 {
            gain = opts.peak_normalize / peak;
        }
    }

    let info_chunk = opts.metadata.as_ref().and_then(build_info_chunk);
    let data_bytes = samples.len() * 2;
    let info_len = info_chunk.as_ref().map_or(0, Vec::len);
    let total_size = 44 + data_bytes + info_len;

    let mut buf = Vec::with_capacity(total_size);

    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&((total_size - 8) as u32).to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt size
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    buf.extend_from_slice(&1u16.to_le_bytes()); // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate (mono * 16-bit)
    buf.extend_from_slice(&2u16.to_le_bytes()); // block align
    buf.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&(data_bytes as u32).to_le_bytes());
    for s in samples {
        let v = (s * gain).clamp(-1.0, 1.0);
        let q = (v * 32767.0).round() as i16;
        buf.extend_from_slice(&q.to_le_bytes());
    }

    if let Some(info) = info_chunk {
        buf.extend_from_slice(&info);
    }

    EncodedWav { bytes: buf, gain }
}

fn build_info_chunk(meta: &WavMetadata<'_>) -> Option<Vec<u8>> {
    // Sub-chunk tuples: (4-byte ID, payload bytes).
    let mut subs: Vec<(&[u8; 4], &[u8])> = Vec::new();
    if let Some(s) = meta.software {
        subs.push((b"ISFT", s.as_bytes()));
    }
    if let Some(c) = meta.comment {
        subs.push((b"ICMT", c.as_bytes()));
    }
    if subs.is_empty() {
        return None;
    }

    // LIST payload = "INFO" fourcc (4 bytes) + each sub (8-byte header
    // + data + pad).
    let mut payload_size: usize = 4;
    for (_, data) in &subs {
        payload_size += 8 + data.len() + (data.len() % 2);
    }

    let mut out = Vec::with_capacity(8 + payload_size);
    out.extend_from_slice(b"LIST");
    out.extend_from_slice(&(payload_size as u32).to_le_bytes());
    out.extend_from_slice(b"INFO");
    for (id, data) in subs {
        out.extend_from_slice(id);
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        if data.len() % 2 == 1 {
            out.push(0); // pad to word align
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_44_bytes_when_no_metadata() {
        let samples = vec![0.0f32; 1000];
        let r = encode_wav(
            &samples,
            48_000,
            &WavOptions {
                peak_normalize: 0.0,
                ..Default::default()
            },
        );
        // 44 byte header + 2*1000 = 2044
        assert_eq!(r.bytes.len(), 44 + 2000);
        assert_eq!(&r.bytes[0..4], b"RIFF");
        assert_eq!(&r.bytes[8..12], b"WAVE");
        assert_eq!(&r.bytes[12..16], b"fmt ");
        assert_eq!(&r.bytes[36..40], b"data");
    }

    #[test]
    fn peak_normalize_scales_to_target() {
        let samples = vec![0.5f32, -0.5];
        let r = encode_wav(
            &samples,
            48_000,
            &WavOptions {
                peak_normalize: 0.95,
                ..Default::default()
            },
        );
        assert!((r.gain - (0.95 / 0.5)).abs() < 1e-6);
    }

    #[test]
    fn peak_normalize_skips_silence() {
        let samples = vec![0.0f32; 100];
        let r = encode_wav(
            &samples,
            48_000,
            &WavOptions {
                peak_normalize: 0.95,
                ..Default::default()
            },
        );
        assert_eq!(r.gain, 1.0);
    }

    #[test]
    fn metadata_appends_list_chunk() {
        let samples = vec![0.0f32; 100];
        let r = encode_wav(
            &samples,
            48_000,
            &WavOptions {
                peak_normalize: 0.0,
                metadata: Some(WavMetadata {
                    software: Some("klattsch"),
                    comment: Some("HH AH"),
                }),
            },
        );
        let header_and_data = 44 + 200;
        assert!(r.bytes.len() > header_and_data);
        assert_eq!(&r.bytes[header_and_data..header_and_data + 4], b"LIST");
    }

    #[test]
    fn fmt_chunk_encodes_sample_rate() {
        let samples = vec![0.0f32; 10];
        let r = encode_wav(
            &samples,
            44_100,
            &WavOptions {
                peak_normalize: 0.0,
                ..Default::default()
            },
        );
        // sample rate at offset 24, little-endian u32
        let sr = u32::from_le_bytes(r.bytes[24..28].try_into().unwrap());
        assert_eq!(sr, 44_100);
    }
}
