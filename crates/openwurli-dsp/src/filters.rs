/// Shared filter primitives for the Wurlitzer 200A signal chain.
///
/// All filters: `new(freq, sample_rate)`, `process(sample) -> sample`, `reset()`.

use std::f64::consts::PI;

/// 1-pole high-pass filter: y[n] = alpha * (y[n-1] + x[n] - x[n-1])
pub struct OnePoleHpf {
    alpha: f64,
    prev_x: f64,
    prev_y: f64,
}

impl OnePoleHpf {
    pub fn new(cutoff_hz: f64, sample_rate: f64) -> Self {
        let rc = 1.0 / (2.0 * PI * cutoff_hz);
        let dt = 1.0 / sample_rate;
        let alpha = rc / (rc + dt);
        Self {
            alpha,
            prev_x: 0.0,
            prev_y: 0.0,
        }
    }

    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.alpha * (self.prev_y + x - self.prev_x);
        self.prev_x = x;
        self.prev_y = y;
        y
    }

    pub fn reset(&mut self) {
        self.prev_x = 0.0;
        self.prev_y = 0.0;
    }
}

/// 1-pole low-pass filter: y[n] = alpha * x[n] + (1 - alpha) * y[n-1]
pub struct OnePoleLpf {
    alpha: f64,
    prev_y: f64,
}

impl OnePoleLpf {
    pub fn new(cutoff_hz: f64, sample_rate: f64) -> Self {
        let rc = 1.0 / (2.0 * PI * cutoff_hz);
        let dt = 1.0 / sample_rate;
        let alpha = dt / (rc + dt);
        Self { alpha, prev_y: 0.0 }
    }

    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.alpha * x + (1.0 - self.alpha) * self.prev_y;
        self.prev_y = y;
        y
    }

    pub fn reset(&mut self) {
        self.prev_y = 0.0;
    }
}

/// DC blocker — 1-pole HPF at very low frequency (default 20 Hz).
pub struct DcBlocker {
    hpf: OnePoleHpf,
}

impl DcBlocker {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            hpf: OnePoleHpf::new(20.0, sample_rate),
        }
    }

    pub fn process(&mut self, x: f64) -> f64 {
        self.hpf.process(x)
    }

    pub fn reset(&mut self) {
        self.hpf.reset();
    }
}

/// Biquad filter — Direct Form II Transposed.
///
/// General-purpose second-order IIR filter. Coefficients set via
/// constructor methods for specific filter types.
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    s1: f64,
    s2: f64,
}

impl Biquad {
    /// Bandpass filter (constant skirt gain, Audio EQ Cookbook).
    pub fn bandpass(center_hz: f64, q: f64, sample_rate: f64) -> Self {
        let w0 = 2.0 * PI * center_hz / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();

        let b0 = alpha;
        let b1 = 0.0;
        let b2 = -alpha;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            s1: 0.0,
            s2: 0.0,
        }
    }

    /// Low-pass filter (Audio EQ Cookbook).
    pub fn lowpass(cutoff_hz: f64, q: f64, sample_rate: f64) -> Self {
        let w0 = 2.0 * PI * cutoff_hz / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();

        let b1 = 1.0 - cos_w0;
        let b0 = b1 / 2.0;
        let b2 = b0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            s1: 0.0,
            s2: 0.0,
        }
    }

    /// High-pass filter (Audio EQ Cookbook).
    pub fn highpass(cutoff_hz: f64, q: f64, sample_rate: f64) -> Self {
        let w0 = 2.0 * PI * cutoff_hz / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();

        let b1 = -(1.0 + cos_w0);
        let b0 = -b1 / 2.0;
        let b2 = b0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            s1: 0.0,
            s2: 0.0,
        }
    }

    /// Update coefficients to highpass without resetting filter state.
    pub fn set_highpass(&mut self, cutoff_hz: f64, q: f64, sample_rate: f64) {
        let new = Self::highpass(cutoff_hz, q, sample_rate);
        self.b0 = new.b0;
        self.b1 = new.b1;
        self.b2 = new.b2;
        self.a1 = new.a1;
        self.a2 = new.a2;
    }

    /// Update coefficients to lowpass without resetting filter state.
    pub fn set_lowpass(&mut self, cutoff_hz: f64, q: f64, sample_rate: f64) {
        let new = Self::lowpass(cutoff_hz, q, sample_rate);
        self.b0 = new.b0;
        self.b1 = new.b1;
        self.b2 = new.b2;
        self.a1 = new.a1;
        self.a2 = new.a2;
    }

    /// Process one sample (Direct Form II Transposed).
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.s1;
        self.s1 = self.b1 * x - self.a1 * y + self.s2;
        self.s2 = self.b2 * x - self.a2 * y;
        y
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hpf_passes_high_freq() {
        let sr = 44100.0;
        let mut hpf = OnePoleHpf::new(1000.0, sr);
        let freq = 5000.0;

        let n = (sr * 0.1) as usize;
        let mut peak = 0.0f64;
        for i in 0..n {
            let x = (2.0 * PI * freq * i as f64 / sr).sin();
            let y = hpf.process(x);
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        assert!(peak > 0.9, "HPF attenuated 5kHz too much: {peak}");
    }

    #[test]
    fn test_hpf_attenuates_low_freq() {
        let sr = 44100.0;
        let mut hpf = OnePoleHpf::new(2000.0, sr);
        let freq = 200.0;

        let n = (sr * 0.1) as usize;
        let mut peak = 0.0f64;
        for i in 0..n {
            let x = (2.0 * PI * freq * i as f64 / sr).sin();
            let y = hpf.process(x);
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        assert!(peak < 0.15, "HPF didn't attenuate 200Hz enough: {peak}");
    }

    #[test]
    fn test_lpf_passes_low_freq() {
        let sr = 44100.0;
        let mut lpf = OnePoleLpf::new(5000.0, sr);
        let freq = 200.0;

        let n = (sr * 0.1) as usize;
        let mut peak = 0.0f64;
        for i in 0..n {
            let x = (2.0 * PI * freq * i as f64 / sr).sin();
            let y = lpf.process(x);
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        assert!(peak > 0.9, "LPF attenuated 200Hz too much: {peak}");
    }

    #[test]
    fn test_lpf_attenuates_high_freq() {
        let sr = 44100.0;
        let mut lpf = OnePoleLpf::new(500.0, sr);
        let freq = 10000.0;

        let n = (sr * 0.1) as usize;
        let mut peak = 0.0f64;
        for i in 0..n {
            let x = (2.0 * PI * freq * i as f64 / sr).sin();
            let y = lpf.process(x);
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        assert!(peak < 0.1, "LPF didn't attenuate 10kHz enough: {peak}");
    }

    #[test]
    fn test_dc_blocker_removes_dc() {
        let sr = 44100.0;
        let mut dc = DcBlocker::new(sr);

        // Feed DC offset for a while
        let n = (sr * 0.5) as usize;
        let mut last = 0.0;
        for _ in 0..n {
            last = dc.process(1.0);
        }
        // After settling, DC should be nearly zero
        assert!(last.abs() < 0.01, "DC blocker didn't remove DC: {last}");
    }

    #[test]
    fn test_biquad_bandpass() {
        let sr = 44100.0;
        let center = 1000.0;
        let mut bpf = Biquad::bandpass(center, 1.0, sr);

        // Feed 1000 Hz — should pass
        let n = (sr * 0.1) as usize;
        let mut peak_center = 0.0f64;
        for i in 0..n {
            let x = (2.0 * PI * center * i as f64 / sr).sin();
            let y = bpf.process(x);
            if i > n / 2 {
                peak_center = peak_center.max(y.abs());
            }
        }

        bpf.reset();

        // Feed 100 Hz — should attenuate
        let mut peak_low = 0.0f64;
        for i in 0..n {
            let x = (2.0 * PI * 100.0 * i as f64 / sr).sin();
            let y = bpf.process(x);
            if i > n / 2 {
                peak_low = peak_low.max(y.abs());
            }
        }

        assert!(peak_center > peak_low * 3.0,
            "BPF center ({peak_center}) should be much louder than off-center ({peak_low})");
    }
}
