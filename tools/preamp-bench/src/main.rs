//! Preamp Bench -- Wurlitzer 200A preamp DSP validation CLI.
//!
//! Measures preamp characteristics and compares against SPICE targets.
//!
//! Usage:
//!   preamp-bench gain [--freq F] [--amplitude A]
//!   preamp-bench sweep [--start F1] [--end F2] [--points N] [--csv FILE]
//!   preamp-bench harmonics [--freq F] [--amplitude A]
//!   preamp-bench tremolo-sweep [--ldr-min R1] [--ldr-max R2] [--steps N] [--csv FILE]
//!   preamp-bench render [--note N] [--velocity V] [--duration D] [--output FILE]

use std::f64::consts::PI;

use openwurli_dsp::dk_preamp::DkPreamp;
use openwurli_dsp::filters::OnePoleHpf;
use openwurli_dsp::hammer::dwell_attenuation;
use openwurli_dsp::oversampler::Oversampler;
use openwurli_dsp::power_amp::PowerAmp;
use openwurli_dsp::preamp::{EbersMollPreamp, PreampModel};
use openwurli_dsp::reed::ModalReed;
use openwurli_dsp::speaker::Speaker;
use openwurli_dsp::tables::{self, CalibrationConfig, NUM_MODES};
use openwurli_dsp::tremolo::Tremolo;
use openwurli_dsp::variation;
use openwurli_dsp::voice::Voice;

use std::io::Write;

const BASE_SR: f64 = 44100.0;
const OVERSAMPLED_SR: f64 = BASE_SR * 2.0;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "gain" => cmd_gain(&args[2..]),
        "sweep" => cmd_sweep(&args[2..]),
        "harmonics" => cmd_harmonics(&args[2..]),
        "tremolo-sweep" => cmd_tremolo_sweep(&args[2..]),
        "render" => cmd_render(&args[2..]),
        "bark-audit" => cmd_bark_audit(&args[2..]),
        "intermod-audit" => cmd_intermod_audit(&args[2..]),
        "calibrate" => cmd_calibrate(&args[2..]),
        "sensitivity" => cmd_sensitivity(&args[2..]),
        "centroid-track" => cmd_centroid_track(&args[2..]),
        "render-poly" => cmd_render_poly(&args[2..]),
        "render-midi" => cmd_render_midi(&args[2..]),
        _ => {
            eprintln!("Unknown subcommand: {}", args[1]);
            print_usage();
        }
    }
}

fn print_usage() {
    eprintln!("Preamp Bench — Wurlitzer 200A preamp DSP validation");
    eprintln!();
    eprintln!("Subcommands:");
    eprintln!("  gain            Measure gain at a single frequency");
    eprintln!("  sweep           Frequency response sweep (log scale)");
    eprintln!("  harmonics       Measure harmonic distortion (H2/H3)");
    eprintln!("  tremolo-sweep   Gain vs LDR resistance sweep");
    eprintln!("  render          Reed -> preamp -> WAV output");
    eprintln!("  bark-audit      Measure H2/H1 at each signal chain stage");
    eprintln!("  intermod-audit  Detect inharmonic intermodulation beating risk");
    eprintln!("  calibrate       Measure gain chain at 5 tap points → CSV");
    eprintln!("  sensitivity     Multi-DS grid sweep → CSV");
    eprintln!("  centroid-track  Spectral centroid tracking over time");
    eprintln!("  render-poly     Render multiple simultaneous notes through shared chain");
    eprintln!("  render-midi     Render a MIDI file through the full signal chain");
    eprintln!();
    eprintln!("Global options:");
    eprintln!("  --model MODEL   Preamp model: dk (default), ebers-moll");
    eprintln!();
    eprintln!("Use --help after any subcommand for options.");
}

fn parse_flag(args: &[String], flag: &str, default: f64) -> f64 {
    for i in 0..args.len().saturating_sub(1) {
        if args[i] == flag {
            return args[i + 1].parse().unwrap_or(default);
        }
    }
    default
}

fn parse_flag_str<'a>(args: &'a [String], flag: &str, default: &'a str) -> &'a str {
    for i in 0..args.len().saturating_sub(1) {
        if args[i] == flag {
            return &args[i + 1];
        }
    }
    default
}

// ─── Model selection ────────────────────────────────────────────────────────

fn create_preamp(args: &[String]) -> Box<dyn PreampModel> {
    let model = parse_flag_str(args, "--model", "dk");
    match model {
        "dk" => Box::new(DkPreamp::new(OVERSAMPLED_SR)),
        "ebers-moll" => Box::new(EbersMollPreamp::new(OVERSAMPLED_SR)),
        other => {
            eprintln!("Unknown model '{other}'. Use 'ebers-moll' or 'dk'.");
            std::process::exit(1);
        }
    }
}

// ─── Gain measurement ───────────────────────────────────────────────────────

/// Measure preamp gain by running a sine wave through the 2x-oversampled preamp.
fn measure_gain_at(preamp: &mut dyn PreampModel, freq: f64, amplitude: f64) -> f64 {
    preamp.reset();
    let mut os = Oversampler::new();

    let n_settle = (BASE_SR * 0.3) as usize;
    let n_measure = (BASE_SR * 0.2) as usize;

    // Settle — exercise the FULL signal path (upsample + preamp + downsample)
    // so that all filter states are primed before measurement begins.
    for i in 0..n_settle {
        let t = i as f64 / BASE_SR;
        let input = amplitude * (2.0 * PI * freq * t).sin();
        let mut up = [0.0f64; 2];
        os.upsample_2x(&[input], &mut up);
        let processed = [preamp.process_sample(up[0]), preamp.process_sample(up[1])];
        let mut down = [0.0f64; 1];
        os.downsample_2x(&processed, &mut down);
    }

    // Measure (downsample to get output at base rate)
    let mut peak = 0.0f64;
    for i in 0..n_measure {
        let t = (n_settle + i) as f64 / BASE_SR;
        let input = amplitude * (2.0 * PI * freq * t).sin();
        let mut up = [0.0f64; 2];
        os.upsample_2x(&[input], &mut up);
        let processed = [preamp.process_sample(up[0]), preamp.process_sample(up[1])];
        let mut down = [0.0f64; 1];
        os.downsample_2x(&processed, &mut down);
        peak = peak.max(down[0].abs());
    }

    peak / amplitude
}

fn cmd_gain(args: &[String]) {
    let freq = parse_flag(args, "--freq", 1000.0);
    let amplitude = parse_flag(args, "--amplitude", 0.001);
    let r_ldr = parse_flag(args, "--ldr", 1_000_000.0);

    let mut preamp = create_preamp(args);
    preamp.set_ldr_resistance(r_ldr);

    let gain = measure_gain_at(preamp.as_mut(), freq, amplitude);
    let gain_db = 20.0 * gain.log10();

    let target_db = if r_ldr > 500_000.0 { 6.0 } else { 12.1 };
    let delta = gain_db - target_db;

    println!("Preamp gain measurement");
    println!("  Frequency:   {freq:.0} Hz");
    println!("  Amplitude:   {amplitude:.6} V");
    println!("  LDR path:    {r_ldr:.0} Ω");
    println!("  Gain:        {gain:.3}x ({gain_db:.2} dB)");
    println!("  SPICE target: {target_db:.1} dB");
    println!("  Delta:       {delta:+.2} dB");
}

// ─── Frequency sweep ────────────────────────────────────────────────────────

fn cmd_sweep(args: &[String]) {
    let start = parse_flag(args, "--start", 20.0);
    let end = parse_flag(args, "--end", 20000.0);
    let points = parse_flag(args, "--points", 50.0) as usize;
    let r_ldr = parse_flag(args, "--ldr", 1_000_000.0);
    let amplitude = parse_flag(args, "--amplitude", 0.001);
    let csv_path = parse_flag_str(args, "--csv", "");

    let mut preamp = create_preamp(args);
    preamp.set_ldr_resistance(r_ldr);

    let log_start = start.ln();
    let log_end = end.ln();

    let mut csv_lines = Vec::new();
    csv_lines.push("freq_hz,gain_db".to_string());

    println!("Frequency response sweep (LDR = {r_ldr:.0} Ω)");
    println!("{:>10}  {:>10}", "Freq (Hz)", "Gain (dB)");
    println!("{:-<10}  {:-<10}", "", "");

    for i in 0..points {
        let frac = i as f64 / (points - 1).max(1) as f64;
        let freq = (log_start + frac * (log_end - log_start)).exp();

        let gain = measure_gain_at(preamp.as_mut(), freq, amplitude);
        let gain_db = 20.0 * gain.log10();

        println!("{freq:>10.1}  {gain_db:>10.2}");
        csv_lines.push(format!("{freq:.1},{gain_db:.2}"));
    }

    if !csv_path.is_empty() {
        std::fs::write(csv_path, csv_lines.join("\n") + "\n").expect("Failed to write CSV");
        println!("\nCSV written to {csv_path}");
    }
}

// ─── Harmonic analysis ──────────────────────────────────────────────────────

fn cmd_harmonics(args: &[String]) {
    let freq = parse_flag(args, "--freq", 440.0);
    let amplitude = parse_flag(args, "--amplitude", 0.005);
    let r_ldr = parse_flag(args, "--ldr", 1_000_000.0);

    let mut preamp = create_preamp(args);
    preamp.set_ldr_resistance(r_ldr);
    let mut os = Oversampler::new();

    let n_total = (BASE_SR * 0.5) as usize;
    let mut output = Vec::with_capacity(n_total);

    for i in 0..n_total {
        let t = i as f64 / BASE_SR;
        let input = amplitude * (2.0 * PI * freq * t).sin();
        let mut up = [0.0f64; 2];
        os.upsample_2x(&[input], &mut up);
        let processed = [preamp.process_sample(up[0]), preamp.process_sample(up[1])];
        let mut down = [0.0f64; 1];
        os.downsample_2x(&processed, &mut down);
        output.push(down[0]);
    }

    // Analyze last quarter (steady state)
    let start = output.len() * 3 / 4;
    let signal = &output[start..];

    let h1 = dft_magnitude(signal, freq, BASE_SR);
    let h2 = dft_magnitude(signal, 2.0 * freq, BASE_SR);
    let h3 = dft_magnitude(signal, 3.0 * freq, BASE_SR);
    let h4 = dft_magnitude(signal, 4.0 * freq, BASE_SR);
    let h5 = dft_magnitude(signal, 5.0 * freq, BASE_SR);

    let thd = ((h2 * h2 + h3 * h3 + h4 * h4 + h5 * h5).sqrt() / h1) * 100.0;
    let h2_h3_ratio = if h3 > 1e-15 {
        20.0 * (h2 / h3).log10()
    } else {
        f64::INFINITY
    };

    println!("Harmonic analysis");
    println!("  Frequency:   {freq:.0} Hz");
    println!("  Amplitude:   {amplitude:.6} V");
    println!("  LDR path:    {r_ldr:.0} Ω");
    println!();
    println!("  H1 (fund):   {h1:.6}");
    println!(
        "  H2:          {h2:.6}  ({:.1} dB rel)",
        20.0 * (h2 / h1).log10()
    );
    println!(
        "  H3:          {h3:.6}  ({:.1} dB rel)",
        20.0 * (h3 / h1).log10()
    );
    println!(
        "  H4:          {h4:.6}  ({:.1} dB rel)",
        20.0 * (h4 / h1).log10()
    );
    println!(
        "  H5:          {h5:.6}  ({:.1} dB rel)",
        20.0 * (h5 / h1).log10()
    );
    println!();
    println!("  THD:         {thd:.4}%");
    println!("  H2/H3:       {h2_h3_ratio:.1} dB  (target: H2 > H3, i.e. > 0 dB)");
}

// ─── Tremolo sweep ──────────────────────────────────────────────────────────

fn cmd_tremolo_sweep(args: &[String]) {
    let ldr_min = parse_flag(args, "--ldr-min", 19_000.0);
    let ldr_max = parse_flag(args, "--ldr-max", 1_000_000.0);
    let steps = parse_flag(args, "--steps", 20.0) as usize;
    let freq = parse_flag(args, "--freq", 1000.0);
    let amplitude = parse_flag(args, "--amplitude", 0.001);
    let csv_path = parse_flag_str(args, "--csv", "");

    let mut preamp = create_preamp(args);

    let log_min = ldr_min.ln();
    let log_max = ldr_max.ln();

    let mut csv_lines = Vec::new();
    csv_lines.push("ldr_ohm,gain_db".to_string());

    println!("Tremolo sweep (gain vs LDR path resistance)");
    println!("{:>12}  {:>10}", "LDR (Ω)", "Gain (dB)");
    println!("{:-<12}  {:-<10}", "", "");

    for i in 0..steps {
        let frac = i as f64 / (steps - 1).max(1) as f64;
        let r_ldr = (log_min + frac * (log_max - log_min)).exp();

        preamp.set_ldr_resistance(r_ldr);
        let gain = measure_gain_at(preamp.as_mut(), freq, amplitude);
        let gain_db = 20.0 * gain.log10();

        println!("{r_ldr:>12.0}  {gain_db:>10.2}");
        csv_lines.push(format!("{r_ldr:.0},{gain_db:.2}"));
    }

    // SPICE targets
    println!();
    println!("SPICE targets:");
    println!("  R_ldr = 1M  (no trem):     6.0 dB");
    println!("  R_ldr = 19K (trem bright): 12.1 dB");
    println!("  Range:                      6.1 dB");

    if !csv_path.is_empty() {
        std::fs::write(csv_path, csv_lines.join("\n") + "\n").expect("Failed to write CSV");
        println!("\nCSV written to {csv_path}");
    }
}

// ─── Render (reed -> preamp -> WAV) ─────────────────────────────────────────

fn cmd_render(args: &[String]) {
    let note = parse_flag(args, "--note", 60.0) as u8;
    let velocity = parse_flag(args, "--velocity", 100.0) as u8;
    let duration = parse_flag(args, "--duration", 2.0);
    let r_ldr = parse_flag(args, "--ldr", 1_000_000.0);
    let volume = parse_flag(args, "--volume", 0.60);
    let speaker_char = parse_flag(args, "--speaker", 1.0);
    let tremolo_rate = parse_flag(args, "--tremolo-rate", 5.63);
    let tremolo_depth = parse_flag(args, "--tremolo-depth", 0.0);
    let no_poweramp = args.contains(&"--no-poweramp".to_string());
    let no_preamp = args.contains(&"--no-preamp".to_string());
    let normalize = args.contains(&"--normalize".to_string());
    let disp_scale: Option<f64> = if args.contains(&"--displacement-scale".to_string()) {
        Some(parse_flag(args, "--displacement-scale", 0.30))
    } else {
        None
    };
    let output_path = parse_flag_str(args, "--output", "/tmp/preamp_render.wav");

    // Render reed voice (reed → pickup with nonlinearity + HPF)
    let reed_output =
        Voice::render_note_with_scale(note, velocity as f64 / 127.0, duration, BASE_SR, disp_scale);

    // Process through oversampled preamp (or bypass)
    let n_samples = reed_output.len();
    let preamp_output = if no_preamp {
        reed_output.clone()
    } else {
        let mut preamp = create_preamp(args);

        // Tremolo: when depth > 0, instantiate LFO at oversampled rate.
        // Otherwise use static LDR resistance (--ldr flag).
        let mut tremolo = if tremolo_depth > 0.0 {
            let mut t = Tremolo::new(tremolo_rate, tremolo_depth, OVERSAMPLED_SR);
            t.set_depth(tremolo_depth);
            Some(t)
        } else {
            preamp.set_ldr_resistance(r_ldr);
            preamp.reset(); // Re-solve DC at the new R_ldr
            None
        };

        let mut os = Oversampler::new();
        let mut out = vec![0.0f64; n_samples];
        for i in 0..n_samples {
            let mut up = [0.0f64; 2];
            os.upsample_2x(&[reed_output[i]], &mut up);
            let processed = [
                {
                    if let Some(ref mut trem) = tremolo {
                        preamp.set_ldr_resistance(trem.process());
                    }
                    preamp.process_sample(up[0])
                },
                {
                    if let Some(ref mut trem) = tremolo {
                        preamp.set_ldr_resistance(trem.process());
                    }
                    preamp.process_sample(up[1])
                },
            ];
            let mut down = [0.0f64; 1];
            os.downsample_2x(&processed, &mut down);
            out[i] = down[0];
        }
        out
    };

    // Output stage: volume → power amp (gain + crossover + clip) → speaker
    // Matches the plugin signal chain in lib.rs
    let mut power_amp = PowerAmp::new();
    let mut speaker = Speaker::new(BASE_SR);
    speaker.set_character(speaker_char);

    let mut final_output = vec![0.0f64; n_samples];
    for i in 0..n_samples {
        let attenuated = preamp_output[i] * volume;
        let amplified = if no_poweramp {
            attenuated
        } else {
            power_amp.process(attenuated)
        };
        final_output[i] = speaker.process(amplified);
    }

    // Peak measurement
    let peak = final_output.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
    let peak_dbfs = if peak > 0.0 {
        20.0 * peak.log10()
    } else {
        -120.0
    };

    // Normalization: opt-in only. Default writes raw samples.
    let scale = if normalize {
        if peak > 0.7 { 0.7 / peak } else { 1.0 }
    } else {
        1.0
    };

    if !normalize && peak > 1.0 {
        eprintln!(
            "WARNING: Peak exceeds 0 dBFS ({peak_dbfs:.1} dBFS) — consider reducing --volume"
        );
    }

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: BASE_SR as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(output_path, spec).expect("Failed to create WAV file");

    let max_val = (1 << 23) - 1;
    for sample in &final_output {
        let scaled = (sample * scale * max_val as f64).round() as i32;
        writer
            .write_sample(scaled.clamp(-max_val, max_val))
            .unwrap();
    }
    writer.finalize().unwrap();

    println!("Render complete");
    println!("  Note:      MIDI {note}");
    println!("  Velocity:  {velocity}");
    println!("  Duration:  {duration:.1}s");
    if tremolo_depth > 0.0 {
        println!("  Tremolo:   rate={tremolo_rate:.1} Hz, depth={tremolo_depth:.2}");
    } else {
        println!("  LDR:       {r_ldr:.0} Ω (static)");
    }
    println!("  Volume:    {volume:.3} (PA gain: 69x, headroom: 22V)");
    println!("  Speaker:   {speaker_char:.1}");
    if let Some(ds) = disp_scale {
        println!("  Disp scale: {ds:.3}");
    }
    if no_preamp {
        println!("  Preamp:    BYPASSED");
    }
    if no_poweramp {
        println!("  Power amp: BYPASSED");
    }
    if normalize {
        println!("  Normalize: ON (-3 dBFS ceiling)");
    }
    println!("  Peak:      {peak_dbfs:.1} dBFS (raw)");
    println!("  Build:     v{}", env!("CARGO_PKG_VERSION"));
    println!("  Output:    {output_path}");
}

// ─── Bark audit ─────────────────────────────────────────────────────────────

/// Measure H2/H1 at every stage of the signal chain to diagnose bark deficiency.
///
/// Stages measured:
///   1. Raw reed (modal synthesis)
///   2. After pickup nonlinearity (y/(1-y) * SENSITIVITY, no HPF)
///   3. After pickup HPF (full pickup output)
///   4. After preamp (oversampled)
fn cmd_bark_audit(args: &[String]) {
    // Pickup constants (duplicated here since they're private in pickup.rs)
    const SENSITIVITY: f64 = 1.8375;
    const MAX_Y: f64 = 0.90;
    const PICKUP_HPF_HZ: f64 = 2312.0;

    let notes: Vec<u8> = if args.iter().any(|a| a == "--notes") {
        let s = parse_flag_str(args, "--notes", "36,48,60,72,84");
        s.split(',').filter_map(|n| n.trim().parse().ok()).collect()
    } else {
        vec![36, 48, 60, 72, 84]
    };
    let velocities: Vec<u8> = if args.iter().any(|a| a == "--velocities") {
        let s = parse_flag_str(args, "--velocities", "40,80,100,127");
        s.split(',').filter_map(|v| v.trim().parse().ok()).collect()
    } else {
        vec![40, 80, 100, 127]
    };

    let duration = 0.5; // seconds
    let measure_start = 0.25; // start measuring at 250ms (steady state)

    println!("=== BARK AUDIT: H2/H1 at each signal chain stage ===");
    println!();
    println!(
        "{:>6} {:>4}  {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Note",
        "Vel",
        "Reed pk",
        "y_peak",
        "NL H2/H1",
        "NL pk(mV)",
        "HPF H2/H1",
        "HPF pk(mV)",
        "Pre H2/H1"
    );
    println!("{}", "-".repeat(100));

    for &note in &notes {
        let params = tables::note_params(note);
        let freq = params.fundamental_hz;
        let h2_freq = freq * 2.0;

        for &vel_byte in &velocities {
            let velocity = vel_byte as f64 / 127.0;

            // ── Stage 1: Raw reed ──
            let detuned = params.fundamental_hz * variation::freq_detune(note);
            let dwell = dwell_attenuation(velocity, detuned, &params.mode_ratios);
            let amp_offsets = variation::mode_amplitude_offsets(note);
            let vel_exp = tables::velocity_exponent(note);
            let vel_scale = tables::velocity_scurve(velocity).powf(vel_exp);
            let _out_scale = tables::output_scale(note, velocity); // post-pickup only

            let mut amplitudes = [0.0f64; NUM_MODES];
            for i in 0..NUM_MODES {
                // output_scale is now post-pickup (decoupled from nonlinearity)
                amplitudes[i] = params.mode_amplitudes[i] * dwell[i] * amp_offsets[i] * vel_scale;
            }

            let mut reed = ModalReed::new(
                detuned,
                &params.mode_ratios,
                &amplitudes,
                &params.mode_decay_rates,
                0.0,
                velocity,
                BASE_SR,
                (note as u32).wrapping_mul(2654435761),
            );

            let n_samples = (duration * BASE_SR) as usize;
            let measure_offset = (measure_start * BASE_SR) as usize;
            let mut reed_buf = vec![0.0f64; n_samples];
            reed.render(&mut reed_buf);

            let reed_steady = &reed_buf[measure_offset..];
            let reed_peak = reed_steady.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
            let _reed_h1 = dft_magnitude(reed_steady, freq, BASE_SR);
            let _reed_h2 = dft_magnitude(reed_steady, h2_freq, BASE_SR);

            // ── Stage 2: After nonlinearity (before HPF) ──
            let displacement_scale = tables::pickup_displacement_scale(note);
            let mut nl_buf = reed_buf.clone();
            for s in &mut nl_buf {
                let y = (*s * displacement_scale).clamp(-MAX_Y, MAX_Y);
                *s = (y / (1.0 - y)) * SENSITIVITY;
            }
            let nl_steady = &nl_buf[measure_offset..];
            let nl_peak = nl_steady.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
            let nl_h1 = dft_magnitude(nl_steady, freq, BASE_SR);
            let nl_h2 = dft_magnitude(nl_steady, h2_freq, BASE_SR);
            let nl_h2_h1 = if nl_h1 > 1e-15 { nl_h2 / nl_h1 } else { 0.0 };
            let y_peak = reed_peak * displacement_scale;

            // ── Stage 3: After pickup HPF ──
            let mut hpf = OnePoleHpf::new(PICKUP_HPF_HZ, BASE_SR);
            let mut hpf_buf = nl_buf.clone();
            for s in &mut hpf_buf {
                *s = hpf.process(*s);
            }
            let hpf_steady = &hpf_buf[measure_offset..];
            let hpf_peak = hpf_steady.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
            let hpf_h1 = dft_magnitude(hpf_steady, freq, BASE_SR);
            let hpf_h2 = dft_magnitude(hpf_steady, h2_freq, BASE_SR);
            let hpf_h2_h1 = if hpf_h1 > 1e-15 { hpf_h2 / hpf_h1 } else { 0.0 };

            // ── Stage 4: After preamp (oversampled) ──
            let mut preamp = create_preamp(args);
            preamp.set_ldr_resistance(1_000_000.0);
            let mut os = Oversampler::new();

            let mut preamp_buf = vec![0.0f64; n_samples];
            for i in 0..n_samples {
                let mut up = [0.0f64; 2];
                os.upsample_2x(&[hpf_buf[i]], &mut up);
                let processed = [preamp.process_sample(up[0]), preamp.process_sample(up[1])];
                let mut down = [0.0f64; 1];
                os.downsample_2x(&processed, &mut down);
                preamp_buf[i] = down[0];
            }
            let pre_steady = &preamp_buf[measure_offset..];
            let pre_h1 = dft_magnitude(pre_steady, freq, BASE_SR);
            let pre_h2 = dft_magnitude(pre_steady, h2_freq, BASE_SR);
            let pre_h2_h1 = if pre_h1 > 1e-15 { pre_h2 / pre_h1 } else { 0.0 };

            // ── Report ──
            let note_name = midi_note_name(note);
            println!(
                "{:>6} {:>4}  {:>10.4} {:>10.4} {:>9.1}% {:>10.2} {:>9.1}% {:>10.2} {:>9.1}%",
                note_name,
                vel_byte,
                reed_peak,
                y_peak,
                nl_h2_h1 * 100.0,
                nl_peak * 1000.0,
                hpf_h2_h1 * 100.0,
                hpf_peak * 1000.0,
                pre_h2_h1 * 100.0,
            );
        }
        println!();
    }

    // Summary
    println!("Legend:");
    println!("  Reed pk   = peak reed displacement (model units)");
    println!(
        "  y_peak    = physical displacement fraction (y = reed_pk * DS), DS = per-note from tables"
    );
    println!("  NL H2/H1  = H2/H1 after 1/(1-y) nonlinearity, before HPF");
    println!("  NL pk(mV) = peak signal after nonlinearity (millivolts)");
    println!("  HPF H2/H1 = H2/H1 after pickup RC HPF at {PICKUP_HPF_HZ} Hz");
    println!("  HPF pk(mV)= peak signal after HPF (millivolts, feeds preamp)");
    println!("  Pre H2/H1 = H2/H1 after preamp (2x gain, no tremolo)");
    println!();
    println!("SPICE targets: y=0.10 → NL H2/H1 = 8.7%, HPF boosts H2 ~1.9x relative to H1");
}

fn midi_note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let name = NAMES[(note % 12) as usize];
    let octave = (note as i32 / 12) - 1;
    format!("{name}{octave}")
}

// ─── Intermod audit ─────────────────────────────────────────────────────────

/// Spectral grass metric: ratio of harmonic energy to midpoint (inter-harmonic) energy.
///
/// Sums DFT magnitude² at integer harmonics 1..N and at midpoints (n+0.5)×f₁.
/// Returns (harmonic_energy_db, midpoint_energy_db, ratio_db).
/// Higher ratio = cleaner. >40 dB = clean, <20 dB = dirty.
fn spectral_grass(
    signal: &[f64],
    fundamental_hz: f64,
    sr: f64,
    max_harmonic: usize,
) -> (f64, f64, f64) {
    let mut harmonic_energy = 0.0f64;
    let mut midpoint_energy = 0.0f64;

    for n in 1..=max_harmonic {
        let freq = n as f64 * fundamental_hz;
        if freq >= sr / 2.0 {
            break;
        }
        let mag = dft_magnitude(signal, freq, sr);
        harmonic_energy += mag * mag;
    }

    for n in 1..max_harmonic {
        let freq = (n as f64 + 0.5) * fundamental_hz;
        if freq >= sr / 2.0 {
            break;
        }
        let mag = dft_magnitude(signal, freq, sr);
        midpoint_energy += mag * mag;
    }

    let h_db = if harmonic_energy > 0.0 {
        10.0 * harmonic_energy.log10()
    } else {
        -120.0
    };
    let m_db = if midpoint_energy > 0.0 {
        10.0 * midpoint_energy.log10()
    } else {
        -120.0
    };
    (h_db, m_db, h_db - m_db)
}

fn cmd_intermod_audit(args: &[String]) {
    let threshold = parse_flag(args, "--threshold", 0.07);
    let do_render = args.contains(&"--render".to_string());
    let duration = parse_flag(args, "--duration", 3.0);

    let notes: Vec<u8> = if args.iter().any(|a| a == "--notes") {
        let s = parse_flag_str(args, "--notes", "");
        s.split(',').filter_map(|n| n.trim().parse().ok()).collect()
    } else {
        (tables::MIDI_LO..=tables::MIDI_HI).collect()
    };

    // ── Static analysis ──
    println!("=== INTERMOD RISK AUDIT ===");
    println!("Threshold: {threshold:.4}");
    println!();
    println!(
        "{:>6} {:>4} {:>6}  {:>5} {:>6} {:>8} {:>8} {:>7} {:>7} {:>8}",
        "Note", "MIDI", "mu", "Mode", "Ratio", "Offset", "Beat Hz", "Eff Amp", "Weight", "Risk"
    );
    println!("{}", "-".repeat(82));

    let mut flagged_notes = Vec::new();

    for &midi in &notes {
        let report = tables::intermod_risk(midi);
        let note_name = midi_note_name(midi);
        let flagged = report.max_risk >= threshold;

        if flagged {
            flagged_notes.push(midi);
        }

        // Print the highest-risk product for each note (compact view)
        if let Some(worst) = report
            .products
            .iter()
            .max_by(|a, b| a.risk_score.partial_cmp(&b.risk_score).unwrap())
        {
            let flag = if flagged { " ***" } else { "" };
            println!(
                "{:>6} {:>4} {:>6.4}  {:>5} {:>6.3} {:>8.5} {:>8.2} {:>7.4} {:>7.3} {:>8.5}{}",
                note_name,
                midi,
                report.mu,
                worst.mode,
                worst.mode_ratio,
                worst.fractional_offset,
                worst.beat_hz,
                worst.effective_amplitude,
                worst.perceptual_weight,
                worst.risk_score,
                flag
            );
        }
    }

    println!();
    println!(
        "Flagged notes (risk >= {threshold:.4}): {}",
        flagged_notes.len()
    );
    if !flagged_notes.is_empty() {
        let names: Vec<String> = flagged_notes
            .iter()
            .map(|&m| format!("{} ({})", midi_note_name(m), m))
            .collect();
        println!("  {}", names.join(", "));
    }

    // ── Render analysis (optional) ──
    if !do_render {
        if !flagged_notes.is_empty() {
            println!();
            println!("Run with --render to analyze flagged notes spectrally.");
        }
        return;
    }

    let render_notes = if args.iter().any(|a| a == "--notes") {
        notes.clone()
    } else {
        flagged_notes.clone()
    };

    if render_notes.is_empty() {
        println!();
        println!("No notes to render-analyze. All clear!");
        return;
    }

    println!();
    println!("=== RENDER ANALYSIS (sustain spectral grass) ===");
    println!("Duration: {duration:.1}s, analysis window: 0.5-2.0s");
    println!();
    println!(
        "{:>6} {:>4}  {:>10} {:>10} {:>10}  {:>8}",
        "Note", "MIDI", "Harm (dB)", "Mid (dB)", "Ratio (dB)", "Verdict"
    );
    println!("{}", "-".repeat(64));

    for &midi in &render_notes {
        let fundamental_hz = tables::midi_to_freq(midi);

        // Render: reed + pickup, no preamp (pickup nonlinearity is the intermod source)
        let signal = Voice::render_note(midi, 1.0, duration, BASE_SR);

        // Extract sustain window
        let start = (0.5 * BASE_SR) as usize;
        let end = (2.0 * BASE_SR).min(signal.len() as f64) as usize;
        if end <= start {
            println!(
                "{:>6} {:>4}  (signal too short)",
                midi_note_name(midi),
                midi
            );
            continue;
        }
        let sustain = &signal[start..end];

        let max_harmonic = (BASE_SR / 2.0 / fundamental_hz).floor() as usize;
        let (h_db, m_db, ratio_db) =
            spectral_grass(sustain, fundamental_hz, BASE_SR, max_harmonic.min(32));

        let verdict = if ratio_db > 40.0 {
            "CLEAN"
        } else if ratio_db > 30.0 {
            "OK"
        } else if ratio_db > 20.0 {
            "MARGINAL"
        } else {
            "DIRTY"
        };

        println!(
            "{:>6} {:>4}  {:>10.1} {:>10.1} {:>10.1}  {:>8}",
            midi_note_name(midi),
            midi,
            h_db,
            m_db,
            ratio_db,
            verdict
        );

        // For marginal/dirty: print per-product detail
        if ratio_db <= 30.0 {
            let report = tables::intermod_risk(midi);
            println!("  Per-product detail:");
            for p in &report.products {
                if p.risk_score < 0.001 {
                    continue;
                }
                let intermod_freq = p.mode_ratio * fundamental_hz;
                let nearest_freq = p.nearest_integer as f64 * fundamental_hz;
                let intermod_mag = dft_magnitude(sustain, intermod_freq, BASE_SR);
                let nearest_mag = dft_magnitude(sustain, nearest_freq, BASE_SR);
                let ratio = if nearest_mag > 1e-15 {
                    20.0 * (intermod_mag / nearest_mag).log10()
                } else {
                    0.0
                };
                println!(
                    "    Mode {}: {:.1} Hz (near H{} @ {:.1} Hz) intermod/harmonic = {:.1} dB, risk={:.5}",
                    p.mode, intermod_freq, p.nearest_integer, nearest_freq, ratio, p.risk_score
                );
            }
        }
    }
}

// ─── DFT helper ─────────────────────────────────────────────────────────────

fn dft_magnitude(signal: &[f64], freq: f64, sr: f64) -> f64 {
    let n = signal.len() as f64;
    let mut re = 0.0;
    let mut im = 0.0;
    for (i, &s) in signal.iter().enumerate() {
        let phase = 2.0 * PI * freq * i as f64 / sr;
        re += s * phase.cos();
        im -= s * phase.sin();
    }
    2.0 * ((re / n).powi(2) + (im / n).powi(2)).sqrt()
}

// ─── Signal measurement helpers ────────────────────────────────────────────

fn parse_csv_list<T: std::str::FromStr>(args: &[String], flag: &str, default: &str) -> Vec<T> {
    let s = parse_flag_str(args, flag, default);
    s.split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

fn peak_db(signal: &[f64]) -> f64 {
    let peak = signal.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
    if peak > 0.0 {
        20.0 * peak.log10()
    } else {
        -120.0
    }
}

fn rms_db(signal: &[f64]) -> f64 {
    let mean_sq = signal.iter().map(|x| x * x).sum::<f64>() / signal.len() as f64;
    if mean_sq > 0.0 {
        10.0 * mean_sq.log10()
    } else {
        -120.0
    }
}

fn h2_h1_ratio_db(signal: &[f64], fundamental_hz: f64, sr: f64) -> f64 {
    let h1 = dft_magnitude(signal, fundamental_hz, sr);
    let h2 = dft_magnitude(signal, 2.0 * fundamental_hz, sr);
    if h1 > 1e-15 {
        20.0 * (h2 / h1).log10()
    } else {
        -120.0
    }
}

// ─── Calibrate subcommand ──────────────────────────────────────────────────

/// Single-config measurement at 5 tap points across the signal chain.
fn cmd_calibrate(args: &[String]) {
    let notes: Vec<u8> = parse_csv_list(args, "--notes", "36,40,44,48,52,56,60,64,68,72,76,80,84");
    let velocities: Vec<u8> = parse_csv_list(args, "--velocities", "40,80,127");
    let ds_at_c4 = parse_flag(args, "--ds-at-c4", 0.85);
    let volume = parse_flag(args, "--volume", 0.40);
    let speaker_char = parse_flag(args, "--speaker", 1.0);
    let zero_trim = args.contains(&"--zero-trim".to_string());
    let mlp = args.contains(&"--mlp".to_string());
    let output_path = parse_flag_str(args, "--output", "/tmp/calibrate.csv");

    let cfg = CalibrationConfig {
        ds_at_c4,
        zero_trim,
        ..CalibrationConfig::default()
    };

    let rows = run_calibrate(&notes, &velocities, &cfg, volume, speaker_char, mlp, args);

    write_calibrate_csv(output_path, &rows);
    eprintln!(
        "Calibrate: {} notes × {} velocities = {} rows → {}",
        notes.len(),
        velocities.len(),
        rows.len(),
        output_path
    );
}

#[derive(Clone)]
struct CalibrateRow {
    midi: u8,
    velocity: u8,
    ds_at_c4: f64,
    ds_actual: f64,
    y_peak: f64,
    t2_peak_db: f64,
    t2_rms_db: f64,
    t2_h2_h1_db: f64,
    t3_peak_db: f64,
    t3_rms_db: f64,
    t4_peak_db: f64,
    t4_rms_db: f64,
    t4_h2_h1_db: f64,
    t5_peak_db: f64,
    t5_rms_db: f64,
    t5_h2_h1_db: f64,
    proxy_db: f64,
    trim_db: f64,
    proxy_error_db: f64,
    tanh_compression_db: f64,
}

fn run_calibrate(
    notes: &[u8],
    velocities: &[u8],
    cfg: &CalibrationConfig,
    volume: f64,
    speaker_char: f64,
    _mlp: bool,
    args: &[String],
) -> Vec<CalibrateRow> {
    // Pickup constants (same as bark-audit)
    const SENSITIVITY: f64 = 1.8375;
    const MAX_Y: f64 = 0.90;
    const PICKUP_HPF_HZ: f64 = 2312.0;

    let duration = 0.5;
    let measure_start = (0.100 * BASE_SR) as usize; // 100ms
    let measure_end = (0.400 * BASE_SR) as usize; // 400ms

    let mut rows = Vec::new();

    for &note in notes {
        let params = tables::note_params(note);
        let freq = params.fundamental_hz;
        let ds_actual = tables::pickup_displacement_scale_with_config(note, cfg);

        for &vel_byte in velocities {
            let velocity = vel_byte as f64 / 127.0;

            // ── T1: Raw reed ──
            // Construct reed exactly as bark-audit does
            let detuned = params.fundamental_hz * variation::freq_detune(note);
            let dwell = dwell_attenuation(velocity, detuned, &params.mode_ratios);
            let amp_offsets = variation::mode_amplitude_offsets(note);
            let vel_exp = tables::velocity_exponent(note);
            let vel_scale = tables::velocity_scurve(velocity).powf(vel_exp);

            let mut amplitudes = [0.0f64; NUM_MODES];
            for i in 0..NUM_MODES {
                amplitudes[i] = params.mode_amplitudes[i] * dwell[i] * amp_offsets[i] * vel_scale;
            }

            let mut reed = ModalReed::new(
                detuned,
                &params.mode_ratios,
                &amplitudes,
                &params.mode_decay_rates,
                0.0,
                velocity,
                BASE_SR,
                (note as u32).wrapping_mul(2654435761),
            );

            let n_samples = (duration * BASE_SR) as usize;
            let mut reed_buf = vec![0.0f64; n_samples];
            reed.render(&mut reed_buf);

            let reed_peak = reed_buf[measure_start..measure_end]
                .iter()
                .map(|x| x.abs())
                .fold(0.0f64, f64::max);
            let y_peak = reed_peak * ds_actual;

            // ── T2: After pickup nonlinearity + HPF ──
            let mut nl_buf = reed_buf.clone();
            for s in &mut nl_buf {
                let y = (*s * ds_actual).clamp(-MAX_Y, MAX_Y);
                *s = (y / (1.0 - y)) * SENSITIVITY;
            }
            let mut hpf = OnePoleHpf::new(PICKUP_HPF_HZ, BASE_SR);
            let mut t2_buf = nl_buf.clone();
            for s in &mut t2_buf {
                *s = hpf.process(*s);
            }
            let t2_window = &t2_buf[measure_start..measure_end];
            let t2_pk = peak_db(t2_window);
            let t2_rm = rms_db(t2_window);
            let t2_h2 = h2_h1_ratio_db(t2_window, freq, BASE_SR);

            // ── T3: After output_scale ──
            let out_scale = tables::output_scale_with_config(note, velocity, cfg);
            let t3_buf: Vec<f64> = t2_buf.iter().map(|s| s * out_scale).collect();
            let t3_window = &t3_buf[measure_start..measure_end];
            let t3_pk = peak_db(t3_window);
            let t3_rm = rms_db(t3_window);

            // ── T4: After preamp (oversampled) ──
            let mut preamp = create_preamp(args);
            preamp.set_ldr_resistance(1_000_000.0);
            let mut os = Oversampler::new();

            let mut t4_buf = vec![0.0f64; n_samples];
            for i in 0..n_samples {
                let mut up = [0.0f64; 2];
                os.upsample_2x(&[t3_buf[i]], &mut up);
                let processed = [preamp.process_sample(up[0]), preamp.process_sample(up[1])];
                let mut down = [0.0f64; 1];
                os.downsample_2x(&processed, &mut down);
                t4_buf[i] = down[0];
            }
            let t4_window = &t4_buf[measure_start..measure_end];
            let t4_pk = peak_db(t4_window);
            let t4_rm = rms_db(t4_window);
            let t4_h2 = h2_h1_ratio_db(t4_window, freq, BASE_SR);

            // ── T5: After volume + power amp + speaker ──
            let mut power_amp = PowerAmp::new();
            let mut speaker = Speaker::new(BASE_SR);
            speaker.set_character(speaker_char);

            let mut t5_buf = vec![0.0f64; n_samples];
            for i in 0..n_samples {
                let attenuated = t4_buf[i] * volume;
                let amplified = power_amp.process(attenuated);
                t5_buf[i] = speaker.process(amplified);
            }
            let t5_window = &t5_buf[measure_start..measure_end];
            let t5_pk = peak_db(t5_window);
            let t5_rm = rms_db(t5_window);
            let t5_h2 = h2_h1_ratio_db(t5_window, freq, BASE_SR);

            // ── Derived metrics ──
            let proxy = 20.0 * out_scale.log10();
            let trim = if cfg.zero_trim {
                0.0
            } else {
                tables::register_trim_db(note)
            };
            let proxy_error = t3_rm - cfg.target_db; // how far from target
            let tanh_compression = t4_pk - t5_pk;

            rows.push(CalibrateRow {
                midi: note,
                velocity: vel_byte,
                ds_at_c4: cfg.ds_at_c4,
                ds_actual,
                y_peak,
                t2_peak_db: t2_pk,
                t2_rms_db: t2_rm,
                t2_h2_h1_db: t2_h2,
                t3_peak_db: t3_pk,
                t3_rms_db: t3_rm,
                t4_peak_db: t4_pk,
                t4_rms_db: t4_rm,
                t4_h2_h1_db: t4_h2,
                t5_peak_db: t5_pk,
                t5_rms_db: t5_rm,
                t5_h2_h1_db: t5_h2,
                proxy_db: proxy,
                trim_db: trim,
                proxy_error_db: proxy_error,
                tanh_compression_db: tanh_compression,
            });

            eprint!(".");
        }
    }
    eprintln!();
    rows
}

fn write_calibrate_csv(path: &str, rows: &[CalibrateRow]) {
    let mut f = std::fs::File::create(path).expect("Failed to create CSV");
    writeln!(
        f,
        "midi,note_name,velocity,ds_at_c4,ds_actual,y_peak,\
         t2_peak_db,t2_rms_db,t2_h2_h1_db,\
         t3_peak_db,t3_rms_db,\
         t4_peak_db,t4_rms_db,t4_h2_h1_db,\
         t5_peak_db,t5_rms_db,t5_h2_h1_db,\
         proxy_db,trim_db,proxy_error_db,tanh_compression_db"
    )
    .unwrap();
    for r in rows {
        writeln!(
            f,
            "{},{},{},{:.4},{:.4},{:.4},\
             {:.2},{:.2},{:.2},\
             {:.2},{:.2},\
             {:.2},{:.2},{:.2},\
             {:.2},{:.2},{:.2},\
             {:.2},{:.2},{:.2},{:.2}",
            r.midi,
            midi_note_name(r.midi),
            r.velocity,
            r.ds_at_c4,
            r.ds_actual,
            r.y_peak,
            r.t2_peak_db,
            r.t2_rms_db,
            r.t2_h2_h1_db,
            r.t3_peak_db,
            r.t3_rms_db,
            r.t4_peak_db,
            r.t4_rms_db,
            r.t4_h2_h1_db,
            r.t5_peak_db,
            r.t5_rms_db,
            r.t5_h2_h1_db,
            r.proxy_db,
            r.trim_db,
            r.proxy_error_db,
            r.tanh_compression_db,
        )
        .unwrap();
    }
}

// ─── Sensitivity subcommand ────────────────────────────────────────────────

/// Multi-DS grid sweep: run calibrate at each DS_AT_C4 value.
fn cmd_sensitivity(args: &[String]) {
    let notes: Vec<u8> = parse_csv_list(args, "--notes", "36,48,54,60,66,72,78,84");
    let velocities: Vec<u8> = parse_csv_list(args, "--velocities", "40,80,127");
    let ds_values: Vec<f64> = parse_csv_list(
        args,
        "--ds-range",
        "0.50,0.55,0.60,0.65,0.70,0.75,0.80,0.85",
    );
    let volume = parse_flag(args, "--volume", 0.40);
    let speaker_char = parse_flag(args, "--speaker", 1.0);
    let scale_mode_raw = parse_flag_str(args, "--scale-mode", "track");
    // Honor --zero-trim as shorthand for --scale-mode zero-trim
    let scale_mode = if args.contains(&"--zero-trim".to_string()) {
        "zero-trim"
    } else {
        scale_mode_raw
    };
    let mlp = args.contains(&"--mlp".to_string());
    let output_path = parse_flag_str(args, "--output", "/tmp/sensitivity.csv");

    let total = ds_values.len() * notes.len() * velocities.len();
    eprintln!(
        "Sensitivity: {} DS × {} notes × {} vel = {} renders",
        ds_values.len(),
        notes.len(),
        velocities.len(),
        total
    );

    let mut all_rows = Vec::new();

    for &ds in &ds_values {
        let cfg = match scale_mode {
            // "freeze" = output_scale at original DS=0.85, pickup at test DS.
            // Since run_calibrate uses cfg for both, freeze just keeps original.
            // The ds_at_c4 column reports the sweep value for plotting.
            "freeze" => CalibrationConfig {
                ds_at_c4: 0.85,
                zero_trim: false,
                ..CalibrationConfig::default()
            },
            "zero-trim" => CalibrationConfig {
                ds_at_c4: ds,
                zero_trim: true,
                ..CalibrationConfig::default()
            },
            // "track" (default): override DS, keep trim
            _ => CalibrationConfig {
                ds_at_c4: ds,
                zero_trim: false,
                ..CalibrationConfig::default()
            },
        };

        let mut rows = run_calibrate(&notes, &velocities, &cfg, volume, speaker_char, mlp, args);

        // Stamp the ds_at_c4 column to the sweep value for analysis
        for r in &mut rows {
            r.ds_at_c4 = ds;
        }

        all_rows.extend(rows);
    }

    write_calibrate_csv(output_path, &all_rows);
    eprintln!(
        "Sensitivity: {} total rows → {}",
        all_rows.len(),
        output_path
    );
}

// ─── Polyphonic render ────────────────────────────────────────────────────

/// Render multiple simultaneous notes through the shared signal chain.
///
/// Usage:
///   preamp-bench render-poly --notes 38,62,66 --velocities 45,40,40 --duration 3.0
///
/// Voices are summed before the preamp (matching the plugin architecture),
/// so intermodulation from the shared nonlinear chain is captured.
fn cmd_render_poly(args: &[String]) {
    let notes: Vec<u8> = parse_csv_list(args, "--notes", "38,59,62,66");
    let velocities_raw: Vec<u8> = parse_csv_list(args, "--velocities", "45,40,40,40");
    let duration = parse_flag(args, "--duration", 3.0);
    let volume = parse_flag(args, "--volume", 0.60);
    let speaker_char = parse_flag(args, "--speaker", 1.0);
    let r_ldr = parse_flag(args, "--ldr", 1_000_000.0);
    let no_poweramp = args.contains(&"--no-poweramp".to_string());
    let normalize = args.contains(&"--normalize".to_string());
    let output_path = parse_flag_str(args, "--output", "/tmp/preamp_render_poly.wav");

    // Pad velocities to match notes if needed
    let velocities: Vec<u8> = notes
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if i < velocities_raw.len() {
                velocities_raw[i]
            } else {
                *velocities_raw.last().unwrap_or(&80)
            }
        })
        .collect();

    let n_samples = (duration * BASE_SR) as usize;

    // Render each voice independently (reed → pickup → output_scale)
    eprintln!(
        "Rendering {} voices, {:.1}s @ {:.0} Hz...",
        notes.len(),
        duration,
        BASE_SR
    );

    let mut sum_buf = vec![0.0f64; n_samples];
    let mut individual_bufs: Vec<Vec<f64>> = Vec::new();

    for (i, (&note, &vel)) in notes.iter().zip(velocities.iter()).enumerate() {
        let velocity = vel as f64 / 127.0;
        let noise_seed = (note as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(i as u32);
        let mut voice = Voice::note_on(note, velocity, BASE_SR, noise_seed, true);

        let mut voice_buf = vec![0.0f64; n_samples];
        let chunk_size = 1024;
        let mut offset = 0;
        while offset < n_samples {
            let end = (offset + chunk_size).min(n_samples);
            voice.render(&mut voice_buf[offset..end]);
            offset = end;
        }

        // Sum into mix
        for j in 0..n_samples {
            sum_buf[j] += voice_buf[j];
        }

        individual_bufs.push(voice_buf);
        eprintln!("  Voice {}: {} vel={}", i, midi_note_name(note), vel);
    }

    // Process through oversampled preamp (shared — this is where intermod happens)
    eprint!("Processing through preamp...");
    let mut preamp = create_preamp(args);
    preamp.set_ldr_resistance(r_ldr);
    preamp.reset();
    let mut os = Oversampler::new();

    let mut preamp_output = vec![0.0f64; n_samples];
    for i in 0..n_samples {
        let mut up = [0.0f64; 2];
        os.upsample_2x(&[sum_buf[i]], &mut up);
        let processed = [preamp.process_sample(up[0]), preamp.process_sample(up[1])];
        let mut down = [0.0f64; 1];
        os.downsample_2x(&processed, &mut down);
        preamp_output[i] = down[0];
    }
    eprintln!(" done");

    // Output stage: volume → power amp → speaker
    let mut power_amp = PowerAmp::new();
    let mut speaker = Speaker::new(BASE_SR);
    speaker.set_character(speaker_char);

    let mut final_output = vec![0.0f64; n_samples];
    for i in 0..n_samples {
        let attenuated = preamp_output[i] * volume * volume; // audio taper
        let amplified = if no_poweramp {
            attenuated
        } else {
            power_amp.process(attenuated)
        };
        final_output[i] = speaker.process(amplified);
    }

    // Also render each voice through its OWN separate chain for comparison
    let mut separate_sum = vec![0.0f64; n_samples];
    for voice_buf in &individual_bufs {
        let mut sep_preamp = create_preamp(args);
        sep_preamp.set_ldr_resistance(r_ldr);
        sep_preamp.reset();
        let mut sep_os = Oversampler::new();
        let mut sep_pa = PowerAmp::new();
        let mut sep_spk = Speaker::new(BASE_SR);
        sep_spk.set_character(speaker_char);

        for i in 0..n_samples {
            let mut up = [0.0f64; 2];
            sep_os.upsample_2x(&[voice_buf[i]], &mut up);
            let processed = [
                sep_preamp.process_sample(up[0]),
                sep_preamp.process_sample(up[1]),
            ];
            let mut down = [0.0f64; 1];
            sep_os.downsample_2x(&processed, &mut down);
            let attenuated = down[0] * volume * volume;
            let amplified = if no_poweramp {
                attenuated
            } else {
                sep_pa.process(attenuated)
            };
            separate_sum[i] += sep_spk.process(amplified);
        }
    }

    // Compute the intermod residual: shared_chain - sum_of_separate
    let mut residual = vec![0.0f64; n_samples];
    for i in 0..n_samples {
        residual[i] = final_output[i] - separate_sum[i];
    }

    // Measurements
    let measure_start = (0.2 * BASE_SR) as usize;
    let measure_end = (2.0 * BASE_SR).min(n_samples as f64) as usize;
    let window = &final_output[measure_start..measure_end];
    let sep_window = &separate_sum[measure_start..measure_end];
    let res_window = &residual[measure_start..measure_end];

    let poly_peak = peak_db(window);
    let sep_peak = peak_db(sep_window);
    let res_peak = peak_db(res_window);
    let poly_rms = rms_db(window);
    let sep_rms = rms_db(sep_window);
    let res_rms = rms_db(res_window);

    // Peak and write WAV files
    let peak = final_output.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
    let peak_dbfs = if peak > 0.0 {
        20.0 * peak.log10()
    } else {
        -120.0
    };

    let scale = if normalize {
        if peak > 0.7 { 0.7 / peak } else { 1.0 }
    } else {
        1.0
    };

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: BASE_SR as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let max_val = (1 << 23) - 1;

    // Write main poly output
    {
        let mut writer = hound::WavWriter::create(output_path, spec).expect("Failed to create WAV");
        for sample in &final_output {
            let scaled = (sample * scale * max_val as f64).round() as i32;
            writer
                .write_sample(scaled.clamp(-max_val, max_val))
                .unwrap();
        }
        writer.finalize().unwrap();
    }

    // Write residual
    let residual_path = output_path.replace(".wav", "_residual.wav");
    {
        let res_peak_abs = residual.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        let res_scale = if res_peak_abs > 1e-10 {
            0.5 / res_peak_abs
        } else {
            1.0
        };
        let mut writer =
            hound::WavWriter::create(&residual_path, spec).expect("Failed to create residual WAV");
        for sample in &residual {
            let scaled = (sample * res_scale * max_val as f64).round() as i32;
            writer
                .write_sample(scaled.clamp(-max_val, max_val))
                .unwrap();
        }
        writer.finalize().unwrap();
    }

    // Report
    println!("Polyphonic render complete");
    println!(
        "  Notes:     {:?}",
        notes
            .iter()
            .map(|&n| format!("{} ({})", midi_note_name(n), n))
            .collect::<Vec<_>>()
    );
    println!("  Velocities: {:?}", velocities);
    println!("  Duration:  {duration:.1}s");
    println!(
        "  Volume:    {volume:.3} (audio taper: {:.3})",
        volume * volume
    );
    println!("  Speaker:   {speaker_char:.1}");
    println!("  Peak:      {peak_dbfs:.1} dBFS");
    println!();
    println!("  === INTERMOD ANALYSIS (0.2-2.0s window) ===");
    println!("  Shared chain (poly):  peak={poly_peak:.1} dBFS  rms={poly_rms:.1} dBFS");
    println!("  Separate chains (sum): peak={sep_peak:.1} dBFS  rms={sep_rms:.1} dBFS");
    println!("  Residual (intermod):  peak={res_peak:.1} dBFS  rms={res_rms:.1} dBFS");
    println!(
        "  Intermod ratio:       {:.1} dB below signal",
        poly_rms - res_rms
    );
    println!();

    let verdict = if (poly_rms - res_rms) > 60.0 {
        "CLEAN — intermod negligible"
    } else if (poly_rms - res_rms) > 40.0 {
        "OK — intermod present but likely inaudible"
    } else if (poly_rms - res_rms) > 20.0 {
        "MARGINAL — intermod may be audible on revealing systems"
    } else {
        "DIRTY — intermod clearly audible"
    };
    println!("  Verdict: {verdict}");
    println!();
    println!("  Output:    {output_path}");
    println!("  Residual:  {residual_path} (normalized for listening)");
}

// ─── MIDI file render ─────────────────────────────────────────────────────

/// Render a MIDI file through the full polyphonic signal chain.
///
/// Replicates the plugin's exact voice management and signal routing:
///   voices (reed → pickup) → sum → oversampled preamp → volume → power amp → speaker
///
/// Usage:
///   preamp-bench render-midi --midi path/to/file.mid --output /tmp/output.wav
fn cmd_render_midi(args: &[String]) {
    let midi_path = parse_flag_str(args, "--midi", "");
    if midi_path.is_empty() {
        eprintln!("Usage: preamp-bench render-midi --midi <file.mid> [--output <file.wav>]");
        eprintln!("  --volume V       Volume (default 0.60)");
        eprintln!("  --speaker S      Speaker character 0-1 (default 1.0)");
        eprintln!("  --no-poweramp    Bypass power amp");
        eprintln!("  --tail T         Extra seconds after last note (default 2.0)");
        return;
    }
    let output_path = parse_flag_str(args, "--output", "/tmp/preamp_render_midi.wav");
    let volume = parse_flag(args, "--volume", 0.60);
    let speaker_char = parse_flag(args, "--speaker", 1.0);
    let no_poweramp = args.contains(&"--no-poweramp".to_string());
    let tail_seconds = parse_flag(args, "--tail", 2.0);

    // Parse MIDI file
    let midi_data = std::fs::read(midi_path).expect("Failed to read MIDI file");
    let smf = midly::Smf::parse(&midi_data).expect("Failed to parse MIDI file");

    let ticks_per_beat = match smf.header.timing {
        midly::Timing::Metrical(tpb) => tpb.as_int() as f64,
        _ => {
            eprintln!("Only metrical (ticks per beat) MIDI timing is supported");
            return;
        }
    };

    // Collect all MIDI events with absolute time in seconds
    #[derive(Clone)]
    enum MidiEvt {
        NoteOn { note: u8, velocity: u8 },
        NoteOff { note: u8 },
        Pedal { on: bool },
    }

    struct TimedEvent {
        time_s: f64,
        evt: MidiEvt,
    }

    let mut events: Vec<TimedEvent> = Vec::new();

    for track in &smf.tracks {
        let mut tempo: f64 = 500_000.0; // default 120 BPM
        let mut time_s: f64 = 0.0;

        for event in track {
            let delta_ticks = event.delta.as_int() as u64;
            let delta_s = (delta_ticks as f64 / ticks_per_beat) * (tempo / 1_000_000.0);
            time_s += delta_s;

            match event.kind {
                midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(t)) => {
                    tempo = t.as_int() as f64;
                }
                midly::TrackEventKind::Midi { message, .. } => match message {
                    midly::MidiMessage::NoteOn { key, vel } => {
                        let v = vel.as_int();
                        if v == 0 {
                            events.push(TimedEvent {
                                time_s,
                                evt: MidiEvt::NoteOff { note: key.as_int() },
                            });
                        } else {
                            events.push(TimedEvent {
                                time_s,
                                evt: MidiEvt::NoteOn {
                                    note: key.as_int(),
                                    velocity: v,
                                },
                            });
                        }
                    }
                    midly::MidiMessage::NoteOff { key, .. } => {
                        events.push(TimedEvent {
                            time_s,
                            evt: MidiEvt::NoteOff { note: key.as_int() },
                        });
                    }
                    midly::MidiMessage::Controller { controller, value } => {
                        if controller.as_int() == 64 {
                            events.push(TimedEvent {
                                time_s,
                                evt: MidiEvt::Pedal {
                                    on: value.as_int() >= 64,
                                },
                            });
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    // Sort by time
    events.sort_by(|a, b| a.time_s.partial_cmp(&b.time_s).unwrap());

    if events.is_empty() {
        eprintln!("No note events found in MIDI file");
        return;
    }

    let last_event_time = events.last().unwrap().time_s;
    let total_duration = last_event_time + tail_seconds;
    let total_samples = (total_duration * BASE_SR) as usize;

    eprintln!(
        "MIDI: {} events, {:.1}s + {:.1}s tail = {:.1}s total ({} samples)",
        events.len(),
        last_event_time,
        tail_seconds,
        total_duration,
        total_samples
    );

    // Voice management (mirrors plugin architecture)
    const MAX_VOICES: usize = 12;

    struct VoiceSlot {
        voice: Option<Voice>,
        active: bool,
        midi_note: u8,
        age: u64,
    }

    let mut voices: Vec<VoiceSlot> = (0..MAX_VOICES)
        .map(|_| VoiceSlot {
            voice: None,
            active: false,
            midi_note: 0,
            age: 0,
        })
        .collect();
    let mut age_counter: u64 = 0;

    let mut preamp = create_preamp(args);
    preamp.set_ldr_resistance(1_000_000.0);
    preamp.reset();
    let mut os = Oversampler::new();
    let mut power_amp = PowerAmp::new();
    let mut speaker = Speaker::new(BASE_SR);
    speaker.set_character(speaker_char);

    let mut output = vec![0.0f64; total_samples];
    let mut event_idx = 0;

    let chunk_size = 64; // process in small chunks for sample-accurate events
    let mut voice_buf = vec![0.0f64; chunk_size];
    let mut sum_buf = vec![0.0f64; chunk_size];
    let mut up_buf = vec![0.0f64; chunk_size * 2];

    let mut sample_pos: usize = 0;
    let mut peak_polyphony: usize = 0;
    let mut note_on_count: usize = 0;
    let mut pedal_down = false;
    // Notes waiting for pedal release to send note_off
    let mut pedal_held: Vec<u8> = Vec::new();

    while sample_pos < total_samples {
        let chunk_end = (sample_pos + chunk_size).min(total_samples);
        let len = chunk_end - sample_pos;
        let chunk_time = sample_pos as f64 / BASE_SR;

        // Process MIDI events at or before this chunk
        while event_idx < events.len() && events[event_idx].time_s <= chunk_time {
            let evt = events[event_idx].evt.clone();
            match evt {
                MidiEvt::NoteOn { note, velocity } => {
                    let note = note.clamp(tables::MIDI_LO, tables::MIDI_HI);
                    let vel = velocity as f64 / 127.0;
                    age_counter += 1;
                    note_on_count += 1;

                    let slot_idx = voices.iter().position(|s| !s.active).unwrap_or_else(|| {
                        voices
                            .iter()
                            .enumerate()
                            .min_by_key(|(_, s)| s.age)
                            .map(|(i, _)| i)
                            .unwrap_or(0)
                    });

                    let seed = (note as u32)
                        .wrapping_mul(2654435761)
                        .wrapping_add(age_counter as u32);
                    voices[slot_idx] = VoiceSlot {
                        voice: Some(Voice::note_on(note, vel, BASE_SR, seed, true)),
                        active: true,
                        midi_note: note,
                        age: age_counter,
                    };
                    let active = voices.iter().filter(|s| s.active).count();
                    peak_polyphony = peak_polyphony.max(active);
                }
                MidiEvt::NoteOff { note } => {
                    let note = note.clamp(tables::MIDI_LO, tables::MIDI_HI);
                    if pedal_down {
                        // Defer note-off until pedal release
                        pedal_held.push(note);
                    } else {
                        // Immediate note-off
                        if let Some(slot) = voices
                            .iter_mut()
                            .filter(|s| s.active && s.midi_note == note)
                            .min_by_key(|s| s.age)
                            && let Some(ref mut v) = slot.voice
                        {
                            v.note_off();
                        }
                    }
                }
                MidiEvt::Pedal { on } => {
                    if on {
                        pedal_down = true;
                    } else {
                        pedal_down = false;
                        // Release all pedal-held notes
                        for held_note in pedal_held.drain(..) {
                            if let Some(slot) = voices
                                .iter_mut()
                                .filter(|s| s.active && s.midi_note == held_note)
                                .min_by_key(|s| s.age)
                                && let Some(ref mut v) = slot.voice
                            {
                                v.note_off();
                            }
                        }
                    }
                }
            }
            event_idx += 1;
        }

        // Clean up silent voices
        for slot in &mut voices {
            if slot.active
                && let Some(ref v) = slot.voice
                && v.is_silent()
            {
                slot.active = false;
                slot.voice = None;
            }
        }

        // Render all active voices → sum
        sum_buf[..len].fill(0.0);
        for slot in &mut voices {
            if !slot.active {
                continue;
            }
            if let Some(ref mut voice) = slot.voice {
                voice_buf[..len].fill(0.0);
                voice.render(&mut voice_buf[..len]);
                for i in 0..len {
                    sum_buf[i] += voice_buf[i];
                }
            }
        }

        // Oversampled preamp
        os.upsample_2x(&sum_buf[..len], &mut up_buf[..len * 2]);
        for s in &mut up_buf[..len * 2] {
            *s = preamp.process_sample(*s);
        }
        let mut down_buf = vec![0.0f64; len];
        os.downsample_2x(&up_buf[..len * 2], &mut down_buf);

        // Output stage: volume → power amp → speaker
        for i in 0..len {
            let attenuated = down_buf[i] * volume * volume;
            let amplified = if no_poweramp {
                attenuated
            } else {
                power_amp.process(attenuated)
            };
            output[sample_pos + i] = speaker.process(amplified);
        }

        sample_pos = chunk_end;
    }

    // Peak measurement
    let peak = output.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
    let peak_dbfs = if peak > 0.0 {
        20.0 * peak.log10()
    } else {
        -120.0
    };

    if peak > 1.0 {
        eprintln!(
            "WARNING: Peak exceeds 0 dBFS ({peak_dbfs:.1} dBFS) — consider reducing --volume"
        );
    }

    // Write WAV
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: BASE_SR as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let max_val = (1 << 23) - 1;
    let mut writer =
        hound::WavWriter::create(output_path, spec).expect("Failed to create WAV file");
    for sample in &output {
        let scaled = (sample * max_val as f64).round() as i32;
        writer
            .write_sample(scaled.clamp(-max_val, max_val))
            .unwrap();
    }
    writer.finalize().unwrap();

    println!("MIDI render complete");
    println!("  File:      {midi_path}");
    println!("  Notes:     {note_on_count} note-ons");
    println!("  Peak poly: {peak_polyphony} voices");
    println!("  Duration:  {total_duration:.1}s");
    println!(
        "  Volume:    {volume:.3} (audio taper: {:.3})",
        volume * volume
    );
    println!("  Speaker:   {speaker_char:.1}");
    if no_poweramp {
        println!("  Power amp: BYPASSED");
    }
    println!("  Peak:      {peak_dbfs:.1} dBFS");
    println!("  Output:    {output_path}");
}

// ─── Centroid tracking ─────────────────────────────────────────────────────

/// Compute spectral centroid of a signal via brute-force DFT.
///
/// Sums frequency-weighted power across bins from min_freq to max_freq.
/// Returns centroid in Hz, or 0.0 if no energy is detected.
fn spectral_centroid(signal: &[f64], sr: f64, min_freq: f64, max_freq: f64) -> f64 {
    let n = signal.len();
    let freq_resolution = sr / n as f64;
    let k_min = (min_freq / freq_resolution).ceil() as usize;
    let k_max = ((max_freq / freq_resolution).floor() as usize).min(n / 2);

    let mut weighted_sum = 0.0;
    let mut power_sum = 0.0;
    for k in k_min..=k_max {
        let freq = k as f64 * freq_resolution;
        // DFT at this bin
        let mut re = 0.0;
        let mut im = 0.0;
        for (i, &s) in signal.iter().enumerate() {
            let phase = 2.0 * PI * k as f64 * i as f64 / n as f64;
            re += s * phase.cos();
            im -= s * phase.sin();
        }
        let mag_sq = re * re + im * im;
        weighted_sum += freq * mag_sq;
        power_sum += mag_sq;
    }
    if power_sum > 0.0 {
        weighted_sum / power_sum
    } else {
        0.0
    }
}

fn cmd_centroid_track(args: &[String]) {
    let note = parse_flag(args, "--note", 60.0) as u8;
    let velocity = parse_flag(args, "--velocity", 100.0) as u8;
    let duration = parse_flag(args, "--duration", 1.0);
    let window_ms = parse_flag(args, "--window-ms", 5.0);
    let hop_ms = parse_flag(args, "--hop-ms", 2.5);
    let end_ms = parse_flag(args, "--end-ms", 500.0);
    let r_ldr = parse_flag(args, "--ldr", 1_000_000.0);
    let volume = parse_flag(args, "--volume", 0.60);
    let speaker_char = parse_flag(args, "--speaker", 1.0);
    let no_poweramp = args.contains(&"--no-poweramp".to_string());
    let no_preamp = args.contains(&"--no-preamp".to_string());
    let csv_path = parse_flag_str(args, "--csv", "");
    let disp_scale: Option<f64> = if args.contains(&"--displacement-scale".to_string()) {
        Some(parse_flag(args, "--displacement-scale", 0.30))
    } else {
        None
    };

    // Render full signal chain (reuse render infrastructure)
    let reed_output =
        Voice::render_note_with_scale(note, velocity as f64 / 127.0, duration, BASE_SR, disp_scale);

    let n_samples = reed_output.len();

    // Preamp stage
    let preamp_output = if no_preamp {
        reed_output.clone()
    } else {
        let mut preamp = create_preamp(args);
        preamp.set_ldr_resistance(r_ldr);
        preamp.reset();
        let mut os = Oversampler::new();
        let mut out = vec![0.0f64; n_samples];
        for i in 0..n_samples {
            let mut up = [0.0f64; 2];
            os.upsample_2x(&[reed_output[i]], &mut up);
            let processed = [preamp.process_sample(up[0]), preamp.process_sample(up[1])];
            let mut down = [0.0f64; 1];
            os.downsample_2x(&processed, &mut down);
            out[i] = down[0];
        }
        out
    };

    // Output stage
    let mut power_amp = PowerAmp::new();
    let mut speaker = Speaker::new(BASE_SR);
    speaker.set_character(speaker_char);

    let mut final_output = vec![0.0f64; n_samples];
    for i in 0..n_samples {
        let attenuated = preamp_output[i] * volume;
        let amplified = if no_poweramp {
            attenuated
        } else {
            power_amp.process(attenuated)
        };
        final_output[i] = speaker.process(amplified);
    }

    // Centroid tracking with Hann-windowed frames
    let window_samples = ((window_ms / 1000.0) * BASE_SR) as usize;
    let hop_samples = ((hop_ms / 1000.0) * BASE_SR) as usize;
    let end_sample = ((end_ms / 1000.0) * BASE_SR) as usize;

    // Frequency range for centroid: 50 Hz to Nyquist/2
    let min_freq = 50.0;
    let max_freq = BASE_SR / 4.0;

    // Pre-compute Hann window
    let hann: Vec<f64> = (0..window_samples)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / window_samples as f64).cos()))
        .collect();

    let note_name = midi_note_name(note);
    println!(
        "Centroid tracking: {} (MIDI {}) vel={}, {}ms Hann windows",
        note_name, note, velocity, window_ms
    );
    if no_preamp {
        println!("  Preamp: BYPASSED");
    }
    if no_poweramp {
        println!("  Power amp: BYPASSED");
    }
    println!();
    println!("  {:>10}  {:>14}", "Time (ms)", "Centroid (Hz)");

    let mut csv_lines = Vec::new();
    csv_lines.push("time_ms,centroid_hz".to_string());

    let mut centroid_at_10ms = None;
    let mut centroid_at_300ms = None;

    let mut pos = 0usize;
    while pos + window_samples <= final_output.len() && pos + window_samples / 2 <= end_sample {
        let center_ms = (pos as f64 + window_samples as f64 / 2.0) / BASE_SR * 1000.0;

        // Apply Hann window
        let windowed: Vec<f64> = final_output[pos..pos + window_samples]
            .iter()
            .zip(hann.iter())
            .map(|(&s, &w)| s * w)
            .collect();

        let centroid = spectral_centroid(&windowed, BASE_SR, min_freq, max_freq);

        if centroid > 0.0 {
            println!("  {:>10.1}  {:>14.0}", center_ms, centroid);
            csv_lines.push(format!("{:.1},{:.1}", center_ms, centroid));
        }

        // Track key timepoints
        if centroid_at_10ms.is_none() && center_ms >= 10.0 {
            centroid_at_10ms = Some(centroid);
        }
        if centroid_at_300ms.is_none() && center_ms >= 300.0 {
            centroid_at_300ms = Some(centroid);
        }

        pos += hop_samples;
    }

    // Summary
    println!();

    // Calibration targets by register
    let midi = note;
    let (attack_lo, attack_hi, sustain_lo, sustain_hi, drift_lo, drift_hi) = if midi <= 48 {
        // Bass
        (600.0, 1000.0, 500.0, 800.0, -200.0, -50.0)
    } else if midi <= 72 {
        // Mid
        (600.0, 1200.0, 600.0, 1000.0, -240.0, -30.0)
    } else {
        // Treble
        (800.0, 1600.0, 800.0, 1400.0, -250.0, -30.0)
    };

    if let Some(c10) = centroid_at_10ms {
        let status = if c10 >= attack_lo && c10 <= attack_hi {
            "OK"
        } else {
            "MISS"
        };
        println!(
            "  Attack centroid (10ms):   {:>6.0} Hz   (target: {:.0}-{:.0})  {}",
            c10, attack_lo, attack_hi, status
        );
    } else {
        println!("  Attack centroid (10ms):   (no data — signal too short or silent)");
    }

    if let Some(c300) = centroid_at_300ms {
        let status = if c300 >= sustain_lo && c300 <= sustain_hi {
            "OK"
        } else {
            "MISS"
        };
        println!(
            "  Sustain centroid (300ms): {:>6.0} Hz   (target: {:.0}-{:.0})  {}",
            c300, sustain_lo, sustain_hi, status
        );
    } else {
        println!("  Sustain centroid (300ms): (no data — signal too short)");
    }

    if let (Some(c10), Some(c300)) = (centroid_at_10ms, centroid_at_300ms) {
        let drift = c300 - c10;
        let status = if drift >= drift_lo && drift <= drift_hi {
            "OK"
        } else {
            "MISS"
        };
        println!(
            "  Drift:                   {:>+6.0} Hz   (target: {:.0} to {:.0}) {}",
            drift, drift_lo, drift_hi, status
        );
    }

    if !csv_path.is_empty() {
        std::fs::write(csv_path, csv_lines.join("\n") + "\n").expect("Failed to write CSV");
        println!("\n  CSV written to {csv_path}");
    }
}
