/// Reed Renderer — Wurlitzer 200A modal synthesis WAV renderer.
///
/// Standalone CLI tool for rendering reed tones to WAV files.
/// Uses physics-derived parameters from docs/reed-and-hammer-physics.md.

use openwurli_dsp::tables;
use openwurli_dsp::voice::Voice;

const SAMPLE_RATE: f64 = 44100.0;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut notes: Vec<u8> = Vec::new();
    let mut velocities: Vec<u8> = Vec::new();
    let mut duration: f64 = 2.0;
    let mut output_dir = String::from(".");
    let mut output_file: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--note" | "-n" => {
                i += 1;
                for s in args[i].split(',') {
                    notes.push(s.trim().parse().expect("invalid MIDI note"));
                }
            }
            "--velocity" | "-v" => {
                i += 1;
                for s in args[i].split(',') {
                    velocities.push(s.trim().parse().expect("invalid velocity"));
                }
            }
            "--duration" | "-d" => {
                i += 1;
                duration = args[i].parse().expect("invalid duration");
            }
            "--output" | "-o" => {
                i += 1;
                output_file = Some(args[i].clone());
            }
            "--output-dir" => {
                i += 1;
                output_dir = args[i].clone();
            }
            "--sweep" => {
                notes = vec![33, 45, 57, 60, 69, 72, 81, 93, 96];
            }
            "--help" | "-h" => {
                print_usage();
                return;
            }
            other => {
                eprintln!("Unknown argument: {other}");
                print_usage();
                std::process::exit(1);
            }
        }
        i += 1;
    }

    if notes.is_empty() {
        notes.push(60);
    }
    if velocities.is_empty() {
        velocities.push(100);
    }

    for &n in &notes {
        if n < tables::MIDI_LO || n > tables::MIDI_HI {
            eprintln!("MIDI note {n} out of range ({}-{})", tables::MIDI_LO, tables::MIDI_HI);
            std::process::exit(1);
        }
    }

    for &midi_note in &notes {
        for &vel in &velocities {
            let velocity_f = vel as f64 / 127.0;
            let note_name = midi_note_name(midi_note);

            let filename = if let Some(ref f) = output_file {
                if notes.len() == 1 && velocities.len() == 1 {
                    f.clone()
                } else {
                    format!("{output_dir}/reed_{note_name}_v{vel}.wav")
                }
            } else {
                format!("{output_dir}/reed_{note_name}_v{vel}.wav")
            };

            eprintln!(
                "Rendering MIDI {midi_note} ({note_name}) vel={vel} dur={duration}s → {filename}"
            );

            let samples = Voice::render_note(midi_note, velocity_f, duration, SAMPLE_RATE);

            let peak = samples.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
            eprintln!("  Peak amplitude: {peak:.6} ({:.1} dBFS)", 20.0 * peak.log10());

            write_wav(&filename, &samples, SAMPLE_RATE as u32);
            eprintln!("  Written: {filename}");
        }
    }
}

fn write_wav(path: &str, samples: &[f64], sample_rate: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("failed to create WAV file");
    let scale = (1 << 23) as f64 - 1.0;
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        writer
            .write_sample((clamped * scale) as i32)
            .expect("failed to write sample");
    }
    writer.finalize().expect("failed to finalize WAV");
}

fn midi_note_name(midi: u8) -> String {
    let names = ["C", "Cs", "D", "Ds", "E", "F", "Fs", "G", "Gs", "A", "As", "B"];
    let octave = (midi / 12) as i32 - 1;
    let note = (midi % 12) as usize;
    format!("{}{}", names[note], octave)
}

fn print_usage() {
    eprintln!(
        r#"Reed Renderer — Wurlitzer 200A modal synthesis WAV renderer

USAGE:
    reed-renderer [OPTIONS]

OPTIONS:
    -n, --note <MIDI[,MIDI,...]>     MIDI note(s) to render (33-96, default: 60)
    -v, --velocity <VEL[,VEL,...]>   Velocity(ies) to render (1-127, default: 100)
    -d, --duration <SECS>            Duration in seconds (default: 2.0)
    -o, --output <PATH>              Output WAV file (single note only)
        --output-dir <DIR>           Output directory for batch mode (default: .)
        --sweep                      Render notes across full keyboard range
    -h, --help                       Print this help

EXAMPLES:
    reed-renderer -n 60 -v 100 -d 2.0 -o middle_c.wav
    reed-renderer -n 60 -v 30,64,100,127            # velocity sweep
    reed-renderer --sweep -v 100 -d 3.0              # keyboard sweep
    reed-renderer -n 33,45,57,69,81,93 -v 64,127     # batch: notes x velocities"#
    );
}
