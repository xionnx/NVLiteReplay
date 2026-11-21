use std::f32::consts::PI;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::audio::WasapiStream;

pub const VOICE_SAMPLE_RATE: usize = 16_000;
const FFT_SIZE: usize = 512;
const FRAME_LEN: usize = 400; // 25 ms @ 16 kHz
const HOP_LEN: usize = 160;   // 10 ms @ 16 kHz (100 fps)
const NUM_MEL_FILTERS: usize = 26;
const NUM_CEPSTRAL: usize = 13; // c1..c13 (exclude DC c0 for loudness invariance)
pub const FEAT_DIM: usize = NUM_CEPSTRAL * 2; // 13 CMVN MFCC + 13 Delta = 26
const MAX_UTTERANCE_SAMPLES: usize = 44_800; // 2.8 seconds max
const MIN_UTTERANCE_SAMPLES: usize = 4_800;  // 0.30 seconds min
const PRE_ROLL_SAMPLES: usize = 2_400;       // 0.15 seconds pre-speech context
const MAX_TEMPLATES: usize = 3;

#[derive(Clone, Debug)]
pub struct VoiceTemplate {
    pub phrase: String,
    pub duration_secs: f32,
    pub frames: Vec<[f32; FEAT_DIM]>,
}

struct MfccFrontend {
    hamming: [f32; FRAME_LEN],
    mel_bins: [usize; NUM_MEL_FILTERS + 2],
    dct_matrix: [[f32; NUM_MEL_FILTERS]; NUM_CEPSTRAL],
}

impl MfccFrontend {
    fn new() -> Self {
        let mut hamming = [0.0f32; FRAME_LEN];
        for (n, w) in hamming.iter_mut().enumerate() {
            *w = 0.54 - 0.46 * (2.0 * PI * n as f32 / (FRAME_LEN - 1) as f32).cos();
        }

        let hz_to_mel = |hz: f32| 2595.0 * (1.0 + hz / 700.0).log10();
        let mel_to_hz = |mel: f32| 700.0 * (10.0f32.powf(mel / 2595.0) - 1.0);

        let low_mel = hz_to_mel(80.0);
        let high_mel = hz_to_mel(7600.0);
        let mut mel_bins = [0usize; NUM_MEL_FILTERS + 2];
        for (i, bin) in mel_bins.iter_mut().enumerate() {
            let mel = low_mel + (high_mel - low_mel) * (i as f32) / ((NUM_MEL_FILTERS + 1) as f32);
            let hz = mel_to_hz(mel);
            let k = (((FFT_SIZE + 1) as f32 * hz) / (VOICE_SAMPLE_RATE as f32)).floor() as usize;
            *bin = k.min(FFT_SIZE / 2);
        }

        let mut dct_matrix = [[0.0f32; NUM_MEL_FILTERS]; NUM_CEPSTRAL];
        let scale = (2.0 / NUM_MEL_FILTERS as f32).sqrt();
        for (k, row) in dct_matrix.iter_mut().enumerate() {
            let c_idx = (k + 1) as f32; // Skip c0 so loudness shifts don't distort template distance
            for (m, val) in row.iter_mut().enumerate() {
                *val = scale * (PI * c_idx * (m as f32 + 0.5) / (NUM_MEL_FILTERS as f32)).cos();
            }
        }

        Self {
            hamming,
            mel_bins,
            dct_matrix,
        }
    }

    fn extract_features(&self, samples: &[f32]) -> Vec<[f32; FEAT_DIM]> {
        if samples.len() < FRAME_LEN {
            return Vec::new();
        }

        // Pre-emphasis filter: y[n] = x[n] - 0.97 * x[n - 1]
        let mut pre = Vec::with_capacity(samples.len());
        pre.push(samples[0]);
        for i in 1..samples.len() {
            pre.push(samples[i] - 0.97 * samples[i - 1]);
        }

        let mut raw_ceps: Vec<[f32; NUM_CEPSTRAL]> = Vec::with_capacity(pre.len() / HOP_LEN + 1);
        let mut re = [0.0f32; FFT_SIZE];
        let mut im = [0.0f32; FFT_SIZE];
        let mut pow_spec = [0.0f32; FFT_SIZE / 2 + 1];
        let mut mel_energies = [0.0f32; NUM_MEL_FILTERS];

        let mut start = 0usize;
        while start + FRAME_LEN <= pre.len() {
            re.fill(0.0);
            im.fill(0.0);
            for i in 0..FRAME_LEN {
                re[i] = pre[start + i] * self.hamming[i];
            }

            fft_radix2_512(&mut re, &mut im);

            for k in 0..=(FFT_SIZE / 2) {
                pow_spec[k] = (re[k] * re[k] + im[k] * im[k]) / (FFT_SIZE as f32);
            }

            for m in 0..NUM_MEL_FILTERS {
                let left = self.mel_bins[m];
                let center = self.mel_bins[m + 1];
                let right = self.mel_bins[m + 2];
                let mut e = 1e-9f32;
                if center > left {
                    for k in left..center {
                        e += pow_spec[k] * ((k - left) as f32 / (center - left) as f32);
                    }
                }
                if right > center {
                    for k in center..=right {
                        e += pow_spec[k] * ((right - k) as f32 / (right - center) as f32);
                    }
                } else if center <= FFT_SIZE / 2 {
                    e += pow_spec[center];
                }
                mel_energies[m] = e.ln();
            }

            let mut c = [0.0f32; NUM_CEPSTRAL];
            for k in 0..NUM_CEPSTRAL {
                let mut sum = 0.0f32;
                for m in 0..NUM_MEL_FILTERS {
                    sum += self.dct_matrix[k][m] * mel_energies[m];
                }
                c[k] = sum;
            }
            raw_ceps.push(c);
            start += HOP_LEN;
        }

        let t_len = raw_ceps.len();
        if t_len < 5 {
            return Vec::new();
        }

        // Cepstral Mean & Variance Normalization (CMVN) for microphone / gain invariance
        let mut mean = [0.0f32; NUM_CEPSTRAL];
        for c in &raw_ceps {
            for k in 0..NUM_CEPSTRAL {
                mean[k] += c[k];
            }
        }
        for m in &mut mean {
            *m /= t_len as f32;
        }

        let mut var = [0.0f32; NUM_CEPSTRAL];
        for c in &raw_ceps {
            for k in 0..NUM_CEPSTRAL {
                let d = c[k] - mean[k];
                var[k] += d * d;
            }
        }
        let mut std_dev = [1.0f32; NUM_CEPSTRAL];
        for k in 0..NUM_CEPSTRAL {
            std_dev[k] = (var[k] / t_len as f32).sqrt().max(0.05);
        }

        for c in &mut raw_ceps {
            for k in 0..NUM_CEPSTRAL {
                c[k] = (c[k] - mean[k]) / std_dev[k];
            }
        }

        // Combine normalized MFCC (0..13) + Delta-MFCC (13..26)
        let mut out = Vec::with_capacity(t_len);
        for t in 0..t_len {
            let prev = t.saturating_sub(1);
            let next = (t + 1).min(t_len - 1);
            let mut feat = [0.0f32; FEAT_DIM];
            for k in 0..NUM_CEPSTRAL {
                feat[k] = raw_ceps[t][k];
                feat[NUM_CEPSTRAL + k] = (raw_ceps[next][k] - raw_ceps[prev][k]) * 0.5;
            }
            out.push(feat);
        }
        out
    }
}

#[inline]
fn fft_radix2_512(re: &mut [f32; FFT_SIZE], im: &mut [f32; FFT_SIZE]) {
    let n = FFT_SIZE;
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while (j & bit) != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }

    let mut len = 2usize;
    while len <= n {
        let half = len >> 1;
        let ang = -2.0 * PI / (len as f32);
        let wlen_r = ang.cos();
        let wlen_i = ang.sin();
        let mut i = 0;
        while i < n {
            let mut w_r = 1.0f32;
            let mut w_i = 0.0f32;
            for k in 0..half {
                let u_r = re[i + k];
                let u_i = im[i + k];
                let v_r = re[i + k + half] * w_r - im[i + k + half] * w_i;
                let v_i = re[i + k + half] * w_i + im[i + k + half] * w_r;

                re[i + k] = u_r + v_r;
                im[i + k] = u_i + v_i;
                re[i + k + half] = u_r - v_r;
                im[i + k + half] = u_i - v_i;

                let next_wr = w_r * wlen_r - w_i * wlen_i;
                w_i = w_r * wlen_i + w_i * wlen_r;
                w_r = next_wr;
            }
            i += len;
        }
        len <<= 1;
    }
}

#[inline]
fn frame_distance_mse(a: &[f32; FEAT_DIM], b: &[f32; FEAT_DIM]) -> f32 {
    let mut sum = 0.0f32;
    for k in 0..FEAT_DIM {
        let d = a[k] - b[k];
        sum += d * d;
    }
    sum / (FEAT_DIM as f32)
}

/// Sakoe-Chiba Banded Dynamic Time Warping distance mapped to 0.0..100.0% similarity score.
pub fn dtw_similarity_percent(a: &[[f32; FEAT_DIM]], b: &[[f32; FEAT_DIM]]) -> f32 {
    let m = a.len();
    let n = b.len();
    if m < 8 || n < 8 {
        return 0.0;
    }

    // Duration ratio sanity check prevents short noise clicks matching multi-word phrases
    let max_len = m.max(n) as f32;
    let min_len = m.min(n) as f32;
    if max_len / min_len > 1.85 {
        return 0.0;
    }

    let band = ((max_len * 0.28).ceil() as usize).max(12);
    let inf = 1e9f32;
    let mut prev = vec![inf; n + 1];
    let mut curr = vec![inf; n + 1];
    prev[0] = 0.0;

    for i in 1..=m {
        curr.fill(inf);
        let mapped_j = (i * n) / m;
        let j_start = mapped_j.saturating_sub(band).max(1);
        let j_end = (mapped_j + band).min(n);

        for j in j_start..=j_end {
            let cost = frame_distance_mse(&a[i - 1], &b[j - 1]);
            let diag = prev[j - 1];
            let up = prev[j] * 1.12;
            let left = curr[j - 1] * 1.12;
            curr[j] = cost + diag.min(up).min(left);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    let total_cost = prev[n];
    if total_cost >= inf * 0.5 {
        return 0.0;
    }
    let avg_mse = total_cost / ((m + n) as f32 * 0.5);
    // Calibrated mapping: avg_mse 0.0 => 100%, 0.55 => ~70%, 1.85 => 0%
    (1.0 - (avg_mse / 1.85)).clamp(0.0, 1.0) * 100.0
}

#[derive(Clone, Debug)]
pub struct VoiceUiSnapshot {
    pub template_count: usize,
    pub template_durations: Vec<f32>,
    pub phrase_label: String,
    pub is_enrolling: bool,
    pub is_speaking: bool,
    pub mic_level: f32,
    pub last_status: String,
    pub last_confidence: f32,
    pub memory_bytes: usize,
}

struct VoiceInner {
    templates: Vec<VoiceTemplate>,
    phrase_label: String,
    is_enrolling: bool,
    enroll_deadline: Option<Instant>,
    mic_level: f32,
    is_speaking: bool,
    last_status: String,
    last_confidence: f32,

    // Streaming VAD buffers @ 16kHz mono
    pending_samples: Vec<f32>,
    pre_roll: Vec<f32>,
    utterance: Vec<f32>,
    noise_rms: f32,
    speech_streak: u32,
    silence_streak: u32,
    last_trigger_at: Option<Instant>,
}

pub struct VoiceRecognizer {
    frontend: MfccFrontend,
    inner: Mutex<VoiceInner>,
}

impl VoiceRecognizer {
    pub fn new(default_phrase: &str) -> Arc<Self> {
        let mut inner = VoiceInner {
            templates: Vec::new(),
            phrase_label: default_phrase.to_string(),
            is_enrolling: false,
            enroll_deadline: None,
            mic_level: 0.0,
            is_speaking: false,
            last_status: "Siap — rekam 2-3 contoh suara frasa Anda.".to_string(),
            last_confidence: 0.0,
            pending_samples: Vec::with_capacity(2048),
            pre_roll: Vec::with_capacity(PRE_ROLL_SAMPLES),
            utterance: Vec::with_capacity(MAX_UTTERANCE_SAMPLES),
            noise_rms: 0.004,
            speech_streak: 0,
            silence_streak: 0,
            last_trigger_at: None,
        };
        if let Some((loaded_phrase, loaded_tpls)) = load_voice_file() {
            if !loaded_phrase.is_empty() {
                inner.phrase_label = loaded_phrase;
            }
            inner.templates = loaded_tpls;
            if !inner.templates.is_empty() {
                inner.last_status = format!(
                    "Siap ({} contoh suara '{}' tersimpan).",
                    inner.templates.len(),
                    inner.phrase_label
                );
            }
        }
        Arc::new(Self {
            frontend: MfccFrontend::new(),
            inner: Mutex::new(inner),
        })
    }

    pub fn set_phrase_label(&self, phrase: &str) {
        let mut g = self.inner.lock().unwrap();
        g.phrase_label = phrase.to_string();
        for t in &mut g.templates {
            t.phrase = phrase.to_string();
        }
        save_voice_file(&g.phrase_label, &g.templates);
    }

    pub fn clear_templates(&self) {
        let mut g = self.inner.lock().unwrap();
        g.templates.clear();
        g.last_confidence = 0.0;
        g.last_status = "Semua contoh suara dihapus. Silakan rekam ulang.".to_string();
        save_voice_file(&g.phrase_label, &g.templates);
    }

    pub fn is_enrolling(&self) -> bool {
        self.inner.lock().unwrap().is_enrolling
    }

    pub fn ui_snapshot(&self) -> VoiceUiSnapshot {
        let g = self.inner.lock().unwrap();
        let mem_bytes: usize = g
            .templates
            .iter()
            .map(|t| t.frames.len() * FEAT_DIM * std::mem::size_of::<f32>())
            .sum();
        VoiceUiSnapshot {
            template_count: g.templates.len(),
            template_durations: g.templates.iter().map(|t| t.duration_secs).collect(),
            phrase_label: g.phrase_label.clone(),
            is_enrolling: g.is_enrolling,
            is_speaking: g.is_speaking,
            mic_level: g.mic_level,
            last_status: g.last_status.clone(),
            last_confidence: g.last_confidence,
            memory_bytes: mem_bytes,
        }
    }

    pub fn begin_enrollment(
        self: &Arc<Self>,
        engine_running: bool,
        phrase: &str,
        mic_device_id: Option<&str>,
    ) {
        {
            let mut g = self.inner.lock().unwrap();
            let still_active = g.enroll_deadline.is_some_and(|d| Instant::now() < d);
            if g.is_enrolling && still_active {
                return;
            }
            g.phrase_label = phrase.to_string();
            g.is_enrolling = true;
            g.enroll_deadline = Some(Instant::now() + Duration::from_secs(5));
            g.is_speaking = false;
            g.speech_streak = 0;
            g.silence_streak = 0;
            g.utterance.clear();
            g.pre_roll.clear();
            g.last_status = format!("Bicara sekarang: \"{}\"...", g.phrase_label);
        }

        // If replay engine is standby, open a temporary shared-mode WASAPI mic stream for ~5s
        if !engine_running {
            let rec_clone = Arc::clone(self);
            let dev_id_owned = mic_device_id
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            std::thread::spawn(move || {
                unsafe {
                    let _ = windows::Win32::System::Com::CoInitializeEx(
                        None,
                        windows::Win32::System::Com::COINIT_MULTITHREADED,
                    );
                }
                let mut mic = WasapiStream::new();
                if !mic.initialize_with_device(false, dev_id_owned.as_deref()) {
                    let mut g = rec_clone.inner.lock().unwrap();
                    g.is_enrolling = false;
                    g.last_status = "Gagal mengakses mikrofon Windows (cek izin Mic).".to_string();
                    drop(g);
                    drop(mic);
                    unsafe { windows::Win32::System::Com::CoUninitialize(); }
                    return;
                }
                mic.set_voice_tap_enabled(true);
                let mut mono_16k = Vec::with_capacity(2048);
                let deadline = Instant::now() + Duration::from_secs(5);
                while Instant::now() < deadline {
                    mic.drain_packets();
                    mic.drain_voice_tap_16k(&mut mono_16k);
                    if !mono_16k.is_empty() {
                        rec_clone.process_16k_chunk(&mono_16k, 65, 8, false);
                    }
                    let done = !rec_clone.inner.lock().unwrap().is_enrolling;
                    if done {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
                {
                    let mut g = rec_clone.inner.lock().unwrap();
                    if g.is_enrolling {
                        g.is_enrolling = false;
                        g.is_speaking = false;
                        g.mic_level = 0.0;
                        if g.utterance.len() >= MIN_UTTERANCE_SAMPLES {
                            let utt = std::mem::take(&mut g.utterance);
                            drop(g);
                            rec_clone.finish_enrollment_utterance(&utt);
                        } else {
                            g.last_status = "Waktu rekam habis — suara kurang keras / belum terdeteksi.".to_string();
                        }
                    } else {
                        g.mic_level = 0.0;
                    }
                }
                drop(mic);
                unsafe { windows::Win32::System::Com::CoUninitialize(); }
            });
        }
    }

    fn finish_enrollment_utterance(&self, samples: &[f32]) {
        let feats = self.frontend.extract_features(samples);
        let dur_s = samples.len() as f32 / (VOICE_SAMPLE_RATE as f32);
        let mut g = self.inner.lock().unwrap();
        g.is_enrolling = false;
        g.is_speaking = false;
        if feats.len() < 12 {
            g.last_status = "Contoh suara terlalu pendek — coba ucapkan penuh.".to_string();
            return;
        }
        if g.templates.len() >= MAX_TEMPLATES {
            g.templates.remove(0);
        }
        let tpl = VoiceTemplate {
            phrase: g.phrase_label.clone(),
            duration_secs: dur_s,
            frames: feats,
        };
        g.templates.push(tpl);
        let count = g.templates.len();
        save_voice_file(&g.phrase_label, &g.templates);
        g.last_status = format!(
            "Contoh suara {}/{} tersimpan ({:.1} dtk)!",
            count, MAX_TEMPLATES, dur_s
        );
    }

    /// Feed mono 16kHz samples from WASAPI mic tap; returns `true` when trigger phrase matches!
    pub fn process_16k_chunk(
        &self,
        samples_16k: &[f32],
        sensitivity_percent: u32,
        cooldown_seconds: u32,
        listening_active: bool,
    ) -> bool {
        if samples_16k.is_empty() {
            return false;
        }

        let mut completed_utterance: Option<(Vec<f32>, bool)> = None;
        {
            let mut g = self.inner.lock().unwrap();
            if g.is_enrolling {
                if let Some(dl) = g.enroll_deadline {
                    if Instant::now() > dl && !g.is_speaking {
                        g.is_enrolling = false;
                        g.last_status = "Timeout — silakan klik Rekam & langsung ucapkan frasa.".to_string();
                        return false;
                    }
                }
            } else if !listening_active {
                g.is_speaking = false;
                g.mic_level = 0.0;
                return false;
            }

            let inner = &mut *g;
            inner.pending_samples.extend_from_slice(samples_16k);

            let mut hop = [0.0f32; HOP_LEN];
            while inner.pending_samples.len() >= HOP_LEN {
                hop.copy_from_slice(&inner.pending_samples[..HOP_LEN]);
                let mut sum_sq = 0.0f32;
                for &s in &hop {
                    sum_sq += s * s;
                }
                let rms = (sum_sq / (HOP_LEN as f32)).sqrt();
                let visual_lvl = (rms * 7.5).clamp(0.0, 1.0);
                inner.mic_level = inner.mic_level * 0.65 + visual_lvl * 0.35;

                let onset_thresh = (0.014f32).max(inner.noise_rms * 3.2);
                let offset_thresh = (0.009f32).max(inner.noise_rms * 2.0);

                if !inner.is_speaking {
                    inner.noise_rms = (inner.noise_rms * 0.992 + rms.min(0.025) * 0.008).clamp(0.0015, 0.03);
                    if rms >= onset_thresh {
                        inner.speech_streak += 1;
                    } else {
                        inner.speech_streak = 0;
                    }

                    // Maintain 150ms pre-roll ring so initial consonants aren't chopped
                    inner.pre_roll.extend_from_slice(&hop);
                    if inner.pre_roll.len() > PRE_ROLL_SAMPLES {
                        let excess = inner.pre_roll.len() - PRE_ROLL_SAMPLES;
                        inner.pre_roll.drain(..excess);
                    }

                    if inner.speech_streak >= 3 {
                        inner.is_speaking = true;
                        inner.silence_streak = 0;
                        inner.utterance.clear();
                        inner.utterance.extend_from_slice(&inner.pre_roll);
                    }
                } else {
                    inner.utterance.extend_from_slice(&hop);
                    if rms < offset_thresh {
                        inner.silence_streak += 1;
                    } else {
                        inner.silence_streak = 0;
                    }

                    if inner.silence_streak >= 28 || inner.utterance.len() >= MAX_UTTERANCE_SAMPLES {
                        let trim_end = if inner.silence_streak >= 28 {
                            inner.utterance.len().saturating_sub(20 * HOP_LEN)
                        } else {
                            inner.utterance.len()
                        };
                        let mut utt = std::mem::take(&mut inner.utterance);
                        utt.truncate(trim_end);
                        let enrolling = inner.is_enrolling;
                        inner.is_speaking = false;
                        inner.speech_streak = 0;
                        inner.silence_streak = 0;
                        inner.pre_roll.clear();

                        if utt.len() >= MIN_UTTERANCE_SAMPLES {
                            completed_utterance = Some((utt, enrolling));
                            inner.pending_samples.drain(..HOP_LEN);
                            break;
                        }
                    }
                }
                inner.pending_samples.drain(..HOP_LEN);
            }
        }

        if let Some((utt, was_enrolling)) = completed_utterance {
            if was_enrolling {
                self.finish_enrollment_utterance(&utt);
                return false;
            } else {
                let feats = self.frontend.extract_features(&utt);
                if feats.len() < 10 {
                    return false;
                }
                let mut g = self.inner.lock().unwrap();
                if g.templates.is_empty() {
                    return false;
                }
                let mut best_score = 0.0f32;
                for tpl in &g.templates {
                    let s = dtw_similarity_percent(&feats, &tpl.frames);
                    if s > best_score {
                        best_score = s;
                    }
                }
                g.last_confidence = best_score;
                let thresh = sensitivity_percent as f32;
                if best_score >= thresh {
                    let in_cooldown = g
                        .last_trigger_at
                        .map(|t| t.elapsed() < Duration::from_secs(cooldown_seconds as u64))
                        .unwrap_or(false);
                    if in_cooldown {
                        g.last_status = format!(
                            "Cocok {:.0}% ({}) — cooldown aktif",
                            best_score, g.phrase_label
                        );
                        return false;
                    }
                    g.last_trigger_at = Some(Instant::now());
                    g.last_status = format!(
                        "TERDETEKSI \"{}\" ({:.0}%) — Menyimpan klip!",
                        g.phrase_label, best_score
                    );
                    return true;
                } else {
                    g.last_status = format!(
                        "Ucapan lain terabaikan (skor {:.0}% < {}%)",
                        best_score, sensitivity_percent
                    );
                }
            }
        }
        false
    }
}

fn voice_file_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.join("NVLiteReplayRustVoiceRecog.voice");
        }
    }
    PathBuf::from("NVLiteReplayRustVoiceRecog.voice")
}

fn save_voice_file(phrase: &str, templates: &[VoiceTemplate]) {
    let mut out = String::new();
    out.push_str("NVVOICE1\n");
    out.push_str(&format!("PHRASE={}\n", phrase.replace('\n', " ")));
    for tpl in templates {
        out.push_str(&format!("TPL {:.3} {}\n", tpl.duration_secs, tpl.frames.len()));
        for fr in &tpl.frames {
            for (idx, val) in fr.iter().enumerate() {
                if idx > 0 {
                    out.push(' ');
                }
                out.push_str(&format!("{:.4}", val));
            }
            out.push('\n');
        }
    }
    let _ = fs::write(voice_file_path(), out);
}

fn load_voice_file() -> Option<(String, Vec<VoiceTemplate>)> {
    let txt = fs::read_to_string(voice_file_path()).ok()?;
    let mut lines = txt.lines();
    if lines.next()?.trim() != "NVVOICE1" {
        return None;
    }
    let mut phrase = String::new();
    let mut templates = Vec::new();
    while let Some(raw) = lines.next() {
        let line = raw.trim();
        if let Some(p) = line.strip_prefix("PHRASE=") {
            phrase = p.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("TPL ") {
            let mut parts = rest.split_whitespace();
            let dur: f32 = parts.next()?.parse().ok()?;
            let nframes: usize = parts.next()?.parse().ok()?;
            let mut frames = Vec::with_capacity(nframes.min(4096));
            for _ in 0..nframes {
                let row = lines.next()?;
                let mut arr = [0.0f32; FEAT_DIM];
                let mut cnt = 0usize;
                for tok in row.split_whitespace() {
                    if cnt < FEAT_DIM {
                        arr[cnt] = tok.parse().unwrap_or(0.0);
                        cnt += 1;
                    }
                }
                if cnt == FEAT_DIM {
                    frames.push(arr);
                }
            }
            if !frames.is_empty() {
                templates.push(VoiceTemplate {
                    phrase: phrase.clone(),
                    duration_secs: dur,
                    frames,
                });
            }
        }
    }
    Some((phrase, templates))
}

fn synth_utterance_a(gain: f32, speed_scale: f32) -> Vec<f32> {
    let base_len = (1.15 * VOICE_SAMPLE_RATE as f32 / speed_scale) as usize;
    let mut out = Vec::with_capacity(base_len);
    for i in 0..base_len {
        let t_norm = i as f32 / base_len as f32;
        // Multi-syllable contour mimicking e.g. "Roman, clip that"
        let (f1, f2, env) = if t_norm < 0.28 {
            let u = t_norm / 0.28;
            (180.0 + u * 240.0, 900.0 + u * 650.0, (u * PI).sin())
        } else if t_norm < 0.36 {
            (120.0, 400.0, 0.08)
        } else if t_norm < 0.62 {
            let u = (t_norm - 0.36) / 0.26;
            (320.0 - u * 90.0, 2100.0 - u * 450.0, (u * PI).sin())
        } else if t_norm < 0.70 {
            (140.0, 400.0, 0.06)
        } else {
            let u = (t_norm - 0.70) / 0.30;
            (150.0 + u * 310.0, 1200.0 + u * 1100.0, (u * PI).sin())
        };
        let t = i as f32 / VOICE_SAMPLE_RATE as f32;
        let sig = ((2.0 * PI * f1 * t).sin() * 0.65 + (2.0 * PI * f2 * t).sin() * 0.35) * env * gain;
        out.push(sig);
    }
    out
}

fn synth_utterance_b(gain: f32) -> Vec<f32> {
    let len = (1.10 * VOICE_SAMPLE_RATE as f32) as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let u = i as f32 / len as f32;
        let f1 = 440.0 - u * 260.0;
        let f2 = 2400.0 - u * 1500.0;
        let t = i as f32 / VOICE_SAMPLE_RATE as f32;
        let env = (u * PI).sin();
        let sig = ((2.0 * PI * f1 * t).sin() * 0.55 + (2.0 * PI * f2 * t).sin() * 0.45) * env * gain;
        out.push(sig);
    }
    out
}

/// Self-test benchmark for Pure-Rust MFCC + CMVN + DTW recognizer accuracy, invariance & speed.
pub fn verify_dsp_pipeline() -> (bool, String) {
    let fe = MfccFrontend::new();
    let tpl_raw = synth_utterance_a(0.45, 1.0);
    let same_fast_quiet = synth_utterance_a(0.18, 1.18); // +18% faster & 2.5x quieter!
    let diff_phrase = synth_utterance_b(0.45);

    let tpl_feats = fe.extract_features(&tpl_raw);
    let fast_feats = fe.extract_features(&same_fast_quiet);
    let diff_feats = fe.extract_features(&diff_phrase);

    let t0 = Instant::now();
    let score_same = dtw_similarity_percent(&fast_feats, &tpl_feats);
    let dtw_us = t0.elapsed().as_micros();
    let score_diff = dtw_similarity_percent(&diff_feats, &tpl_feats);

    // Also test end-to-end streaming VAD + VoiceRecognizer trigger firing & silence immunity
    let vr = VoiceRecognizer::new("Roman, clip that");
    {
        let mut g = vr.inner.lock().unwrap();
        g.templates.clear();
        g.templates.push(VoiceTemplate {
            phrase: "Roman, clip that".to_string(),
            duration_secs: 1.15,
            frames: tpl_feats,
        });
    }
    let silence = vec![0.0008f32; 16_000];
    let sil_trig = vr.process_16k_chunk(&silence, 65, 8, true);

    let mut padded_match = vec![0.0005f32; 4_000];
    padded_match.extend_from_slice(&same_fast_quiet);
    padded_match.extend_from_slice(&vec![0.0005f32; 6_000]);
    let match_trig = vr.process_16k_chunk(&padded_match, 65, 8, true);

    let pass = !sil_trig && match_trig && score_same >= 65.0 && score_diff < 55.0;
    let summary = format!(
        "Voice MFCC+DTW Test: same_fast_quiet={:.1}%, unrelated_phrase={:.1}%, DTW_time={} us, silence_false_trigger={}, vad_match_trigger={}",
        score_same, score_diff, dtw_us, sil_trig, match_trig
    );
    (pass, summary)
}
