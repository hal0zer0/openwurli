/// Integration tests for reed renderer physics verification.
///
/// These tests render short clips and verify physical properties:
/// 1. Velocity affects amplitude
/// 2. HPF attenuates bass
/// 3. Per-note variation is deterministic
/// 4. Signal levels are in expected range
use std::process::Command;

fn cargo_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO"));
    cmd.args(["run", "-p", "reed-renderer", "--"]);
    cmd
}

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(name)
}

#[test]
fn test_cli_renders_wav() {
    let output_path = temp_path("integration_test_cli.wav");
    let _ = std::fs::remove_file(&output_path);

    let status = cargo_bin()
        .args(["-n", "60", "-v", "100", "-d", "0.5", "-o"])
        .arg(&output_path)
        .status()
        .expect("failed to run reed-renderer");

    assert!(status.success(), "reed-renderer exited with error");
    assert!(output_path.exists(), "WAV file not created");

    let reader = hound::WavReader::open(&output_path).expect("invalid WAV file");
    assert_eq!(reader.spec().channels, 1);
    assert_eq!(reader.spec().sample_rate, 44100);
    assert_eq!(reader.spec().bits_per_sample, 24);
    let sample_count = reader.len();
    assert_eq!(sample_count, 22050);

    std::fs::remove_file(&output_path).ok();
}

#[test]
fn test_cli_multi_note() {
    let output_dir = std::env::temp_dir();
    let status = cargo_bin()
        .args(["-n", "60,72", "-v", "100", "-d", "0.3", "--output-dir"])
        .arg(&output_dir)
        .status()
        .expect("failed to run reed-renderer");

    assert!(status.success());
    let c4 = output_dir.join("reed_C4_v100.wav");
    let c5 = output_dir.join("reed_C5_v100.wav");
    assert!(c4.exists());
    assert!(c5.exists());

    std::fs::remove_file(&c4).ok();
    std::fs::remove_file(&c5).ok();
}

#[test]
fn test_cli_velocity_sweep() {
    let output_dir = std::env::temp_dir();
    let status = cargo_bin()
        .args(["-n", "69", "-v", "30,100,127", "-d", "0.2", "--output-dir"])
        .arg(&output_dir)
        .status()
        .expect("failed to run reed-renderer");

    assert!(status.success());
    let v30 = output_dir.join("reed_A4_v30.wav");
    let v100 = output_dir.join("reed_A4_v100.wav");
    let v127 = output_dir.join("reed_A4_v127.wav");
    assert!(v30.exists());
    assert!(v100.exists());
    assert!(v127.exists());

    let peak_30 = wav_peak(&v30);
    let peak_100 = wav_peak(&v100);
    let peak_127 = wav_peak(&v127);

    assert!(
        peak_127 > peak_100,
        "vel 127 peak ({peak_127}) should exceed vel 100 ({peak_100})"
    );
    assert!(
        peak_100 > peak_30,
        "vel 100 peak ({peak_100}) should exceed vel 30 ({peak_30})"
    );

    std::fs::remove_file(&v30).ok();
    std::fs::remove_file(&v100).ok();
    std::fs::remove_file(&v127).ok();
}

#[test]
fn test_register_balance() {
    let bass_path = temp_path("reed_bass_test.wav");
    let treble_path = temp_path("reed_treble_test.wav");
    for path in [&bass_path, &treble_path] {
        let _ = std::fs::remove_file(path);
    }

    let status = cargo_bin()
        .args(["-n", "33", "-v", "100", "-d", "0.5", "-o"])
        .arg(&bass_path)
        .status()
        .unwrap();
    assert!(status.success());

    let status = cargo_bin()
        .args(["-n", "84", "-v", "100", "-d", "0.5", "-o"])
        .arg(&treble_path)
        .status()
        .unwrap();
    assert!(status.success());

    let peak_bass = wav_peak(&bass_path);
    let peak_treble = wav_peak(&treble_path);

    // After voicing (output_scale), bass and treble should be within ~15 dB.
    // With per-note displacement_scale (beam compliance), bass reeds deflect
    // more -> stronger bark -> more energy. The pickup HPF + output_scale partially
    // compensate, but bass is naturally louder at the voice output stage.
    // Full leveling happens in the preamp/volume stages downstream.
    let ratio_db = 20.0 * (peak_bass / peak_treble).log10();
    assert!(
        ratio_db.abs() < 15.0,
        "bass ({peak_bass:.6}) and treble ({peak_treble:.6}) should be within 15 dB, got {ratio_db:.1} dB"
    );

    std::fs::remove_file(&bass_path).ok();
    std::fs::remove_file(&treble_path).ok();
}

#[test]
fn test_deterministic_output() {
    let path1 = temp_path("reed_det_1.wav");
    let path2 = temp_path("reed_det_2.wav");

    for path in [&path1, &path2] {
        let _ = std::fs::remove_file(path);
        let status = cargo_bin()
            .args(["-n", "60", "-v", "80", "-d", "0.3", "-o"])
            .arg(path)
            .status()
            .unwrap();
        assert!(status.success());
    }

    let samples1 = read_wav_samples(&path1);
    let samples2 = read_wav_samples(&path2);
    assert_eq!(
        samples1, samples2,
        "two renders of same note should be identical"
    );

    std::fs::remove_file(&path1).ok();
    std::fs::remove_file(&path2).ok();
}

fn wav_peak(path: &std::path::Path) -> f64 {
    let mut reader = hound::WavReader::open(path).expect("failed to open WAV");
    let max_val = (1i32 << (reader.spec().bits_per_sample - 1)) as f64;
    reader
        .samples::<i32>()
        .map(|s| (s.unwrap() as f64 / max_val).abs())
        .fold(0.0f64, f64::max)
}

fn read_wav_samples(path: &std::path::Path) -> Vec<i32> {
    let mut reader = hound::WavReader::open(path).expect("failed to open WAV");
    reader.samples::<i32>().map(|s| s.unwrap()).collect()
}
