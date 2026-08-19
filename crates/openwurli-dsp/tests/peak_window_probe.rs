// Peak-invariant probes (2026-08-19, prompted by the fleet window-length
// caution, robogogo thread 199).
//
// FINDINGS:
// 1. Window length is FINE: true peak arrives at t=0.039 s (attack
//    transient); 8 s finds nothing later. Wurli peaks are early.
// 2. The invariant is PHASE-FRAGILE: sweeping chord onset across one
//    tremolo cycle (warmed engine, as production always is) gives peaks
//    0.997..1.144 at vol=1.0. The in-tree ≤1.02 test passes only because
//    it omits warm_up() — a cold-engine state no host produces — and lands
//    on a lucky phase. Production worst-case exceeds the documented
//    invariant by +1.16 dB. PSG decision pending (re-trim vs document).
use openwurli_dsp::WurliEngine;

#[test]
#[ignore = "diagnostic probe"]
fn peak_arrival_time_probe() {
    let sr = 44_100.0;
    let mut e = WurliEngine::new(sr);
    e.set_volume(1.0);
    e.set_tremolo_depth(1.0);
    e.set_speaker_character(0.0);
    e.set_mlp_enabled(true);
    e.set_noise_enabled(false);
    e.warm_up();
    let mut warmup = vec![0.0f32; 1024];
    for _ in 0..6 {
        e.render(&mut warmup);
    }
    for &n in &[48u8, 55, 60, 63, 67, 70] {
        e.note_on(n, 0.95);
    }
    let total = (sr * 8.0) as usize;
    let mut buf = vec![0.0f32; 1024];
    let (mut peak, mut peak_at, mut pos) = (0.0f32, 0usize, 0usize);
    let mut peak_1s = 0.0f32;
    while pos < total {
        let len = 1024.min(total - pos);
        e.render(&mut buf[..len]);
        for (i, &s) in buf[..len].iter().enumerate() {
            let a = s.abs();
            if pos + i < (sr as usize) {
                peak_1s = peak_1s.max(a);
            }
            if a > peak {
                peak = a;
                peak_at = pos + i;
            }
        }
        pos += len;
    }
    println!(
        "peak within 1.0s: {peak_1s:.4} | true peak over 8s: {peak:.4} at t={:.3}s | ratio {:+.3} dB",
        peak_at as f64 / sr,
        20.0 * (peak / peak_1s).log10()
    );
}

#[test]
#[ignore = "diagnostic probe"]
fn peak_vs_tremolo_phase_probe() {
    let sr = 44_100.0;
    // 5.63 Hz tremolo period ~178 ms; sweep chord onset across ~1 cycle.
    for delay_ms in [0u32, 22, 44, 66, 89, 111, 133, 155, 178] {
        let mut e = WurliEngine::new(sr);
        e.set_volume(1.0);
        e.set_tremolo_depth(1.0);
        e.set_speaker_character(0.0);
        e.set_mlp_enabled(true);
        e.set_noise_enabled(false);
        e.warm_up();
        let mut warmup = vec![0.0f32; 1024];
        for _ in 0..6 {
            e.render(&mut warmup);
        }
        let delay = (sr * delay_ms as f64 / 1000.0) as usize;
        let mut buf = vec![0.0f32; 1024];
        let mut pos = 0usize;
        while pos < delay {
            let len = 1024.min(delay - pos);
            e.render(&mut buf[..len]);
            pos += len;
        }
        for &n in &[48u8, 55, 60, 63, 67, 70] {
            e.note_on(n, 0.95);
        }
        let total = (sr * 2.0) as usize;
        let (mut peak, mut pos2) = (0.0f32, 0usize);
        while pos2 < total {
            let len = 1024.min(total - pos2);
            e.render(&mut buf[..len]);
            for &s in &buf[..len] {
                peak = peak.max(s.abs());
            }
            pos2 += len;
        }
        println!("onset delay {delay_ms:>3} ms -> peak {peak:.4}");
    }
}
