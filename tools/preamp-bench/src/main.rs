/// Preamp Bench — Wurlitzer 200A preamp DSP validation CLI.
///
/// Measures preamp characteristics and compares against SPICE targets.
///
/// Usage:
///   preamp-bench gain [--freq F] [--amplitude A]
///   preamp-bench sweep [--start F1] [--end F2] [--points N] [--csv FILE]
///   preamp-bench harmonics [--freq F] [--amplitude A]
///   preamp-bench tremolo-sweep [--ldr-min R1] [--ldr-max R2] [--steps N] [--csv FILE]
///   preamp-bench render [--note N] [--velocity V] [--duration D] [--output FILE]

use std::f64::consts::PI;

use openwurli_dsp::filters::OnePoleHpf;
use openwurli_dsp::oversampler::Oversampler;
use openwurli_dsp::power_amp::PowerAmp;
use openwurli_dsp::preamp::{EbersMollPreamp, PreampModel};
use openwurli_dsp::reed::ModalReed;
use openwurli_dsp::hammer::dwell_attenuation;
use openwurli_dsp::speaker::Speaker;
use openwurli_dsp::tables::{self, NUM_MODES};
use openwurli_dsp::variation;
use openwurli_dsp::voice::Voice;

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

// ─── Gain measurement ───────────────────────────────────────────────────────

/// Measure preamp gain by running a sine wave through the 2x-oversampled preamp.
fn measure_gain_at(preamp: &mut EbersMollPreamp, freq: f64, amplitude: f64) -> f64 {
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

    let mut preamp = EbersMollPreamp::new(OVERSAMPLED_SR);
    preamp.set_ldr_resistance(r_ldr);

    let gain = measure_gain_at(&mut preamp, freq, amplitude);
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

    let mut preamp = EbersMollPreamp::new(OVERSAMPLED_SR);
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

        let gain = measure_gain_at(&mut preamp, freq, amplitude);
        let gain_db = 20.0 * gain.log10();

        println!("{freq:>10.1}  {gain_db:>10.2}");
        csv_lines.push(format!("{freq:.1},{gain_db:.2}"));
    }

    if !csv_path.is_empty() {
        std::fs::write(csv_path, csv_lines.join("\n") + "\n")
            .expect("Failed to write CSV");
        println!("\nCSV written to {csv_path}");
    }
}

// ─── Harmonic analysis ──────────────────────────────────────────────────────

fn cmd_harmonics(args: &[String]) {
    let freq = parse_flag(args, "--freq", 440.0);
    let amplitude = parse_flag(args, "--amplitude", 0.005);
    let r_ldr = parse_flag(args, "--ldr", 1_000_000.0);

    let mut preamp = EbersMollPreamp::new(OVERSAMPLED_SR);
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
    println!("  H2:          {h2:.6}  ({:.1} dB rel)", 20.0 * (h2 / h1).log10());
    println!("  H3:          {h3:.6}  ({:.1} dB rel)", 20.0 * (h3 / h1).log10());
    println!("  H4:          {h4:.6}  ({:.1} dB rel)", 20.0 * (h4 / h1).log10());
    println!("  H5:          {h5:.6}  ({:.1} dB rel)", 20.0 * (h5 / h1).log10());
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

    let mut preamp = EbersMollPreamp::new(OVERSAMPLED_SR);

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
        let gain = measure_gain_at(&mut preamp, freq, amplitude);
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
        std::fs::write(csv_path, csv_lines.join("\n") + "\n")
            .expect("Failed to write CSV");
        println!("\nCSV written to {csv_path}");
    }
}

// ─── Render (reed -> preamp -> WAV) ─────────────────────────────────────────

fn cmd_render(args: &[String]) {
    let note = parse_flag(args, "--note", 60.0) as u8;
    let velocity = parse_flag(args, "--velocity", 100.0) as u8;
    let duration = parse_flag(args, "--duration", 2.0);
    let r_ldr = parse_flag(args, "--ldr", 1_000_000.0);
    let preamp_gain = parse_flag(args, "--gain", 40.0);
    let volume = parse_flag(args, "--volume", 0.05);
    let speaker_char = parse_flag(args, "--speaker", 1.0);
    let output_path = parse_flag_str(args, "--output", "/tmp/preamp_render.wav");

    // Render reed voice (reed → pickup with nonlinearity + HPF)
    let reed_output = Voice::render_note(note, velocity as f64 / 127.0, duration, BASE_SR);

    // Process through oversampled preamp
    let mut preamp = EbersMollPreamp::new(OVERSAMPLED_SR);
    preamp.set_ldr_resistance(r_ldr);
    let mut os = Oversampler::new();

    let n_samples = reed_output.len();
    let mut preamp_output = vec![0.0f64; n_samples];
    for i in 0..n_samples {
        let mut up = [0.0f64; 2];
        os.upsample_2x(&[reed_output[i]], &mut up);
        let processed = [preamp.process_sample(up[0]), preamp.process_sample(up[1])];
        let mut down = [0.0f64; 1];
        os.downsample_2x(&processed, &mut down);
        preamp_output[i] = down[0];
    }

    // Output stage: gain → volume → power amp → speaker
    // Matches the plugin signal chain in lib.rs
    let mut power_amp = PowerAmp::new();
    let mut speaker = Speaker::new(BASE_SR);
    speaker.set_character(speaker_char);

    let mut final_output = vec![0.0f64; n_samples];
    for i in 0..n_samples {
        let attenuated = preamp_output[i] * preamp_gain * volume;
        let amplified = power_amp.process(attenuated);
        final_output[i] = speaker.process(amplified);
    }

    // Normalize and write WAV
    let peak = final_output.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
    let peak_dbfs = if peak > 0.0 { 20.0 * peak.log10() } else { -120.0 };

    // Normalize to -3 dBFS if needed
    let scale = if peak > 0.7 { 0.7 / peak } else { 1.0 };

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: BASE_SR as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(output_path, spec)
        .expect("Failed to create WAV file");

    let max_val = (1 << 23) - 1;
    for sample in &final_output {
        let scaled = (sample * scale * max_val as f64).round() as i32;
        writer.write_sample(scaled.clamp(-max_val, max_val)).unwrap();
    }
    writer.finalize().unwrap();

    println!("Render complete");
    println!("  Note:      MIDI {note}");
    println!("  Velocity:  {velocity}");
    println!("  Duration:  {duration:.1}s");
    println!("  LDR:       {r_ldr:.0} Ω");
    println!("  Gain:      {preamp_gain:.1}x, Volume: {volume:.3}");
    println!("  Speaker:   {speaker_char:.1}");
    println!("  Peak:      {peak_dbfs:.1} dBFS (raw)");
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
    const DISPLACEMENT_SCALE: f64 = 0.30;
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
    println!("{:>6} {:>4}  {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Note", "Vel",
        "Reed pk", "y_peak",
        "NL H2/H1", "NL pk(mV)",
        "HPF H2/H1", "HPF pk(mV)",
        "Pre H2/H1");
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
            let vel_scale = velocity.powf(vel_exp);
            let out_scale = tables::output_scale(note);

            let mut amplitudes = [0.0f64; NUM_MODES];
            for i in 0..NUM_MODES {
                amplitudes[i] = params.mode_amplitudes[i] * dwell[i] * amp_offsets[i]
                    * vel_scale * out_scale;
            }

            let mut reed = ModalReed::new(
                detuned, &params.mode_ratios, &amplitudes,
                &params.mode_decay_rates, BASE_SR,
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
            let mut nl_buf = reed_buf.clone();
            for s in &mut nl_buf {
                let y = (*s * DISPLACEMENT_SCALE).clamp(-MAX_Y, MAX_Y);
                *s = (y / (1.0 - y)) * SENSITIVITY;
            }
            let nl_steady = &nl_buf[measure_offset..];
            let nl_peak = nl_steady.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
            let nl_h1 = dft_magnitude(nl_steady, freq, BASE_SR);
            let nl_h2 = dft_magnitude(nl_steady, h2_freq, BASE_SR);
            let nl_h2_h1 = if nl_h1 > 1e-15 { nl_h2 / nl_h1 } else { 0.0 };
            let y_peak = reed_peak * DISPLACEMENT_SCALE;

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
            let mut preamp = EbersMollPreamp::new(OVERSAMPLED_SR);
            preamp.set_ldr_resistance(1_000_000.0);
            let mut os = Oversampler::new();

            let mut preamp_buf = vec![0.0f64; n_samples];
            for i in 0..n_samples {
                let mut up = [0.0f64; 2];
                os.upsample_2x(&[hpf_buf[i]], &mut up);
                let processed = [
                    preamp.process_sample(up[0]),
                    preamp.process_sample(up[1]),
                ];
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
                note_name, vel_byte,
                reed_peak, y_peak,
                nl_h2_h1 * 100.0, nl_peak * 1000.0,
                hpf_h2_h1 * 100.0, hpf_peak * 1000.0,
                pre_h2_h1 * 100.0,
            );
        }
        println!();
    }

    // Summary
    println!("Legend:");
    println!("  Reed pk   = peak reed displacement (model units)");
    println!("  y_peak    = physical displacement fraction (y = reed_pk * {DISPLACEMENT_SCALE})");
    println!("  NL H2/H1  = H2/H1 after 1/(1-y) nonlinearity, before HPF");
    println!("  NL pk(mV) = peak signal after nonlinearity (millivolts)");
    println!("  HPF H2/H1 = H2/H1 after pickup RC HPF at {PICKUP_HPF_HZ} Hz");
    println!("  HPF pk(mV)= peak signal after HPF (millivolts, feeds preamp)");
    println!("  Pre H2/H1 = H2/H1 after preamp (2x gain, no tremolo)");
    println!();
    println!("SPICE targets: y=0.10 → NL H2/H1 = 8.7%, HPF boosts H2 ~1.9x relative to H1");
}

fn midi_note_name(note: u8) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let name = NAMES[(note % 12) as usize];
    let octave = (note as i32 / 12) - 1;
    format!("{name}{octave}")
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
