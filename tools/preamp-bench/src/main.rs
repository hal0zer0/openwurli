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

use openwurli_dsp::oversampler::Oversampler;
use openwurli_dsp::preamp::{EbersMollPreamp, PreampModel};
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

    // Settle
    for i in 0..n_settle {
        let t = i as f64 / BASE_SR;
        let input = amplitude * (2.0 * PI * freq * t).sin();
        let mut up = [0.0f64; 2];
        os.upsample_2x(&[input], &mut up);
        for s in &up {
            preamp.process_sample(*s);
        }
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
    let output_path = parse_flag_str(args, "--output", "/tmp/preamp_render.wav");

    // Render reed voice
    let reed_output = Voice::render_note(note, velocity as f64 / 127.0, duration, BASE_SR);

    // Scale into preamp's millivolt operating range (same as plugin)
    const PREAMP_INPUT_SCALE: f64 = 0.03;

    // Process through oversampled preamp
    let mut preamp = EbersMollPreamp::new(OVERSAMPLED_SR);
    preamp.set_ldr_resistance(r_ldr);
    let mut os = Oversampler::new();

    let n_samples = reed_output.len();
    let mut final_output = vec![0.0f64; n_samples];
    for i in 0..n_samples {
        let scaled = reed_output[i] * PREAMP_INPUT_SCALE;
        let mut up = [0.0f64; 2];
        os.upsample_2x(&[scaled], &mut up);
        let processed = [preamp.process_sample(up[0]), preamp.process_sample(up[1])];
        let mut down = [0.0f64; 1];
        os.downsample_2x(&processed, &mut down);
        final_output[i] = down[0];
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
    println!("  Peak:      {peak_dbfs:.1} dBFS (raw)");
    println!("  Output:    {output_path}");
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
