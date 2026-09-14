//! Tremolo depth ladder: AM depth in dB, measured through the real preamp.
use openwurli_dsp::dk_preamp_legacy::DkPreamp;
use openwurli_dsp::preamp::PreampModel;
use openwurli_dsp::tremolo::Tremolo;

#[test]
#[ignore = "probe"]
fn depth_ladder() {
    let sr = 88_200.0;
    let f = 1000.0;
    println!("depth   AM depth (dB)   Z_min      Z_max");
    for depth in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let mut trem = Tremolo::new(depth, sr);
        let mut pre = DkPreamp::new(sr);
        // warm both
        for _ in 0..(sr * 1.5) as usize {
            let z = trem.process();
            pre.set_ldr_resistance(z);
            pre.process_sample(0.0);
        }
        let (mut zmin, mut zmax) = (f64::MAX, f64::MIN);
        // envelope follower on the preamp output
        let mut env = 0.0f64;
        let (mut emin, mut emax) = (f64::MAX, f64::MIN);
        let n = (sr * 0.6) as usize; // ~3.4 tremolo cycles at 5.7 Hz
        for i in 0..n {
            let z = trem.process();
            zmin = zmin.min(z);
            zmax = zmax.max(z);
            pre.set_ldr_resistance(z);
            let t = i as f64 / sr;
            let y = pre.process_sample(0.001 * (2.0 * std::f64::consts::PI * f * t).sin());
            let a = y.abs();
            // fast attack / slow release peak follower, well above the tremolo rate
            env = if a > env { a } else { env + (a - env) * 0.0008 };
            if i > n / 4 {
                emin = emin.min(env);
                emax = emax.max(env);
            }
        }
        let am_db = 20.0 * (emax / emin).log10();
        println!("{depth:5.2}   {am_db:9.2}      {:.0}   {:.0}", zmin, zmax);
    }
}
