use openwurli_dsp::mlp_correction::MlpCorrections;
#[test]
#[ignore = "probe"]
fn untrained_heads() {
    println!("note vel   freq_H2..H6 (cents)                decay_H2..H6                  ds");
    for (m, v) in [
        (48u8, 0.63f64),
        (60, 0.63),
        (60, 0.88),
        (72, 0.63),
        (84, 0.88),
    ] {
        let c = MlpCorrections::infer(m, v);
        print!("{m:4} {v:.2}  ");
        for f in c.freq_offsets_cents.iter() {
            print!("{f:7.2} ");
        }
        print!("  ");
        for d in c.decay_offsets.iter() {
            print!("{d:6.3} ");
        }
        println!("  {:.4}", c.ds_correction);
    }
}
