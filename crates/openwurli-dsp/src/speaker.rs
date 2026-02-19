/// Wurlitzer 200A speaker cabinet model — Hammerstein nonlinearity + HPF/LPF.
///
/// The 200A uses two 4"x8" oval ceramic-magnet speakers in an open-backed
/// ABS plastic lid. This produces:
///   - Bass rolloff: open-baffle cancellation + speaker resonance (~85-100 Hz)
///   - Treble rolloff: cone breakup (~7-8 kHz)
///   - Cone nonlinearity: Kms hardening (odd harmonics) + BL asymmetry
///     (even harmonics). Modeled as a direct memoryless polynomial — harmonics
///     are phase-coherent with the input signal by construction.
///   - Thermal voice coil compression: slow power-dependent gain reduction (~5s τ)
///
/// Architecture: static polynomial waveshaper → linear filters (HPF + LPF).
/// This is a textbook Hammerstein model. The polynomial is memoryless (no
/// internal filters), so generated harmonics maintain natural phase relationships
/// with the fundamental and with any existing harmonics from upstream stages.
///
/// "Speaker Character" parameter blends from bypass (flat, linear) to authentic
/// (full nonlinearity + HPF + LPF). At character=0.0 all nonlinearity
/// coefficients are zero — pure linear passthrough.
use crate::filters::Biquad;

/// HPF cutoff at fully authentic position.
const HPF_AUTHENTIC_HZ: f64 = 95.0;
/// HPF Q (slightly underdamped for speaker resonance bump).
const HPF_Q: f64 = 0.75;
/// LPF cutoff at fully authentic position.
const LPF_AUTHENTIC_HZ: f64 = 7500.0;
/// LPF Q (Butterworth).
const LPF_Q: f64 = 0.707;
/// HPF cutoff at bypass position (effectively transparent).
const HPF_BYPASS_HZ: f64 = 20.0;
/// LPF cutoff at bypass position (effectively transparent).
const LPF_BYPASS_HZ: f64 = 20000.0;

/// Thermal compression time constant (seconds).
const THERMAL_TAU: f64 = 5.0;

pub struct Speaker {
    hpf: Biquad,
    lpf: Biquad,
    character: f64,
    sample_rate: f64,
    // Polynomial coefficients (scaled by character)
    a2: f64, // x² — BL asymmetry → even harmonics (H2, H4)
    a3: f64, // x³ — Kms hardening → odd harmonics (H3, H5)
    thermal_coeff: f64,
    thermal_alpha: f64,
    thermal_state: f64,
}

impl Speaker {
    pub fn new(sample_rate: f64) -> Self {
        let mut s = Self {
            hpf: Biquad::highpass(HPF_AUTHENTIC_HZ, HPF_Q, sample_rate),
            lpf: Biquad::lowpass(LPF_AUTHENTIC_HZ, LPF_Q, sample_rate),
            character: 1.0,
            sample_rate,
            a2: 0.0,
            a3: 0.0,
            thermal_coeff: 0.0,
            thermal_alpha: 1.0 / (THERMAL_TAU * sample_rate),
            thermal_state: 0.0,
        };
        s.update_coefficients();
        s
    }

    /// Set speaker character: 0.0 = bypass (flat, linear), 1.0 = authentic.
    pub fn set_character(&mut self, character: f64) {
        let c = character.clamp(0.0, 1.0);
        if (c - self.character).abs() > 1e-6 {
            self.character = c;
            self.update_coefficients();
        }
    }

    fn update_coefficients(&mut self) {
        let c = self.character;
        // Logarithmic interpolation of cutoff frequencies
        let hpf_hz = HPF_BYPASS_HZ * (HPF_AUTHENTIC_HZ / HPF_BYPASS_HZ).powf(c);
        let lpf_hz = LPF_BYPASS_HZ * (LPF_AUTHENTIC_HZ / LPF_BYPASS_HZ).powf(c);
        self.hpf.set_highpass(hpf_hz, HPF_Q, self.sample_rate);
        self.lpf.set_lowpass(lpf_hz, LPF_Q, self.sample_rate);

        // Polynomial coefficients — all zero at character=0
        self.a2 = 0.2 * c; // BL asymmetry (even harmonics)
        self.a3 = 0.6 * c; // Kms hardening (odd harmonics)
        self.thermal_coeff = 2.0 * c;
    }

    pub fn process(&mut self, input: f64) -> f64 {
        // 1. Static polynomial waveshaper (memoryless — phase-coherent harmonics)
        //    y = (x + a2·x² + a3·x³) / (1 + a2 + a3)
        //    Normalization ensures y(±1) = ±1 (unity gain at full scale),
        //    preserving harmonic generation ratios without boosting peak levels.
        //    x² → H2, H4 (BL asymmetry, even harmonics)
        //    x³ → H3, H5 (Kms hardening, odd harmonics) + fundamental compression
        let x2 = input * input;
        let x3 = x2 * input;
        let shaped = (input + self.a2 * x2 + self.a3 * x3) / (1.0 + self.a2 + self.a3);

        // 2. Cone excursion limit (Xmax soft stop)
        //    Real speaker cones have physical excursion limits where the spider
        //    and surround stiffen rapidly. tanh models the soft mechanical stop.
        //    At normal levels (|shaped| < 0.5): <8% compression (inaudible).
        //    At ff chords (|shaped| > 1.0): graceful saturation to ±1.0.
        let limited = shaped.tanh();

        // 3. Thermal voice coil compression (slow envelope follower)
        let power = x2;
        self.thermal_state += (power - self.thermal_state) * self.thermal_alpha;
        let thermal_gain = 1.0 / (1.0 + self.thermal_coeff * self.thermal_state.sqrt());

        // 4. Linear filters (HPF + LPF)
        let filtered = self.hpf.process(limited * thermal_gain);
        self.lpf.process(filtered)
    }

    pub fn reset(&mut self) {
        self.hpf.reset();
        self.lpf.reset();
        self.thermal_state = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn measure_response(speaker: &mut Speaker, freq: f64, sr: f64) -> f64 {
        speaker.reset();
        let n = (sr * 0.2) as usize;
        let mut peak = 0.0f64;
        for i in 0..n {
            let x = (2.0 * PI * freq * i as f64 / sr).sin();
            let y = speaker.process(x);
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        peak
    }

    #[test]
    fn test_authentic_bass_rolloff() {
        let sr = 44100.0;
        let mut speaker = Speaker::new(sr);
        speaker.set_character(1.0);

        let mid = measure_response(&mut speaker, 500.0, sr);
        let bass = measure_response(&mut speaker, 50.0, sr);

        let atten_db = 20.0 * (bass / mid).log10();
        assert!(
            atten_db < -6.0,
            "50Hz should be attenuated: {atten_db:.1} dB"
        );
    }

    #[test]
    fn test_authentic_treble_rolloff() {
        let sr = 44100.0;
        let mut speaker = Speaker::new(sr);
        speaker.set_character(1.0);

        let mid = measure_response(&mut speaker, 1000.0, sr);
        let treble = measure_response(&mut speaker, 15000.0, sr);

        let atten_db = 20.0 * (treble / mid).log10();
        assert!(
            atten_db < -6.0,
            "15kHz should be attenuated: {atten_db:.1} dB"
        );
    }

    #[test]
    fn test_bypass_is_flat() {
        let sr = 44100.0;
        let mut speaker = Speaker::new(sr);
        speaker.set_character(0.0);

        let low = measure_response(&mut speaker, 100.0, sr);
        let mid = measure_response(&mut speaker, 1000.0, sr);
        let high = measure_response(&mut speaker, 10000.0, sr);

        // All should be within 1 dB of each other
        let ratio_low = (20.0 * (low / mid).log10()).abs();
        let ratio_high = (20.0 * (high / mid).log10()).abs();
        assert!(
            ratio_low < 1.0,
            "Bypass should be flat at 100Hz: {ratio_low:.1} dB"
        );
        assert!(
            ratio_high < 1.0,
            "Bypass should be flat at 10kHz: {ratio_high:.1} dB"
        );
    }

    #[test]
    fn test_nonlinearity_generates_harmonics() {
        // Feed a sine through authentic speaker, verify harmonic content
        let sr = 44100.0;
        let mut speaker = Speaker::new(sr);
        speaker.set_character(1.0);
        speaker.reset();

        let freq = 200.0;
        let n = (sr * 0.5) as usize;
        let mut samples = Vec::with_capacity(n);
        for i in 0..n {
            let x = 0.8 * (2.0 * PI * freq * i as f64 / sr).sin();
            samples.push(speaker.process(x));
        }

        let analysis_start = n / 2;
        let fundamental_mag = dft_magnitude(&samples[analysis_start..], freq, sr);
        let h2_mag = dft_magnitude(&samples[analysis_start..], 2.0 * freq, sr);
        let h3_mag = dft_magnitude(&samples[analysis_start..], 3.0 * freq, sr);

        let thd = (h2_mag * h2_mag + h3_mag * h3_mag).sqrt() / fundamental_mag;
        assert!(
            thd > 0.005,
            "Speaker should generate measurable THD: {thd:.4}"
        );
        // Both even and odd harmonics should be present
        assert!(
            h2_mag > 0.0001,
            "Should have H2 (BL asymmetry): {h2_mag:.6}"
        );
        assert!(
            h3_mag > 0.0001,
            "Should have H3 (Kms hardening): {h3_mag:.6}"
        );
    }

    #[test]
    fn test_nonlinearity_amplitude_dependent() {
        // Louder signals should produce more THD (polynomial grows with amplitude)
        let sr = 44100.0;

        let thd_loud = measure_thd(200.0, 0.8, sr);
        let thd_quiet = measure_thd(200.0, 0.2, sr);

        assert!(
            thd_loud > thd_quiet * 1.2,
            "Loud THD ({thd_loud:.4}) should exceed quiet THD ({thd_quiet:.4}) (tanh Xmax limits growth)"
        );
    }

    #[test]
    fn test_thermal_compression() {
        // Sustained loud signal should compress vs. the initial level
        let sr = 44100.0;
        let mut speaker = Speaker::new(sr);
        speaker.set_character(1.0);

        let freq = 300.0;
        let n = (sr * 8.0) as usize;

        let settle = (sr * 0.2) as usize;
        let early_end = (sr * 0.5) as usize;
        let mut early_peak = 0.0f64;
        let mut late_peak = 0.0f64;

        for i in 0..n {
            let x = 0.9 * (2.0 * PI * freq * i as f64 / sr).sin();
            let y = speaker.process(x);
            if i > settle && i < early_end {
                early_peak = early_peak.max(y.abs());
            }
            if i > n - (sr * 0.5) as usize {
                late_peak = late_peak.max(y.abs());
            }
        }

        let compression_db = 20.0 * (late_peak / early_peak).log10();
        assert!(
            compression_db < -0.3,
            "Thermal compression should reduce level: {compression_db:.2} dB"
        );
    }

    fn measure_thd(freq: f64, amplitude: f64, sr: f64) -> f64 {
        let mut speaker = Speaker::new(sr);
        speaker.set_character(1.0);
        speaker.reset();

        let n = (sr * 0.5) as usize;
        let mut samples = Vec::with_capacity(n);
        for i in 0..n {
            let x = amplitude * (2.0 * PI * freq * i as f64 / sr).sin();
            samples.push(speaker.process(x));
        }

        let start = n / 2;
        let f_mag = dft_magnitude(&samples[start..], freq, sr);
        let h2 = dft_magnitude(&samples[start..], 2.0 * freq, sr);
        let h3 = dft_magnitude(&samples[start..], 3.0 * freq, sr);
        (h2 * h2 + h3 * h3).sqrt() / f_mag
    }

    /// Single-bin DFT magnitude (Goertzel-like).
    fn dft_magnitude(samples: &[f64], freq: f64, sr: f64) -> f64 {
        let n = samples.len() as f64;
        let mut re = 0.0;
        let mut im = 0.0;
        for (i, &s) in samples.iter().enumerate() {
            let phase = 2.0 * PI * freq * i as f64 / sr;
            re += s * phase.cos();
            im += s * phase.sin();
        }
        (re * re + im * im).sqrt() / n
    }
}
