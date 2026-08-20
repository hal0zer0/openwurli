// Diagnostic: what the MLP actually applies across the keyboard.
use openwurli_dsp::mlp_correction::MlpCorrections;

#[test]
#[ignore = "diagnostic dump"]
fn dump_mlp_corrections() {
    for vel in [0.63, 0.88] {
        println!("--- velocity {vel} ---");
        for midi in (36..=96).step_by(3) {
            let c = MlpCorrections::infer(midi, vel);
            let fmax = c
                .freq_offsets_cents
                .iter()
                .fold(0.0f64, |a, &v| a.max(v.abs()));
            let dmin = c.decay_offsets.iter().cloned().fold(f64::MAX, f64::min);
            let dmax = c.decay_offsets.iter().cloned().fold(f64::MIN, f64::max);
            println!(
                "midi {midi:>3}: ds {:.4}  max|freq| {fmax:>6.2}c  decay [{dmin:.3}..{dmax:.3}]",
                c.ds_correction
            );
        }
    }
}
