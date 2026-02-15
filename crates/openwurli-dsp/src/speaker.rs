/// Wurlitzer 200A speaker cabinet model — variable HPF + LPF.
///
/// The 200A uses two 4"x8" oval ceramic-magnet speakers in an open-backed
/// ABS plastic lid. This produces:
///   - Bass rolloff: open-baffle cancellation + speaker resonance (~85-100 Hz)
///   - Treble rolloff: cone breakup (~7-8 kHz)
///
/// "Speaker Character" parameter blends from bypass (flat) to authentic
/// (full HPF + LPF). Intermediate positions interpolate cutoff frequencies.

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

pub struct Speaker {
    hpf: Biquad,
    lpf: Biquad,
    character: f64,
    sample_rate: f64,
}

impl Speaker {
    pub fn new(sample_rate: f64) -> Self {
        let mut s = Self {
            hpf: Biquad::highpass(HPF_AUTHENTIC_HZ, HPF_Q, sample_rate),
            lpf: Biquad::lowpass(LPF_AUTHENTIC_HZ, LPF_Q, sample_rate),
            character: 1.0,
            sample_rate,
        };
        s.update_coefficients();
        s
    }

    /// Set speaker character: 0.0 = bypass (flat), 1.0 = authentic (HPF+LPF).
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
    }

    pub fn process(&mut self, input: f64) -> f64 {
        let x = self.hpf.process(input);
        self.lpf.process(x)
    }

    pub fn reset(&mut self) {
        self.hpf.reset();
        self.lpf.reset();
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
        assert!(atten_db < -6.0, "50Hz should be attenuated: {atten_db:.1} dB");
    }

    #[test]
    fn test_authentic_treble_rolloff() {
        let sr = 44100.0;
        let mut speaker = Speaker::new(sr);
        speaker.set_character(1.0);

        let mid = measure_response(&mut speaker, 1000.0, sr);
        let treble = measure_response(&mut speaker, 15000.0, sr);

        let atten_db = 20.0 * (treble / mid).log10();
        assert!(atten_db < -6.0, "15kHz should be attenuated: {atten_db:.1} dB");
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
        assert!(ratio_low < 1.0, "Bypass should be flat at 100Hz: {ratio_low:.1} dB");
        assert!(ratio_high < 1.0, "Bypass should be flat at 10kHz: {ratio_high:.1} dB");
    }
}
