//! Preamp CPU probe: engine render with tremolo at full depth (r_ldr moving
//! every sample — the knob-move-heavy scenario behind the v0.5.2 legacy
//! default). Build twice to compare:
//!   cargo test -p openwurli-dsp --release --test preamp_perf_probe -- --ignored --nocapture
//!   cargo test -p openwurli-dsp --release --features melange-preamp --test preamp_perf_probe -- --ignored --nocapture
use openwurli_dsp::WurliEngine;
use std::time::Instant;

#[test]
#[ignore = "perf probe; run explicitly in release"]
fn preamp_tremolo_cpu_probe() {
    let sr = 44_100.0;
    let mut engine = WurliEngine::new(sr);
    engine.set_tremolo_depth(1.0);
    engine.warm_up();
    for (n, v) in [(48u8, 110u8), (55, 100), (60, 105), (64, 95), (67, 100)] {
        engine.note_on(n, v as f32 / 127.0);
    }
    let seconds = 4.0;
    let n = (sr * seconds) as usize;
    let mut buf = [0.0f32; 64];
    let mut peak = 0.0f32;
    let t0 = Instant::now();
    let mut rendered = 0usize;
    while rendered < n {
        let take = (n - rendered).min(64);
        engine.render(&mut buf[..take]);
        for &s in &buf[..take] {
            peak = peak.max(s.abs());
        }
        rendered += take;
    }
    let dt = t0.elapsed().as_secs_f64();
    let feature = if cfg!(feature = "melange-preamp") {
        "MELANGE 12-node"
    } else {
        "LEGACY 8-node"
    };
    println!(
        "preamp={feature}: {dt:.3}s for {seconds}s audio = {:.1}% of realtime (peak {peak:.3})",
        100.0 * dt / seconds
    );
    assert!(peak.is_finite() && peak > 0.0);
}
