/// Electrostatic pickup model — linear sensitivity + cascaded HPFs.
///
/// The Wurlitzer 200A pickup is a capacitive sensor: reed vibration modulates
/// the capacitance between the reed and a charged metal plate. The output
/// voltage is proportional to displacement times a sensitivity constant.
///
/// Two high-pass filters shape the frequency response:
/// 1. Pickup RC: 1-pole HPF at 2312 Hz (R_total=287K, C=240pF)
/// 2. C20 input filter: 1-pole HPF at 1903 Hz (R=380K, C20=220pF)
///
/// These cascaded HPFs attenuate bass content and shape the tonal character.

/// Pickup sensitivity: V_bias * C_0 / C_total = 147 * 3/240 = 1.8375 V/unit
const SENSITIVITY: f64 = 1.8375;

pub struct Pickup {
    hpf1: OnePoleHpf,
    hpf2: OnePoleHpf,
}

impl Pickup {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            hpf1: OnePoleHpf::new(2312.0, sample_rate),
            hpf2: OnePoleHpf::new(1903.0, sample_rate),
        }
    }

    /// Process a buffer of reed displacement samples in-place.
    /// Input: arbitrary displacement units. Output: millivolts.
    pub fn process(&mut self, buffer: &mut [f64]) {
        for sample in buffer.iter_mut() {
            let v = *sample * SENSITIVITY;
            let v = self.hpf1.process(v);
            let v = self.hpf2.process(v);
            *sample = v;
        }
    }
}

/// 1-pole high-pass filter: y[n] = alpha * (y[n-1] + x[n] - x[n-1])
struct OnePoleHpf {
    alpha: f64,
    prev_x: f64,
    prev_y: f64,
}

impl OnePoleHpf {
    fn new(cutoff_hz: f64, sample_rate: f64) -> Self {
        let rc = 1.0 / (2.0 * std::f64::consts::PI * cutoff_hz);
        let dt = 1.0 / sample_rate;
        let alpha = rc / (rc + dt);
        Self {
            alpha,
            prev_x: 0.0,
            prev_y: 0.0,
        }
    }

    fn process(&mut self, x: f64) -> f64 {
        let y = self.alpha * (self.prev_y + x - self.prev_x);
        self.prev_x = x;
        self.prev_y = y;
        y
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
        let two_pi = 2.0 * std::f64::consts::PI;

        // Run for 0.1s to reach steady state
        let n = (sr * 0.1) as usize;
        let mut peak = 0.0f64;
        for i in 0..n {
            let x = (two_pi * freq * i as f64 / sr).sin();
            let y = hpf.process(x);
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        // 5 kHz through 1 kHz HPF should pass with minimal attenuation
        assert!(peak > 0.9, "HPF attenuated 5kHz too much: {peak}");
    }

    #[test]
    fn test_hpf_attenuates_low_freq() {
        let sr = 44100.0;
        let mut hpf = OnePoleHpf::new(2000.0, sr);
        let freq = 200.0;
        let two_pi = 2.0 * std::f64::consts::PI;

        let n = (sr * 0.1) as usize;
        let mut peak = 0.0f64;
        for i in 0..n {
            let x = (two_pi * freq * i as f64 / sr).sin();
            let y = hpf.process(x);
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        // 200 Hz through 2 kHz HPF should be heavily attenuated (~-20 dB)
        assert!(peak < 0.15, "HPF didn't attenuate 200Hz enough: {peak}");
    }

    #[test]
    fn test_pickup_scales_by_sensitivity() {
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        // High-frequency signal should pass through with ~SENSITIVITY gain
        let freq = 10000.0;
        let two_pi = 2.0 * std::f64::consts::PI;

        let n = (sr * 0.05) as usize;
        let mut buf: Vec<f64> = (0..n)
            .map(|i| (two_pi * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);

        let peak = buf[n / 2..].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        // At 10 kHz, both HPFs pass most signal. Digital warping reduces gain slightly.
        // Expected: SENSITIVITY (~1.84) * analog_gain (~0.96) * digital_warping (~0.8)
        assert!(peak > 1.2, "pickup output too low at 10kHz: {peak}");
        assert!(peak < 2.2, "pickup output too high at 10kHz: {peak}");
    }
}
