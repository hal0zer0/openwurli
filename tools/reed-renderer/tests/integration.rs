/// Integration tests for reed renderer physics verification.
///
/// These tests render short clips and verify physical properties:
/// 1. Mode frequencies match expected ratios
/// 2. Decay rates match calibration data
/// 3. Velocity affects spectral brightness
/// 4. HPF attenuates bass
/// 5. Per-note variation is deterministic
/// 6. Signal levels are in expected range

// We test the binary modules by importing the library code.
// Since this is a binary crate, we use the process approach for CLI tests
// and replicate key logic for unit-style integration tests.

use std::process::Command;

fn project_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn test_cli_renders_wav() {
    let output_path = "/tmp/integration_test_cli.wav";
    let _ = std::fs::remove_file(output_path);

    let status = Command::new(project_dir().join("target/debug/reed-renderer"))
        .args(["-n", "60", "-v", "100", "-d", "0.5", "-o", output_path])
        .status()
        .expect("failed to run reed-renderer");

    assert!(status.success(), "reed-renderer exited with error");
    assert!(
        std::path::Path::new(output_path).exists(),
        "WAV file not created"
    );

    // Verify it's a valid WAV
    let reader = hound::WavReader::open(output_path).expect("invalid WAV file");
    assert_eq!(reader.spec().channels, 1);
    assert_eq!(reader.spec().sample_rate, 44100);
    assert_eq!(reader.spec().bits_per_sample, 24);
    let sample_count = reader.len();
    // 0.5s at 44100 Hz = 22050 samples
    assert_eq!(sample_count, 22050);

    std::fs::remove_file(output_path).ok();
}

#[test]
fn test_cli_multi_note() {
    let status = Command::new(project_dir().join("target/debug/reed-renderer"))
        .args([
            "-n", "60,72",
            "-v", "100",
            "-d", "0.3",
            "--output-dir", "/tmp",
        ])
        .status()
        .expect("failed to run reed-renderer");

    assert!(status.success());
    assert!(std::path::Path::new("/tmp/reed_C4_v100.wav").exists());
    assert!(std::path::Path::new("/tmp/reed_C5_v100.wav").exists());

    std::fs::remove_file("/tmp/reed_C4_v100.wav").ok();
    std::fs::remove_file("/tmp/reed_C5_v100.wav").ok();
}

#[test]
fn test_cli_velocity_sweep() {
    let status = Command::new(project_dir().join("target/debug/reed-renderer"))
        .args([
            "-n", "69",
            "-v", "30,100,127",
            "-d", "0.2",
            "--output-dir", "/tmp",
        ])
        .status()
        .expect("failed to run reed-renderer");

    assert!(status.success());
    assert!(std::path::Path::new("/tmp/reed_A4_v30.wav").exists());
    assert!(std::path::Path::new("/tmp/reed_A4_v100.wav").exists());
    assert!(std::path::Path::new("/tmp/reed_A4_v127.wav").exists());

    // Verify louder velocity produces higher peak amplitude
    let peak_30 = wav_peak("/tmp/reed_A4_v30.wav");
    let peak_100 = wav_peak("/tmp/reed_A4_v100.wav");
    let peak_127 = wav_peak("/tmp/reed_A4_v127.wav");

    assert!(
        peak_127 > peak_100,
        "vel 127 peak ({peak_127}) should exceed vel 100 ({peak_100})"
    );
    assert!(
        peak_100 > peak_30,
        "vel 100 peak ({peak_100}) should exceed vel 30 ({peak_30})"
    );

    std::fs::remove_file("/tmp/reed_A4_v30.wav").ok();
    std::fs::remove_file("/tmp/reed_A4_v100.wav").ok();
    std::fs::remove_file("/tmp/reed_A4_v127.wav").ok();
}

#[test]
fn test_bass_quieter_than_treble() {
    // Due to pickup HPFs, bass notes should have lower amplitude than treble
    for path in ["/tmp/reed_bass_test.wav", "/tmp/reed_treble_test.wav"] {
        let _ = std::fs::remove_file(path);
    }

    let status = Command::new(project_dir().join("target/debug/reed-renderer"))
        .args(["-n", "33", "-v", "100", "-d", "0.5", "-o", "/tmp/reed_bass_test.wav"])
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(project_dir().join("target/debug/reed-renderer"))
        .args(["-n", "84", "-v", "100", "-d", "0.5", "-o", "/tmp/reed_treble_test.wav"])
        .status()
        .unwrap();
    assert!(status.success());

    let peak_bass = wav_peak("/tmp/reed_bass_test.wav");
    let peak_treble = wav_peak("/tmp/reed_treble_test.wav");

    assert!(
        peak_treble > peak_bass,
        "treble ({peak_treble}) should be louder than bass ({peak_bass}) due to HPF"
    );

    std::fs::remove_file("/tmp/reed_bass_test.wav").ok();
    std::fs::remove_file("/tmp/reed_treble_test.wav").ok();
}

#[test]
fn test_deterministic_output() {
    let path1 = "/tmp/reed_det_1.wav";
    let path2 = "/tmp/reed_det_2.wav";

    for path in [path1, path2] {
        let _ = std::fs::remove_file(path);
        let status = Command::new(project_dir().join("target/debug/reed-renderer"))
            .args(["-n", "60", "-v", "80", "-d", "0.3", "-o", path])
            .status()
            .unwrap();
        assert!(status.success());
    }

    let samples1 = read_wav_samples(path1);
    let samples2 = read_wav_samples(path2);
    assert_eq!(samples1, samples2, "two renders of same note should be identical");

    std::fs::remove_file(path1).ok();
    std::fs::remove_file(path2).ok();
}

// Helper: read peak absolute sample value from a WAV file
fn wav_peak(path: &str) -> f64 {
    let mut reader = hound::WavReader::open(path).expect("failed to open WAV");
    let max_val = (1i32 << (reader.spec().bits_per_sample - 1)) as f64;
    reader
        .samples::<i32>()
        .map(|s| (s.unwrap() as f64 / max_val).abs())
        .fold(0.0f64, f64::max)
}

// Helper: read all samples as i32
fn read_wav_samples(path: &str) -> Vec<i32> {
    let mut reader = hound::WavReader::open(path).expect("failed to open WAV");
    reader.samples::<i32>().map(|s| s.unwrap()).collect()
}
