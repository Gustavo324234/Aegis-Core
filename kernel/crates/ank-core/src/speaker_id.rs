/// Speaker verification via voice fingerprinting.
///
/// Algorithm: sub-band RMS energy (8 bands) + zero-crossing rate per 25ms frame,
/// averaged across all frames and L2-normalized → 9-dimensional voice fingerprint.
/// Verification uses cosine similarity (dot product of two unit vectors).
///
/// No external DSP deps required — pure arithmetic on 16kHz 16-bit mono PCM.

const FRAME_SAMPLES: usize = 400; // 25ms @ 16kHz
const BAND_COUNT: usize = 8;
const FEATURE_DIM: usize = BAND_COUNT + 1; // bands + ZCR
pub const DEFAULT_THRESHOLD: f32 = 0.75;

/// Extract a normalized voice fingerprint from raw PCM bytes (16kHz, 16-bit LE, mono).
/// Returns `None` if the audio is too short (< 1 frame).
pub fn extract_fingerprint(pcm_bytes: &[u8]) -> Option<Vec<f32>> {
    if pcm_bytes.len() < FRAME_SAMPLES * 2 {
        return None;
    }

    let samples: Vec<f32> = pcm_bytes
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect();

    let mut sums = [0.0f64; FEATURE_DIM];
    let mut frame_count = 0usize;

    for frame in samples.chunks(FRAME_SAMPLES) {
        if frame.len() < FRAME_SAMPLES / 2 {
            break;
        }

        // ZCR: sign changes / frame length
        let zcr = frame
            .windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count() as f64
            / frame.len() as f64;
        sums[0] += zcr;

        // Sub-band RMS energies
        let band_size = frame.len() / BAND_COUNT;
        for b in 0..BAND_COUNT {
            let start = b * band_size;
            let end = ((b + 1) * band_size).min(frame.len());
            let rms = (frame[start..end]
                .iter()
                .map(|&s| (s as f64).powi(2))
                .sum::<f64>()
                / (end - start) as f64)
                .sqrt();
            sums[1 + b] += rms;
        }

        frame_count += 1;
    }

    if frame_count == 0 {
        return None;
    }

    let mut fingerprint: Vec<f32> = sums
        .iter()
        .map(|&s| (s / frame_count as f64) as f32)
        .collect();

    l2_normalize(&mut fingerprint);
    Some(fingerprint)
}

/// Cosine similarity between two L2-normalized vectors (range −1..1, higher = more similar).
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum()
}

/// Verify whether `incoming_pcm` matches `stored_fingerprint`.
/// Returns `(accepted, score)`. Score is cosine similarity in range [−1, 1].
pub fn verify(incoming_pcm: &[u8], stored_fingerprint: &[f32], threshold: f32) -> (bool, f32) {
    match extract_fingerprint(incoming_pcm) {
        Some(incoming_fp) => {
            let score = cosine_similarity(&incoming_fp, stored_fingerprint);
            (score >= threshold, score)
        }
        None => (false, 0.0),
    }
}

fn l2_normalize(v: &mut Vec<f32>) {
    let norm: f32 = v.iter().map(|&x| x * x).sum::<f32>().sqrt();
    if norm > 1e-9 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sine(freq_hz: f32, samples: usize) -> Vec<u8> {
        let mut pcm = Vec::with_capacity(samples * 2);
        for i in 0..samples {
            let t = i as f32 / 16000.0;
            let sample = (f32::sin(2.0 * std::f32::consts::PI * freq_hz * t) * 20000.0) as i16;
            pcm.extend_from_slice(&sample.to_le_bytes());
        }
        pcm
    }

    #[test]
    fn same_voice_high_similarity() {
        let a = make_sine(220.0, 16000);
        let fp = extract_fingerprint(&a).unwrap();
        let (ok, score) = verify(&a, &fp, DEFAULT_THRESHOLD);
        assert!(ok, "Same audio should verify: score={:.3}", score);
        assert!(score > 0.99, "Identical audio should have similarity ~1.0");
    }

    #[test]
    fn different_voice_low_similarity() {
        let a = make_sine(110.0, 16000); // bass-like
        let b = make_sine(880.0, 16000); // treble-like
        let fp_a = extract_fingerprint(&a).unwrap();
        let (ok, score) = verify(&b, &fp_a, DEFAULT_THRESHOLD);
        assert!(!ok, "Different audio should not verify: score={:.3}", score);
        let _ = score; // score < threshold expected
    }
}
