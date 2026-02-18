"""Shared harmonic analysis utilities for the ML training pipeline.

Provides Goertzel single-frequency measurement (precise, slow) and
FFT-based bulk harmonic extraction (fast, ~15x quicker for H1-H8).
Factored from compare_ltas.py.
"""

import numpy as np
import soundfile as sf


def load_audio(path, sr_target=44100):
    """Load audio file (WAV/FLAC), downmix to mono, resample if needed.

    Returns (data_float64, sample_rate).
    """
    data, sr = sf.read(path, dtype='float64', always_2d=True)
    # Downmix to mono
    if data.shape[1] > 1:
        data = data.mean(axis=1)
    else:
        data = data[:, 0]

    if sr != sr_target:
        from scipy.signal import resample_poly
        from math import gcd
        g = gcd(sr_target, sr)
        data = resample_poly(data, sr_target // g, sr // g)
        sr = sr_target

    return data, sr


def goertzel_mag(signal, sr, target_freq, search_pct=0.01):
    """Goertzel algorithm: amplitude at target_freq with +/-search_pct peak search.

    Searches +/-search_pct around target_freq in 11 steps to handle tuning variation.
    Returns linear amplitude (not dB).
    """
    best_mag = 0.0
    N = len(signal)
    for offset in np.linspace(-search_pct, search_pct, 11):
        f = target_freq * (1.0 + offset)
        k = int(round(f * N / sr))
        if k <= 0 or k >= N // 2:
            continue
        w = 2.0 * np.pi * k / N
        coeff = 2.0 * np.cos(w)
        s1, s2 = 0.0, 0.0
        for x in signal:
            s0 = x + coeff * s1 - s2
            s2 = s1
            s1 = s0
        mag = np.sqrt(s1 * s1 + s2 * s2 - coeff * s1 * s2) / N * 2.0
        if mag > best_mag:
            best_mag = mag
    return best_mag


def extract_harmonics_fft(signal, sr, f0, n_harmonics=8, search_pct=0.01):
    """FFT-based harmonic extraction with per-harmonic peak search.

    Zero-pads to 4x length for frequency resolution, then searches +/-search_pct
    around each harmonic for the peak bin.

    Returns:
        amps: array of linear amplitudes for H1..Hn
        freqs: array of measured frequencies for H1..Hn
    """
    N = len(signal)
    # Zero-pad to 4x for ~0.07 Hz resolution at 44.1kHz with 0.65s window
    nfft = N * 4
    window = np.hanning(N)
    # Correct for window energy loss (hanning has coherent gain of 0.5)
    windowed = signal * window
    spectrum = np.abs(np.fft.rfft(windowed, n=nfft))
    # Normalize: 2/N for single-sided, /0.5 for hanning coherent gain
    spectrum = spectrum * 2.0 / N / 0.5
    freqs_axis = np.fft.rfftfreq(nfft, d=1.0 / sr)

    amps = np.zeros(n_harmonics)
    freqs = np.zeros(n_harmonics)

    for h in range(n_harmonics):
        fh = f0 * (h + 1)
        if fh >= sr / 2 - 100:
            amps[h] = 1e-20
            freqs[h] = fh
            continue
        # Search window
        f_lo = fh * (1.0 - search_pct)
        f_hi = fh * (1.0 + search_pct)
        mask = (freqs_axis >= f_lo) & (freqs_axis <= f_hi)
        if not np.any(mask):
            amps[h] = 1e-20
            freqs[h] = fh
            continue
        idx = np.where(mask)[0]
        peak_idx = idx[np.argmax(spectrum[idx])]
        amps[h] = spectrum[peak_idx]
        freqs[h] = freqs_axis[peak_idx]

    return amps, freqs


def extract_harmonics_goertzel(signal, sr, f0, n_harmonics=8, search_pct=0.01):
    """Goertzel-based harmonic extraction (precise, slower).

    Returns:
        amps: array of linear amplitudes for H1..Hn
    """
    amps = np.zeros(n_harmonics)
    for h in range(n_harmonics):
        fh = f0 * (h + 1)
        if fh >= sr / 2 - 100:
            amps[h] = 1e-20
        else:
            amps[h] = max(goertzel_mag(signal, sr, fh, search_pct), 1e-20)
    return amps


def amps_to_dB(amps, ref=None):
    """Convert linear amplitudes to dB. If ref is None, use H1 (first element)."""
    if ref is None:
        ref = max(amps[0], 1e-20)
    return 20.0 * np.log10(np.maximum(amps, 1e-20) / ref)


def midi_to_freq(midi_note):
    """Convert MIDI note number to frequency in Hz."""
    return 440.0 * 2.0 ** ((midi_note - 69) / 12.0)


def freq_to_midi(freq):
    """Convert frequency in Hz to MIDI note number (float)."""
    return 69.0 + 12.0 * np.log2(freq / 440.0)


def extract_sustain_window(data, sr, onset_offset_ms=150, sustain_end_ms=800):
    """Extract sustain window from a mono audio signal.

    Finds onset by 10% peak threshold, then returns the window from
    onset+onset_offset_ms to onset+sustain_end_ms.
    """
    peak = np.max(np.abs(data))
    if peak < 1e-10:
        return data, 0
    threshold = 0.10 * peak
    onset_idx = 0
    for i in range(len(data)):
        if abs(data[i]) > threshold:
            onset_idx = i
            break
    start = onset_idx + int(onset_offset_ms * sr / 1000)
    end = onset_idx + int(sustain_end_ms * sr / 1000)
    end = min(end, len(data))
    if start >= end:
        start = max(0, end - int(0.2 * sr))
    return data[start:end], onset_idx
