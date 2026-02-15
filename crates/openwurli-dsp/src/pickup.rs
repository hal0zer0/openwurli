/// Electrostatic pickup model — linear sensitivity + RC HPF.
///
/// The Wurlitzer 200A pickup is a capacitive sensor: reed vibration modulates
/// the capacitance between the reed and a charged metal plate. The output
/// voltage is proportional to displacement times a sensitivity constant.
///
/// One high-pass filter shapes the frequency response:
///   Pickup RC: 1-pole HPF at 2312 Hz (R_total=287K, C=240pF)
///   R_total = R_feed (1M) || (R-1 + R-2||R-3) = 1M || 402K = 287K
///
/// C20 (220 pF shunt cap at preamp input) is an RF protection cap, not an
/// audio filter — its cutoff relative to R-1 (22K) is ~33 kHz. Not modeled.

use crate::filters::OnePoleHpf;

/// Pickup sensitivity: V_bias * C_0 / C_total = 147 * 3/240 = 1.8375 V/unit
const SENSITIVITY: f64 = 1.8375;

pub struct Pickup {
    hpf: OnePoleHpf,
}

impl Pickup {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            hpf: OnePoleHpf::new(2312.0, sample_rate),
        }
    }

    /// Process a buffer of reed displacement samples in-place.
    /// Input: arbitrary displacement units. Output: millivolts.
    pub fn process(&mut self, buffer: &mut [f64]) {
        for sample in buffer.iter_mut() {
            let v = *sample * SENSITIVITY;
            let v = self.hpf.process(v);
            *sample = v;
        }
    }

    pub fn reset(&mut self) {
        self.hpf.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_hpf_passes_high_freq() {
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        let freq = 10000.0;

        let n = (sr * 0.05) as usize;
        let mut buf: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);

        let peak = buf[n / 2..].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(peak > 1.2, "pickup output too low at 10kHz: {peak}");
        assert!(peak < 2.2, "pickup output too high at 10kHz: {peak}");
    }

    #[test]
    fn test_hpf_attenuates_bass() {
        let sr = 44100.0;
        let mut pickup = Pickup::new(sr);
        let freq = 100.0;

        let n = (sr * 0.1) as usize;
        let mut buf: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        pickup.process(&mut buf);

        let peak = buf[n / 2..].iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        assert!(peak < 0.15, "pickup should heavily attenuate 100Hz: {peak}");
    }
}
