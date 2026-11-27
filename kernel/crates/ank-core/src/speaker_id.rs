//! Speaker verification via multi-feature acoustic voice fingerprinting (CORE-412 / EPIC 63).
//!
//! v2 Embedding (56 dims, pure Rust @ 16 kHz PCM16 mono):
//!   - Pre-emphasis (0.97) + Hamming-windowed 25ms frames (400 samples, 10ms hop).
//!   - Active-speech frame gating via 90th-percentile RMS so silence does not skew embeddings.
//!   - 20 Mel-spaced Goertzel log-energies -> Type-II DCT -> 16 liftered MFCCs (c1..c16):
//!       * 16 mean MFCCs
//!       * 16 std-dev MFCCs
//!       * 16 mean |delta-MFCC|
//!   - 3 Pitch (F0) descriptors via normalized autocorrelation in [75 Hz .. 400 Hz].
//!   - 5 Spectral shape descriptors (ZCR mean/std, centroid, 85% rolloff, spectral flatness).
//!
//! Maintains backward compatibility with legacy 9-dim fingerprints during verification.

use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

const FRAME_SAMPLES: usize = 400; // 25 ms @ 16 kHz
const HOP_SAMPLES: usize = 160; // 10 ms @ 16 kHz
const SAMPLE_RATE: f32 = 16_000.0;
const PRE_EMPHASIS: f32 = 0.97;

// Legacy v1 centers (kept for backward verification of previously enrolled 9-dim vectors)
const LEGACY_BAND_CENTERS: [f32; 8] = [125.0, 250.0, 500.0, 1000.0, 2000.0, 3500.0, 5500.0, 7000.0];
pub const LEGACY_FEATURE_DIM: usize = 9;

// v2 Mel-spaced filterbank centers spanning 110 Hz .. 7500 Hz
const MEL_CENTERS: [f32; 20] = [
    110.0, 185.0, 265.0, 355.0, 455.0, 565.0, 685.0, 820.0, 975.0, 1150.0, 1350.0, 1580.0, 1840.0,
    2140.0, 2490.0, 2900.0, 3400.0, 4100.0, 5200.0, 7000.0,
];
const NUM_CEPSTRA: usize = 16; // c1..c16 (c0 loudness excluded for mic gain invariance)
pub const FEATURE_DIM: usize = NUM_CEPSTRA * 3 + 3 + 5; // 48 + 3 + 5 = 56
pub const DEFAULT_THRESHOLD: f32 = 0.78;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrolledSpeakerProfile {
    pub id: String,
    pub label: String,
    pub fingerprint: Vec<f32>,
    pub threshold: f32,
    pub sample_count: usize,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WakeMatchResult {
    pub matched: bool,
    pub wake_only: bool,
    pub command: String,
    pub matched_phrase: String,
}

/// Goertzel algorithm: computes DFT power at `target_freq` Hz over `samples`.
fn goertzel_power(samples: &[f32], target_freq: f32) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let n = samples.len() as f32;
    let k = (n * target_freq / SAMPLE_RATE).round();
    let omega = 2.0 * PI * k / n;
    let coeff = 2.0 * omega.cos();
    let mut s_prev2 = 0.0f32;
    let mut s_prev1 = 0.0f32;
    for &x in samples {
        let s = x + coeff * s_prev1 - s_prev2;
        s_prev2 = s_prev1;
        s_prev1 = s;
    }
    let power = s_prev2 * s_prev2 + s_prev1 * s_prev1 - coeff * s_prev1 * s_prev2;
    power.max(0.0) / n
}

fn l2_normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|&x| x * x).sum::<f32>().sqrt();
    if norm > 1e-9 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Resamples 16-bit LE mono PCM linearly to `to_rate`.
pub fn resample_pcm16_mono(pcm_le: &[u8], from_rate: u32, to_rate: u32) -> Vec<u8> {
    if pcm_le.len() < 2 || from_rate == 0 || to_rate == 0 || from_rate == to_rate {
        return pcm_le.to_vec();
    }
    let src_len = pcm_le.len() / 2;
    let mut src = Vec::with_capacity(src_len);
    for i in 0..src_len {
        src.push(i16::from_le_bytes([pcm_le[2 * i], pcm_le[2 * i + 1]]) as f32);
    }
    let dst_len = ((src_len as f64 * f64::from(to_rate)) / f64::from(from_rate)).round() as usize;
    if dst_len == 0 {
        return Vec::new();
    }
    let ratio = f64::from(from_rate) / f64::from(to_rate);
    let mut out = Vec::with_capacity(dst_len * 2);
    for i in 0..dst_len {
        let pos = i as f64 * ratio;
        let idx = (pos as usize).min(src_len.saturating_sub(1));
        let next = (idx + 1).min(src_len.saturating_sub(1));
        let frac = (pos - idx as f64) as f32;
        let sample = (src[idx] + (src[next] - src[idx]) * frac).clamp(-32768.0, 32767.0) as i16;
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

/// Legacy 9-dimensional Goertzel fingerprint (retained solely for stored v1 compatibility).
fn extract_legacy_fingerprint_9(pcm_bytes: &[u8]) -> Option<Vec<f32>> {
    if pcm_bytes.len() < FRAME_SAMPLES * 2 {
        return None;
    }
    let count = pcm_bytes.len() / 2;
    let mut samples = Vec::with_capacity(count);
    for i in 0..count {
        samples.push(i16::from_le_bytes([pcm_bytes[2 * i], pcm_bytes[2 * i + 1]]) as f32 / 32768.0);
    }

    let mut sums = [0.0f64; LEGACY_FEATURE_DIM];
    let mut frame_count = 0usize;

    for frame in samples.chunks(FRAME_SAMPLES) {
        if frame.len() < FRAME_SAMPLES / 2 {
            break;
        }
        let zcr = frame
            .windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count() as f64
            / frame.len() as f64;
        sums[0] += zcr;
        for (b, &center_hz) in LEGACY_BAND_CENTERS.iter().enumerate() {
            sums[1 + b] += goertzel_power(frame, center_hz) as f64;
        }
        frame_count += 1;
    }
    if frame_count == 0 {
        return None;
    }
    let mut fp: Vec<f32> = sums
        .iter()
        .map(|&s| (s / frame_count as f64) as f32)
        .collect();
    l2_normalize(&mut fp);
    Some(fp)
}

/// Estimates fundamental pitch frequency (Hz) on a frame via normalized autocorrelation (75..400 Hz).
fn estimate_frame_pitch_hz(frame: &[f32]) -> Option<f32> {
    let min_lag = 40usize; // 400 Hz @ 16kHz
    let max_lag = 213usize.min(frame.len().saturating_sub(2)); // 75 Hz @ 16kHz
    if max_lag <= min_lag {
        return None;
    }

    let energy0: f32 = frame.iter().map(|&x| x * x).sum();
    if energy0 < 1e-5 {
        return None;
    }

    let mut best_lag = 0usize;
    let mut best_corr = 0.0f32;

    for lag in min_lag..=max_lag {
        let overlap = frame.len() - lag;
        let mut dot = 0.0f32;
        let mut e1 = 0.0f32;
        let mut e2 = 0.0f32;
        for i in 0..overlap {
            let a = frame[i];
            let b = frame[i + lag];
            dot += a * b;
            e1 += a * a;
            e2 += b * b;
        }
        let denom = (e1 * e2).sqrt();
        if denom > 1e-7 {
            let corr = dot / denom;
            if corr > best_corr {
                best_corr = corr;
                best_lag = lag;
            }
        }
    }

    if best_corr >= 0.35 && best_lag > 0 {
        Some(SAMPLE_RATE / best_lag as f32)
    } else {
        None
    }
}

/// Extracts a 56-dimensional voice fingerprint from raw PCM bytes (16 kHz, 16-bit LE, mono).
/// Returns `None` if the audio is too short (< 1 frame / 25ms).
pub fn extract_fingerprint(pcm_bytes: &[u8]) -> Option<Vec<f32>> {
    if pcm_bytes.len() < FRAME_SAMPLES * 2 {
        return None;
    }

    let count = pcm_bytes.len() / 2;
    let mut raw = Vec::with_capacity(count);
    for i in 0..count {
        raw.push(i16::from_le_bytes([pcm_bytes[2 * i], pcm_bytes[2 * i + 1]]) as f32 / 32768.0);
    }

    // Collect frame start indices and RMS levels for silence gating.
    let mut frame_starts = Vec::new();
    let mut frame_rms = Vec::new();
    let mut pos = 0usize;
    while pos + FRAME_SAMPLES <= raw.len() {
        let slice = &raw[pos..pos + FRAME_SAMPLES];
        let rms = (slice.iter().map(|&x| x * x).sum::<f32>() / FRAME_SAMPLES as f32).sqrt();
        frame_starts.push(pos);
        frame_rms.push(rms);
        pos += HOP_SAMPLES;
    }

    if frame_starts.is_empty() {
        return None;
    }

    let mut sorted_rms = frame_rms.clone();
    sorted_rms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p90_idx = ((sorted_rms.len() as f32 * 0.90) as usize).min(sorted_rms.len() - 1);
    let rms_gate = (sorted_rms[p90_idx] * 0.15).max(0.002);

    let active_indices: Vec<usize> = {
        let gated: Vec<usize> = frame_rms
            .iter()
            .enumerate()
            .filter_map(|(idx, &r)| (r >= rms_gate).then_some(idx))
            .collect();
        if gated.len() >= 3 {
            gated
        } else {
            (0..frame_starts.len()).collect()
        }
    };

    // Hamming window coefficients.
    let hamming: Vec<f32> = (0..FRAME_SAMPLES)
        .map(|i| 0.54 - 0.46 * (2.0 * PI * i as f32 / (FRAME_SAMPLES - 1) as f32).cos())
        .collect();

    let n_frames = active_indices.len();
    let mut mfcc_frames: Vec<[f64; NUM_CEPSTRA]> = Vec::with_capacity(n_frames);
    let mut pitch_norm_vals: Vec<f64> = Vec::with_capacity(n_frames);
    let mut zcr_vals: Vec<f64> = Vec::with_capacity(n_frames);
    let mut centroid_vals: Vec<f64> = Vec::with_capacity(n_frames);
    let mut rolloff_vals: Vec<f64> = Vec::with_capacity(n_frames);
    let mut flatness_vals: Vec<f64> = Vec::with_capacity(n_frames);

    let mel_len = MEL_CENTERS.len();
    let mut windowed = vec![0.0f32; FRAME_SAMPLES];

    for &frame_idx in &active_indices {
        let start = frame_starts[frame_idx];
        let raw_frame = &raw[start..start + FRAME_SAMPLES];

        // ZCR and pitch work on the raw un-emphasized frame.
        let zcr = raw_frame
            .windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count() as f64
            / FRAME_SAMPLES as f64;
        zcr_vals.push(zcr);

        if let Some(f0) = estimate_frame_pitch_hz(raw_frame) {
            pitch_norm_vals.push(((f0 - 75.0) / 325.0).clamp(0.0, 1.0) as f64);
        }

        // Pre-emphasis + Hamming window for spectral / MFCC extraction.
        windowed[0] = raw_frame[0] * hamming[0];
        for i in 1..FRAME_SAMPLES {
            windowed[i] = (raw_frame[i] - PRE_EMPHASIS * raw_frame[i - 1]) * hamming[i];
        }

        let mut band_powers = [0.0f64; 20];
        let mut log_energies = [0.0f64; 20];
        let mut total_power = 0.0f64;
        let mut weighted_freq = 0.0f64;
        let mut log_power_sum = 0.0f64;

        for (b, &center_hz) in MEL_CENTERS.iter().enumerate() {
            let p = f64::from(goertzel_power(&windowed, center_hz)).max(1e-12);
            band_powers[b] = p;
            log_energies[b] = p.ln();
            total_power += p;
            weighted_freq += p * f64::from(center_hz);
            log_power_sum += p.ln();
        }

        // Spectral shape ratios normalized into [0, 1].
        let centroid_ratio = if total_power > 1e-12 {
            (weighted_freq / total_power) / 7000.0
        } else {
            0.0
        };
        centroid_vals.push(centroid_ratio.clamp(0.0, 1.0));

        let mut accum_p = 0.0f64;
        let rolloff_target = total_power * 0.85;
        let mut rolloff_freq = 7000.0f64;
        for (b, &p) in band_powers.iter().enumerate() {
            accum_p += p;
            if accum_p >= rolloff_target {
                rolloff_freq = f64::from(MEL_CENTERS[b]);
                break;
            }
        }
        rolloff_vals.push((rolloff_freq / 7000.0).clamp(0.0, 1.0));

        let arith_mean = (total_power / mel_len as f64).max(1e-12);
        let geom_mean = (log_power_sum / mel_len as f64).exp();
        flatness_vals.push((geom_mean / arith_mean).clamp(0.0, 1.0));

        // Type-II DCT c1..c16 with sinusoidal liftering (L = 22).
        let mut mfcc = [0.0f64; NUM_CEPSTRA];
        let scale = (2.0 / mel_len as f64).sqrt();
        for (k_idx, mfcc_coeff) in mfcc.iter_mut().enumerate().take(NUM_CEPSTRA) {
            let c_idx = (k_idx + 1) as f64;
            let mut sum = 0.0f64;
            for (n, &log_e) in log_energies.iter().enumerate() {
                sum += log_e
                    * (std::f64::consts::PI * c_idx * (n as f64 + 0.5) / mel_len as f64).cos();
            }
            let lifter = 1.0 + 11.0 * (std::f64::consts::PI * c_idx / 22.0).sin();
            *mfcc_coeff = sum * scale * lifter;
        }
        mfcc_frames.push(mfcc);
    }

    if mfcc_frames.is_empty() {
        return None;
    }

    let nf = mfcc_frames.len() as f64;
    let mut out = vec![0.0f32; FEATURE_DIM];

    // 1. MFCC means (0..16), MFCC std-devs (16..32), mean |delta-MFCC| (32..48)
    let mut mfcc_means = [0.0f64; NUM_CEPSTRA];
    for frame in &mfcc_frames {
        for j in 0..NUM_CEPSTRA {
            mfcc_means[j] += frame[j];
        }
    }
    for mean in &mut mfcc_means {
        *mean /= nf;
    }

    let mut mfcc_vars = [0.0f64; NUM_CEPSTRA];
    for frame in &mfcc_frames {
        for j in 0..NUM_CEPSTRA {
            let d = frame[j] - mfcc_means[j];
            mfcc_vars[j] += d * d;
        }
    }

    let mut mfcc_deltas = [0.0f64; NUM_CEPSTRA];
    if mfcc_frames.len() > 1 {
        for w in mfcc_frames.windows(2) {
            for j in 0..NUM_CEPSTRA {
                mfcc_deltas[j] += (w[1][j] - w[0][j]).abs();
            }
        }
        let d_count = (mfcc_frames.len() - 1) as f64;
        for d in &mut mfcc_deltas {
            *d /= d_count;
        }
    }

    // Balance weights so MFCC dynamics, pitch, and spectral shape all contribute meaningfully.
    for j in 0..NUM_CEPSTRA {
        out[j] = (mfcc_means[j] * 0.12) as f32;
        out[NUM_CEPSTRA + j] = ((mfcc_vars[j] / nf).sqrt() * 0.12) as f32;
        out[2 * NUM_CEPSTRA + j] = (mfcc_deltas[j] * 0.14) as f32;
    }

    // 2. Pitch F0 stats (48..51)
    let voiced_ratio = pitch_norm_vals.len() as f64 / nf;
    let (pitch_mean, pitch_std) = mean_and_std(&pitch_norm_vals);
    out[48] = (pitch_mean * 1.4) as f32;
    out[49] = (pitch_std * 1.0) as f32;
    out[50] = (voiced_ratio * 0.9) as f32;

    // 3. Spectral shape stats (51..56)
    let (zcr_mean, zcr_std) = mean_and_std(&zcr_vals);
    let (cent_mean, _) = mean_and_std(&centroid_vals);
    let (roll_mean, _) = mean_and_std(&rolloff_vals);
    let (flat_mean, _) = mean_and_std(&flatness_vals);
    out[51] = (zcr_mean * 2.5) as f32;
    out[52] = (zcr_std * 2.0) as f32;
    out[53] = (cent_mean * 1.2) as f32;
    out[54] = (roll_mean * 1.0) as f32;
    out[55] = (flat_mean * 1.0) as f32;

    l2_normalize(&mut out);
    Some(out)
}

fn mean_and_std(vals: &[f64]) -> (f64, f64) {
    if vals.is_empty() {
        return (0.0, 0.0);
    }
    let n = vals.len() as f64;
    let mean = vals.iter().sum::<f64>() / n;
    let var = vals.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n;
    (mean, var.sqrt())
}

/// Averages 1..N PCM enrollments into a single L2-normalized centroid vector.
pub fn merge_enrollments(samples: &[Vec<u8>]) -> Option<Vec<f32>> {
    let mut sum = vec![0.0f32; FEATURE_DIM];
    let mut count = 0usize;
    for pcm in samples {
        if let Some(fp) = extract_fingerprint(pcm) {
            if fp.len() == FEATURE_DIM {
                for (acc, v) in sum.iter_mut().zip(fp.iter()) {
                    *acc += *v;
                }
                count += 1;
            }
        }
    }
    if count == 0 {
        return None;
    }
    for acc in &mut sum {
        *acc /= count as f32;
    }
    l2_normalize(&mut sum);
    Some(sum)
}

/// Cosine similarity between two L2-normalized vectors (range −1..1, higher = more similar).
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum()
}

/// Verify whether `incoming_pcm` matches `stored_fingerprint`.
/// Supports both legacy 9-dim fingerprints and v2 56-dim embeddings seamlessly.
pub fn verify(incoming_pcm: &[u8], stored_fingerprint: &[f32], threshold: f32) -> (bool, f32) {
    let incoming = if stored_fingerprint.len() == LEGACY_FEATURE_DIM {
        extract_legacy_fingerprint_9(incoming_pcm)
    } else {
        extract_fingerprint(incoming_pcm)
    };
    match incoming {
        Some(incoming_fp) => {
            let score = cosine_similarity(&incoming_fp, stored_fingerprint);
            (score >= threshold, score)
        }
        None => (false, 0.0),
    }
}

/// Verifies `incoming_pcm` against multiple enrolled voice profiles.
/// Returns `(accepted, matched_label, best_score, effective_threshold)`.
pub fn verify_multi(
    incoming_pcm: &[u8],
    profiles: &[EnrolledSpeakerProfile],
) -> (bool, Option<String>, f32, f32) {
    if profiles.is_empty() {
        return (true, None, 1.0, DEFAULT_THRESHOLD);
    }

    let fp_v2 = extract_fingerprint(incoming_pcm);
    let mut fp_v1: Option<Vec<f32>> = None;

    let mut best_accepted = false;
    let mut best_label: Option<String> = None;
    let mut best_score = -1.0f32;
    let mut best_threshold = DEFAULT_THRESHOLD;

    for profile in profiles {
        let incoming = if profile.fingerprint.len() == LEGACY_FEATURE_DIM {
            if fp_v1.is_none() {
                fp_v1 = extract_legacy_fingerprint_9(incoming_pcm);
            }
            fp_v1.as_deref()
        } else {
            fp_v2.as_deref()
        };

        let Some(vec) = incoming else {
            continue;
        };

        let score = cosine_similarity(vec, &profile.fingerprint);
        if score > best_score {
            best_score = score;
            best_threshold = profile.threshold;
            if score >= profile.threshold {
                best_accepted = true;
                best_label = Some(profile.label.clone());
            } else {
                best_accepted = false;
                best_label = None;
            }
        }
    }

    if best_score < -0.5 {
        best_score = 0.0;
    }

    (best_accepted, best_label, best_score, best_threshold)
}

fn fold_spanish_token(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.to_lowercase().chars() {
        let mapped = match ch {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            _ => ch,
        };
        if mapped.is_alphanumeric() {
            out.push(mapped);
        }
    }
    out
}

/// Checks whether a transcript begins with a dynamic wake phrase for `agent_name` or `"Aegis"`,
/// e.g. `"Aegis escuchame, anotá $5000"`, `"Jarvis escuchame"`, `"Oye Jarvis ..."`.
/// Preserves the original casing/punctuation of the remaining user command.
pub fn match_and_strip_wake_phrase(transcript: &str, agent_name: &str) -> WakeMatchResult {
    let trimmed = transcript.trim();
    if trimmed.is_empty() {
        return WakeMatchResult {
            matched: false,
            wake_only: false,
            command: String::new(),
            matched_phrase: String::new(),
        };
    }

    // Build word spans in original text: (byte_start, byte_end, normalized_token)
    let mut words: Vec<(usize, usize, String)> = Vec::new();
    let mut cur_start: Option<usize> = None;
    for (idx, ch) in trimmed.char_indices() {
        if ch.is_alphanumeric() {
            if cur_start.is_none() {
                cur_start = Some(idx);
            }
        } else if let Some(s) = cur_start.take() {
            let norm = fold_spanish_token(&trimmed[s..idx]);
            if !norm.is_empty() {
                words.push((s, idx, norm));
            }
        }
    }
    if let Some(s) = cur_start.take() {
        let norm = fold_spanish_token(&trimmed[s..]);
        if !norm.is_empty() {
            words.push((s, trimmed.len(), norm));
        }
    }

    if words.is_empty() {
        return WakeMatchResult {
            matched: false,
            wake_only: false,
            command: String::new(),
            matched_phrase: String::new(),
        };
    }

    let custom_norm = fold_spanish_token(agent_name);
    let mut aliases: Vec<String> = Vec::new();
    if !custom_norm.is_empty() {
        aliases.push(custom_norm);
    }
    for base in ["aegis", "eyis", "egis", "ayis", "hegis", "aegys", "ejis"] {
        if !aliases.iter().any(|a| a == base) {
            aliases.push(base.to_string());
        }
    }

    let greetings = ["hola", "oye", "ok", "hey", "che", "eh", "buenas"];
    let cue_words = ["escuchame", "escucha", "atendeme", "decime"];
    let is_alias = |tok: &str| -> bool { aliases.iter().any(|a| a == tok) };

    // Optional single leading greeting ("hola aegis escuchame ..." or "oye jarvis ...")
    let start_idx = if words.len() >= 2 && greetings.contains(&words[0].2.as_str()) {
        1
    } else {
        0
    };

    let mut matched_end_idx: Option<usize> = None;

    if start_idx < words.len() {
        let first = words[start_idx].2.as_str();
        // Pattern 1: "[greeting] <name> [escuchame] [por favor]"
        if is_alias(first) {
            let mut end_i = start_idx;
            if end_i + 1 < words.len() && cue_words.contains(&words[end_i + 1].2.as_str()) {
                end_i += 1;
            }
            matched_end_idx = Some(end_i);
        }
        // Pattern 2: "[greeting] escuchame <name>"
        else if cue_words.contains(&first)
            && start_idx + 1 < words.len()
            && is_alias(&words[start_idx + 1].2)
        {
            matched_end_idx = Some(start_idx + 1);
        }
    }

    let Some(mut end_i) = matched_end_idx else {
        return WakeMatchResult {
            matched: false,
            wake_only: false,
            command: String::new(),
            matched_phrase: String::new(),
        };
    };

    // Strip optional polite filler right after the wake phrase ("por favor").
    if end_i + 2 < words.len() && words[end_i + 1].2 == "por" && words[end_i + 2].2 == "favor" {
        end_i += 2;
    }

    let phrase_end_byte = words[end_i].1;
    let matched_phrase = trimmed[..phrase_end_byte]
        .trim_matches(|c: char| !c.is_alphanumeric() && c != ' ')
        .to_string();

    let rest = trimmed[phrase_end_byte..]
        .trim_start_matches([',', '.', ':', ';', '!', '?', '-', ' '])
        .trim()
        .to_string();

    let wake_only = rest.is_empty();
    WakeMatchResult {
        matched: true,
        wake_only,
        command: rest,
        matched_phrase,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_synthetic_voice(f0: f32, formants: &[f32], secs: f32) -> Vec<u8> {
        let total = (16000.0 * secs) as usize;
        let mut pcm = Vec::with_capacity(total * 2);
        for i in 0..total {
            let t = i as f32 / 16000.0;
            // Harmonic pulse train + two formant resonances for realistic pitch + timbre separation.
            let mut v = 0.45 * (2.0 * PI * f0 * t).sin() + 0.25 * (4.0 * PI * f0 * t).sin();
            for (idx, &ff) in formants.iter().enumerate() {
                let amp = 0.25 / (idx as f32 + 1.0);
                v += amp * (2.0 * PI * ff * t).sin();
            }
            let sample = (v.clamp(-0.98, 0.98) * 20000.0) as i16;
            pcm.extend_from_slice(&sample.to_le_bytes());
        }
        pcm
    }

    #[test]
    fn same_voice_high_similarity() {
        let a = make_synthetic_voice(125.0, &[650.0, 1450.0], 1.2);
        let fp = extract_fingerprint(&a).unwrap();
        assert_eq!(fp.len(), FEATURE_DIM);
        let (ok, score) = verify(&a, &fp, DEFAULT_THRESHOLD);
        assert!(ok, "Same audio should verify: score={:.3}", score);
        assert!(score > 0.99, "Identical audio should have similarity ~1.0");
    }

    #[test]
    fn different_voice_low_similarity() {
        let male_low = make_synthetic_voice(110.0, &[500.0, 1100.0], 1.2);
        let female_high = make_synthetic_voice(265.0, &[1800.0, 4200.0], 1.2);
        let fp_a = extract_fingerprint(&male_low).unwrap();
        let (ok, score) = verify(&female_high, &fp_a, DEFAULT_THRESHOLD);
        assert!(!ok, "Different voice should not verify: score={:.3}", score);
        assert!(
            score < DEFAULT_THRESHOLD,
            "Score {:.3} should be below threshold {}",
            score,
            DEFAULT_THRESHOLD
        );
    }

    #[test]
    fn silence_padding_does_not_corrupt_fingerprint() {
        let voice = make_synthetic_voice(145.0, &[720.0, 1900.0], 1.0);
        let mut padded = vec![0u8; 16000 * 2 * 2]; // 2s silence before
        padded.extend_from_slice(&voice);
        padded.extend_from_slice(&vec![0u8; 16000 * 2 * 2]); // 2s silence after

        let fp_clean = extract_fingerprint(&voice).unwrap();
        let fp_padded = extract_fingerprint(&padded).unwrap();
        let sim = cosine_similarity(&fp_clean, &fp_padded);
        assert!(
            sim > 0.92,
            "Active-speech RMS gating must keep padded voice similarity high (got {:.3})",
            sim
        );
    }

    #[test]
    fn multi_profile_verification_matches_second_speaker_and_rejects_intruder() {
        let spk1_a = make_synthetic_voice(115.0, &[520.0, 1200.0], 1.0);
        let spk1_b = make_synthetic_voice(118.0, &[540.0, 1240.0], 1.0);
        let spk2 = make_synthetic_voice(210.0, &[950.0, 2400.0], 1.0);
        let intruder = make_synthetic_voice(340.0, &[3200.0, 5600.0], 1.0);

        let profiles = vec![
            EnrolledSpeakerProfile {
                id: "v1".to_string(),
                label: "Tavo".to_string(),
                fingerprint: merge_enrollments(&[spk1_a, spk1_b]).unwrap(),
                threshold: DEFAULT_THRESHOLD,
                sample_count: 2,
                created_at: "2026-11-26".to_string(),
            },
            EnrolledSpeakerProfile {
                id: "v2".to_string(),
                label: "Invitado".to_string(),
                fingerprint: extract_fingerprint(&spk2).unwrap(),
                threshold: DEFAULT_THRESHOLD,
                sample_count: 1,
                created_at: "2026-11-26".to_string(),
            },
        ];

        let (acc2, lbl2, sc2, _) = verify_multi(&spk2, &profiles);
        assert!(
            acc2,
            "Second enrolled profile must match (score={:.3})",
            sc2
        );
        assert_eq!(lbl2.as_deref(), Some("Invitado"));

        let (acc_int, lbl_int, sc_int, _) = verify_multi(&intruder, &profiles);
        assert!(
            !acc_int,
            "Unregistered speaker must be rejected (score={:.3})",
            sc_int
        );
        assert!(lbl_int.is_none());
    }

    #[test]
    fn dynamic_wake_phrase_matches_aegis_and_custom_assistant_name() {
        let r1 = match_and_strip_wake_phrase("Aegis escuchame, anotá $5000 de café", "Aegis");
        assert!(r1.matched);
        assert!(!r1.wake_only);
        assert_eq!(r1.command, "anotá $5000 de café");

        let r2 = match_and_strip_wake_phrase("Hola Jarvis escúchame", "Jarvis");
        assert!(r2.matched);
        assert!(r2.wake_only);
        assert!(r2.command.is_empty());

        let r3 = match_and_strip_wake_phrase("Oye Nova por favor recordame comprar pan", "Nova");
        assert!(r3.matched);
        assert!(!r3.wake_only);
        assert_eq!(r3.command, "recordame comprar pan");

        let r4 = match_and_strip_wake_phrase("Hola Marcos, ¿cómo estuvo tu día?", "Jarvis");
        assert!(!r4.matched);
    }
}
