//! WurliEngine — synth engine extracted from `openwurli-plugin`.
//!
//! Owns voice management (slots, allocation, stealing, sustain pedal)
//! and the shared signal chain (preamp → fixed circuit drive → power amp
//! → speaker → PSG × user volume). Framework-agnostic so any host
//! (nih-plug, custom DAW integration, oomox/Vurli) can wrap it without
//! copying voice or signal-chain code.
//!
//! Smoothing model: the three audio-rate user params (volume, tremolo
//! depth, speaker character) ramp internally over `SMOOTH_SAMPLES_AT_44K1`
//! base-rate samples after each setter call to avoid zipper noise on knob
//! moves. Hosts can call setters at block rate without their own smoothers.
//! Block-rate params (MLP, DI limiter, noise) take effect immediately.

use crate::dk_preamp::DkPreamp;
use crate::oversampler::Oversampler;
use crate::power_amp::PowerAmp;
use crate::preamp::PreampModel;
use crate::speaker::Speaker;
use crate::tables;
use crate::tremolo::Tremolo;
use crate::voice::Voice;

const MAX_VOICES: usize = 64;
const MAX_BLOCK_SIZE: usize = 8192;

/// Voice slot lifecycle. Public for engine introspection from tests and
/// debug tooling; not part of the realtime API surface — hosts shouldn't
/// need to read this in `process()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceState {
    Free,
    Held,
    /// Key released while sustain pedal was down — reed rings undamped.
    Sustained,
    Releasing,
}

struct VoiceSlot {
    voice: Option<Voice>,
    state: VoiceState,
    midi_note: u8,
    age: u64,
    /// Voice being faded out from stealing (5 ms linear crossfade).
    steal_voice: Option<Voice>,
    steal_fade: u32,
    steal_fade_len: u32,
}

impl Default for VoiceSlot {
    fn default() -> Self {
        Self {
            voice: None,
            state: VoiceState::Free,
            midi_note: 0,
            age: 0,
            steal_voice: None,
            steal_fade: 0,
            steal_fade_len: 0,
        }
    }
}

/// Single-pole linear smoother: given a target value, produces a per-sample
/// ramp toward it over `samples` base-rate samples. Used for vol, tremolo
/// depth, speaker character so block-rate setter calls don't zipper.
struct LinearSmoother {
    current: f64,
    target: f64,
    step: f64,
    samples_remaining: u32,
    ramp_samples: u32,
}

impl LinearSmoother {
    fn new(initial: f64, ramp_samples: u32) -> Self {
        Self {
            current: initial,
            target: initial,
            step: 0.0,
            samples_remaining: 0,
            ramp_samples,
        }
    }

    fn set_target(&mut self, target: f64) {
        if (target - self.target).abs() < 1e-9 {
            return;
        }
        self.target = target;
        let delta = target - self.current;
        if self.ramp_samples == 0 {
            self.current = target;
            self.samples_remaining = 0;
            return;
        }
        self.step = delta / self.ramp_samples as f64;
        self.samples_remaining = self.ramp_samples;
    }

    /// Snap to value instantly (no ramp). Used on `reset()` and
    /// `set_sample_rate()` to avoid carrying a stale ramp across.
    fn snap_to(&mut self, value: f64) {
        self.current = value;
        self.target = value;
        self.step = 0.0;
        self.samples_remaining = 0;
    }

    fn set_ramp_samples(&mut self, ramp_samples: u32) {
        self.ramp_samples = ramp_samples;
        // Recompute step for any in-flight ramp so it still finishes correctly.
        if self.samples_remaining > 0 {
            self.step = (self.target - self.current) / ramp_samples.max(1) as f64;
            self.samples_remaining = ramp_samples;
        }
    }

    #[inline]
    fn next(&mut self) -> f64 {
        if self.samples_remaining > 0 {
            self.current += self.step;
            self.samples_remaining -= 1;
            if self.samples_remaining == 0 {
                self.current = self.target;
            }
        }
        self.current
    }
}

/// Wurlitzer 200A synth engine — owns voices, signal chain, and all
/// params except the host-facing parameter container.
///
/// Host integration sketch:
/// ```ignore
/// let mut engine = WurliEngine::new(sample_rate);
/// // Per buffer:
/// engine.set_volume(params.volume.value() as f64);
/// engine.set_tremolo_depth(params.tremolo_depth.value() as f64);
/// engine.set_speaker_character(params.speaker.value() as f64);
/// engine.set_mlp_enabled(params.mlp.value());
/// engine.set_noise_enabled(params.noise_enable.value());
/// engine.set_noise_gain(params.noise_gain.value() as f64);
/// for event in midi_events {
///     match event {
///         NoteOn(n, v) => engine.note_on(n, v),
///         NoteOff(n)   => engine.note_off(n),
///         Sustain(on)  => engine.set_sustain(on),
///     }
/// }
/// engine.render(&mut mono_out_buffer);
/// ```
pub struct WurliEngine {
    voices: Vec<VoiceSlot>,
    age_counter: u64,

    // Shared signal chain (mono, post voice-sum)
    preamp: DkPreamp,
    /// C-9 pole at the amp input (volume network, see tables::volume_pot_pole_hz):
    /// one-pole lowpass state and its coefficient, refreshed per base-rate sample.
    c9_state: f64,
    c9_alpha: f64,

    /// Test-only power-amp drive probe. Accumulates peak and mean-square of
    /// the signal actually presented to `PowerAmp::process`, i.e.
    /// `preamp_out * volume_pot_gain(vol)`. Used by the drive-headroom probe
    /// to compare against the amp's linearity envelope. `cfg(test)` so the
    /// hot loop carries nothing in release builds.
    #[cfg(test)]
    pub(crate) drive_peak: f64,
    #[cfg(test)]
    pub(crate) drive_sumsq: f64,
    #[cfg(test)]
    pub(crate) drive_n: u64,
    tremolo: Tremolo,
    oversampler: Oversampler,
    power_amp: PowerAmp,
    speaker: Speaker,

    // Pre-allocated scratch buffers
    voice_buf: Vec<f64>,
    sum_buf: Vec<f64>,
    up_buf: Vec<f64>,
    out_buf: Vec<f64>,

    // Sample rates
    sample_rate: f64,
    os_sample_rate: f64,
    /// Whether to oversample the preamp (false at >= 88.2 kHz host rates).
    oversample: bool,

    // MIDI state
    sustain_held: bool,
    mlp_enabled: bool,

    // Smoothed audio-rate params (target set by host, ramped per sample)
    volume: LinearSmoother,
    tremolo_depth: LinearSmoother,
    speaker_character: LinearSmoother,

    // Diagnostic counter — increments each time the NaN guard in
    // render_voices_to_preamp_out zeroes a block. A non-zero value at the
    // end of a render run means at least one voice produced non-finite
    // output and was force-freed without a release ramp.
    nan_guard_fires: u64,
}

impl WurliEngine {
    pub fn new(sample_rate: f64) -> Self {
        let oversample = sample_rate < 88_200.0;
        let os_sr = if oversample {
            sample_rate * 2.0
        } else {
            sample_rate
        };
        let ramp = ramp_samples_for_rate(sample_rate);
        Self {
            voices: (0..MAX_VOICES).map(|_| VoiceSlot::default()).collect(),
            age_counter: 0,
            preamp: DkPreamp::new(os_sr),
            c9_state: 0.0,
            c9_alpha: 1.0,
            #[cfg(test)]
            drive_peak: 0.0,
            #[cfg(test)]
            drive_sumsq: 0.0,
            #[cfg(test)]
            drive_n: 0,
            tremolo: Tremolo::new(0.5, os_sr),
            oversampler: Oversampler::new(),
            // Power amp runs at the oversampled rate alongside the preamp so
            // the BE integrator inside the melange-generated solver gets a
            // small enough timestep to avoid manufacturing high-order harmonics
            // on harmonic-rich inputs. Speaker stays at base rate (linear,
            // doesn't benefit from oversampling).
            power_amp: PowerAmp::new_at_sample_rate(os_sr),
            speaker: Speaker::new(sample_rate),
            voice_buf: vec![0.0; MAX_BLOCK_SIZE],
            sum_buf: vec![0.0; MAX_BLOCK_SIZE],
            up_buf: vec![0.0; MAX_BLOCK_SIZE * 2],
            out_buf: vec![0.0; MAX_BLOCK_SIZE],
            sample_rate,
            os_sample_rate: os_sr,
            oversample,
            sustain_held: false,
            mlp_enabled: false,
            volume: LinearSmoother::new(0.5, ramp),
            tremolo_depth: LinearSmoother::new(0.5, ramp),
            speaker_character: LinearSmoother::new(0.0, ramp),
            nan_guard_fires: 0,
        }
    }

    pub fn reset(&mut self) {
        for slot in &mut self.voices {
            slot.state = VoiceState::Free;
            slot.voice = None;
            slot.steal_voice = None;
            slot.steal_fade = 0;
        }
        self.preamp.reset();
        self.c9_state = 0.0;
        self.tremolo.reset();
        self.oversampler.reset();
        self.power_amp.reset();
        self.speaker.reset();
        self.age_counter = 0;
        self.sustain_held = false;
        // Snap smoothers so a ramp doesn't survive a transport reset.
        self.volume.snap_to(self.volume.target);
        self.tremolo_depth.snap_to(self.tremolo_depth.target);
        self.speaker_character
            .snap_to(self.speaker_character.target);
        self.warm_up();
    }

    /// Settle the preamp / shadow-pump / CdS-envelope to their steady operating
    /// point by running internal silence with the tremolo active. The real 200A
    /// preamp is always biased at its DC operating point; a solver started cold
    /// takes ~0.5 s (the shadow-pump's multi-second transient) to reach the
    /// steady tremolo modulation, and a note struck during that window rides a
    /// deep gain excursion that reads as repeated attacks. Called after every
    /// `reset()` and `set_sample_rate()` so the first note is always clean.
    /// Not on the per-buffer path — a one-time settle, allocation-free.
    pub fn warm_up(&mut self) {
        let mut scratch = [0.0f32; 512];
        let total = (self.sample_rate * 0.6) as usize;
        let mut done = 0;
        while done < total {
            let len = 512.min(total - done);
            self.render(&mut scratch[..len]);
            done += len;
        }
    }

    pub fn set_sample_rate(&mut self, sr: f64) {
        self.sample_rate = sr;
        self.oversample = sr < 88_200.0;
        self.os_sample_rate = if self.oversample { sr * 2.0 } else { sr };
        self.preamp = DkPreamp::new(self.os_sample_rate);
        self.tremolo = Tremolo::new(self.tremolo_depth.target, self.os_sample_rate);
        self.oversampler = Oversampler::new();
        self.power_amp = PowerAmp::new_at_sample_rate(self.os_sample_rate);
        self.speaker = Speaker::new(sr);
        let ramp = ramp_samples_for_rate(sr);
        self.volume.set_ramp_samples(ramp);
        self.tremolo_depth.set_ramp_samples(ramp);
        self.speaker_character.set_ramp_samples(ramp);
        self.warm_up();
    }

    pub fn ensure_buffer_capacity(&mut self, max_samples: usize) {
        if self.sum_buf.len() < max_samples {
            self.voice_buf.resize(max_samples, 0.0);
            self.sum_buf.resize(max_samples, 0.0);
            self.up_buf.resize(max_samples * 2, 0.0);
            self.out_buf.resize(max_samples, 0.0);
        }
    }

    // ── MIDI ────────────────────────────────────────────────────────────

    pub fn note_on(&mut self, note: u8, velocity: f32) {
        let note = note.clamp(tables::MIDI_LO, tables::MIDI_HI);

        // Re-strike of a sustained note: damp the old vibration before
        // the new attack (real 200A has one reed per pitch).
        for slot in &mut self.voices {
            if slot.state == VoiceState::Sustained && slot.midi_note == note {
                slot.state = VoiceState::Releasing;
                if let Some(ref mut voice) = slot.voice {
                    voice.note_off();
                }
            }
        }

        let slot_idx = self.allocate_voice();
        let slot = &mut self.voices[slot_idx];

        if slot.state != VoiceState::Free {
            // Stealing an active voice: 5 ms linear crossfade.
            let fade_samples = (self.sample_rate * 0.005) as u32;
            slot.steal_voice = slot.voice.take();
            slot.steal_fade = fade_samples;
            slot.steal_fade_len = fade_samples;
        }

        self.age_counter += 1;
        let noise_seed = (note as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(self.age_counter as u32);
        slot.voice = Some(Voice::note_on(
            note,
            velocity as f64,
            self.sample_rate,
            noise_seed,
            self.mlp_enabled,
        ));
        slot.state = VoiceState::Held;
        slot.midi_note = note;
        slot.age = self.age_counter;
    }

    pub fn note_off(&mut self, note: u8) {
        let note = note.clamp(tables::MIDI_LO, tables::MIDI_HI);
        let oldest_idx = self
            .voices
            .iter()
            .enumerate()
            .filter(|(_, s)| s.state == VoiceState::Held && s.midi_note == note)
            .min_by_key(|(_, s)| s.age)
            .map(|(i, _)| i);
        if let Some(idx) = oldest_idx {
            if self.sustain_held {
                self.voices[idx].state = VoiceState::Sustained;
            } else {
                self.voices[idx].state = VoiceState::Releasing;
                if let Some(ref mut voice) = self.voices[idx].voice {
                    voice.note_off();
                }
            }
        }
    }

    pub fn set_sustain(&mut self, held: bool) {
        if self.sustain_held && !held {
            // Pedal release: damp every voice that was held by the pedal.
            for slot in &mut self.voices {
                if slot.state == VoiceState::Sustained {
                    slot.state = VoiceState::Releasing;
                    if let Some(ref mut voice) = slot.voice {
                        voice.note_off();
                    }
                }
            }
        }
        self.sustain_held = held;
    }

    // ── Param setters ────────────────────────────────────────────────────

    /// One-pole lowpass coefficient for corner `fc` at rate `sr`
    /// (exact-decay form; a corner above Nyquist degrades to passthrough).
    fn one_pole_alpha(fc: f64, sr: f64) -> f64 {
        if !fc.is_finite() || fc >= 0.5 * sr {
            1.0
        } else {
            1.0 - (-2.0 * std::f64::consts::PI * fc / sr).exp()
        }
    }

    pub fn set_volume(&mut self, v: f64) {
        self.volume.set_target(v);
    }

    pub fn set_tremolo_depth(&mut self, depth: f64) {
        self.tremolo_depth.set_target(depth);
    }

    pub fn set_speaker_character(&mut self, c: f64) {
        self.speaker_character.set_target(c);
    }

    pub fn set_mlp_enabled(&mut self, on: bool) {
        self.mlp_enabled = on;
    }

    pub fn set_noise_enabled(&mut self, on: bool) {
        self.preamp.set_noise_enabled(on);
    }

    pub fn set_noise_gain(&mut self, gain: f64) {
        self.preamp.set_thermal_gain(gain);
    }

    /// Enable / disable rail sag modeling on the power amp. On by default
    /// (correct physics; +0.66 % CPU). Toggle off for bit-compat A/B against
    /// the pre-rail-sag adapter. See `power_amp.rs` `RailDynamics` and
    /// `docs/research/output-stage.md` §4.3.1.
    pub fn set_rail_sag(&mut self, on: bool) {
        self.power_amp.set_rail_sag(on);
    }

    pub fn rail_sag_enabled(&self) -> bool {
        self.power_amp.rail_sag_enabled()
    }

    /// Power-amp diagnostic snapshot:
    /// `(clamp_count, nr_max_iter_count, peak_output_volts)`.
    /// Use during diagnostic renders to see if the divergence guard is
    /// firing, NR is failing, or the amp is producing unphysical voltages.
    pub fn power_amp_diag(&self) -> (u64, u64, f64) {
        self.power_amp.diag_snapshot()
    }

    // ── Render ───────────────────────────────────────────────────────────

    /// Render `out.len()` mono samples through the full chain.
    pub fn render(&mut self, out: &mut [f32]) {
        let len = out.len();
        if len == 0 {
            return;
        }
        self.ensure_buffer_capacity(len);

        // After this call, out_buf holds the post-power-amp signal at base rate
        // (preamp + volume network + power amp ran inside the OS bus together).
        self.render_voices_to_preamp_out(0, len);

        for (i, sample_slot) in out.iter_mut().enumerate() {
            let speaker_char = self.speaker_character.next();
            self.speaker.set_character(speaker_char);
            let shaped = self.speaker.process(self.out_buf[i]);
            // User volume is the drawn pot between preamp and power amp and
            // was applied in render_voices_to_preamp_out. POST_SPEAKER_GAIN is
            // the rail-to-full-scale mapping (0 dB: ±22 V = ±1.0). No
            // nonlinear ceiling — output peak management belongs in the host
            // / Vurli per project scope rules.
            let post_gain = shaped * tables::POST_SPEAKER_GAIN;
            let sample = post_gain as f32;
            // NaN guard: non-finite samples crash PipeWire/JACK audio engines.
            *sample_slot = if sample.is_finite() {
                sample
            } else {
                self.preamp.reset();
                self.c9_state = 0.0;
                self.oversampler.reset();
                self.power_amp.reset();
                self.speaker.reset();
                0.0f32
            };
        }

        self.cleanup_voices();
    }

    /// Render voices through the preamp and write into `self.out_buf[offset..offset+len]`.
    /// Pulled out so introspection tests can call it without going through the chain.
    pub(crate) fn render_voices_to_preamp_out(&mut self, offset: usize, len: usize) {
        // Sum all active voices
        self.sum_buf[..len].fill(0.0);
        for slot in &mut self.voices {
            if slot.state == VoiceState::Free && slot.steal_voice.is_none() {
                continue;
            }

            if let Some(ref mut voice) = slot.voice {
                voice.render(&mut self.voice_buf[..len]);
                for i in 0..len {
                    self.sum_buf[i] += self.voice_buf[i];
                }
            }

            if let Some(ref mut steal) = slot.steal_voice {
                steal.render(&mut self.voice_buf[..len]);
                let fade_len = slot.steal_fade_len as f64;
                for i in 0..len {
                    let remaining = slot.steal_fade.saturating_sub(i as u32);
                    let gain = remaining as f64 / fade_len;
                    self.sum_buf[i] += self.voice_buf[i] * gain;
                }
                slot.steal_fade = slot.steal_fade.saturating_sub(len as u32);
                if slot.steal_fade == 0 {
                    slot.steal_voice = None;
                }
            }
        }

        // NaN guard: catch non-finite voice output BEFORE the oversampler.
        // Once NaN enters IIR allpass filter state, it persists and causes
        // per-sample preamp resets (expensive full_dc_solve) → xruns → frozen audio.
        if self.sum_buf[..len].iter().any(|s| !s.is_finite()) {
            self.nan_guard_fires += 1;
            self.sum_buf[..len].fill(0.0);
            for slot in &mut self.voices {
                if slot.state == VoiceState::Free && slot.steal_voice.is_none() {
                    continue;
                }
                if let Some(ref mut voice) = slot.voice {
                    voice.render(&mut self.voice_buf[..len]);
                    if self.voice_buf[..len].iter().any(|s| !s.is_finite()) {
                        slot.state = VoiceState::Free;
                        slot.voice = None;
                    }
                }
                if let Some(ref mut steal) = slot.steal_voice {
                    steal.render(&mut self.voice_buf[..len]);
                    if self.voice_buf[..len].iter().any(|s| !s.is_finite()) {
                        slot.steal_voice = None;
                        slot.steal_fade = 0;
                    }
                }
            }
        }

        if self.oversample {
            self.oversampler
                .upsample_2x(&self.sum_buf[..len], &mut self.up_buf[..len * 2]);

            // Per base-rate sample: advance tremolo depth + volume smoothers
            // once. Run the preamp + power amp twice (once per OS sample) so
            // both nonlinear stages see 88.2 kHz timestep — critical for the
            // melange BE integrator inside the power amp not to manufacture
            // high-order harmonics from the harmonic-rich pickup output.
            for i in 0..len {
                let depth = self.tremolo_depth.next();
                self.tremolo.set_depth(depth);
                // The drawn volume network: preamp open-circuit output → R-11
                // → 10K pot at the user's position → amp input (loaded), with
                // C-9 against the wiper's source resistance as a one-pole.
                let vol = self.volume.next();
                let drive_gain =
                    self.preamp.open_circuit_output_factor() * tables::volume_pot_gain(vol);
                self.c9_alpha =
                    Self::one_pole_alpha(tables::volume_pot_pole_hz(vol), self.os_sample_rate);

                for j in 0..2 {
                    let idx = i * 2 + j;
                    let r_ldr = self.tremolo.process();
                    self.preamp.set_ldr_resistance(r_ldr);
                    let preamp_out = self.preamp.process_sample(self.up_buf[idx]);
                    self.c9_state += self.c9_alpha * (preamp_out * drive_gain - self.c9_state);
                    let drive = self.c9_state;
                    #[cfg(test)]
                    {
                        self.drive_peak = self.drive_peak.max(drive.abs());
                        self.drive_sumsq += drive * drive;
                        self.drive_n += 1;
                    }
                    self.up_buf[idx] = self.power_amp.process(drive);
                }
            }

            self.oversampler.downsample_2x(
                &self.up_buf[..len * 2],
                &mut self.out_buf[offset..offset + len],
            );
        } else {
            for i in 0..len {
                let depth = self.tremolo_depth.next();
                self.tremolo.set_depth(depth);
                let r_ldr = self.tremolo.process();
                self.preamp.set_ldr_resistance(r_ldr);
                let preamp_out = self.preamp.process_sample(self.sum_buf[i]);
                let vol = self.volume.next();
                let drive_gain =
                    self.preamp.open_circuit_output_factor() * tables::volume_pot_gain(vol);
                self.c9_alpha =
                    Self::one_pole_alpha(tables::volume_pot_pole_hz(vol), self.os_sample_rate);
                self.c9_state += self.c9_alpha * (preamp_out * drive_gain - self.c9_state);
                let drive = self.c9_state;
                #[cfg(test)]
                {
                    self.drive_peak = self.drive_peak.max(drive.abs());
                    self.drive_sumsq += drive * drive;
                    self.drive_n += 1;
                }
                self.out_buf[offset + i] = self.power_amp.process(drive);
            }
        }
    }

    fn allocate_voice(&self) -> usize {
        let mut best_idx = 0;
        let mut best_priority = u64::MAX;

        for (i, slot) in self.voices.iter().enumerate() {
            // Free (immediate) > oldest Releasing > oldest Sustained > oldest Held.
            // Sustained voices already had their key released — less disruptive
            // to steal than a Held voice the player is still pressing.
            let priority = match slot.state {
                VoiceState::Free => return i,
                VoiceState::Releasing => slot.age,
                VoiceState::Sustained => slot.age + u64::MAX / 4,
                VoiceState::Held => slot.age + u64::MAX / 2,
            };
            if priority < best_priority {
                best_priority = priority;
                best_idx = i;
            }
        }

        best_idx
    }

    fn cleanup_voices(&mut self) {
        for slot in &mut self.voices {
            if slot.state != VoiceState::Free
                && let Some(ref voice) = slot.voice
                && voice.is_silent()
            {
                slot.state = VoiceState::Free;
                slot.voice = None;
            }
        }
    }

    // ── Test/inspection helpers ──────────────────────────────────────────

    #[doc(hidden)]
    pub fn active_voice_count(&self) -> usize {
        self.voices
            .iter()
            .filter(|s| s.state != VoiceState::Free)
            .count()
    }

    #[doc(hidden)]
    pub fn held_voice_count(&self) -> usize {
        self.voices
            .iter()
            .filter(|s| s.state == VoiceState::Held)
            .count()
    }

    #[doc(hidden)]
    pub fn sustained_voice_count(&self) -> usize {
        self.voices
            .iter()
            .filter(|s| s.state == VoiceState::Sustained)
            .count()
    }

    #[doc(hidden)]
    pub fn has_steal_voice_for(&self, note: u8) -> bool {
        self.voices
            .iter()
            .any(|s| s.midi_note == note && s.steal_voice.is_some())
    }

    #[doc(hidden)]
    pub fn count_voices_in_state(&self, state: VoiceState) -> usize {
        self.voices.iter().filter(|s| s.state == state).count()
    }

    #[doc(hidden)]
    pub fn count_voices_with_note_in_state(&self, note: u8, state: VoiceState) -> usize {
        self.voices
            .iter()
            .filter(|s| s.state == state && s.midi_note == note)
            .count()
    }

    /// Forces sustain state directly — bypasses the pedal release damping
    /// path. Test-only; production code should use `set_sustain`.
    #[doc(hidden)]
    pub fn force_sustain_held(&mut self, held: bool) {
        self.sustain_held = held;
    }

    #[doc(hidden)]
    /// Number of times the NaN guard has zeroed a block since construction
    /// or the last `reset_nan_guard_count()`. Use for diagnostics — a
    /// non-zero value means at least one voice produced non-finite output
    /// and was force-freed without a release ramp.
    pub fn nan_guard_fires(&self) -> u64 {
        self.nan_guard_fires
    }

    /// Reset the NaN-guard fire counter to zero. Useful when scoping a
    /// counter to a specific MIDI passage rather than the engine lifetime.
    pub fn reset_nan_guard_count(&mut self) {
        self.nan_guard_fires = 0;
    }

    pub fn is_sustain_held(&self) -> bool {
        self.sustain_held
    }
}

fn ramp_samples_for_rate(sample_rate: f64) -> u32 {
    // 5 ms equivalent at any rate.
    ((sample_rate * 0.005) as u32).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> WurliEngine {
        WurliEngine::new(44_100.0)
    }

    #[test]
    fn test_engine_instantiates() {
        let e = engine();
        assert_eq!(e.voices.len(), MAX_VOICES);
        assert_eq!(e.sample_rate, 44_100.0);
    }

    #[test]
    fn test_note_on_allocates_voice() {
        let mut e = engine();
        e.note_on(60, 0.8);
        assert_eq!(e.held_voice_count(), 1);
    }

    #[test]
    fn test_note_off_releases_voice() {
        let mut e = engine();
        e.note_on(60, 0.8);
        e.note_off(60);
        assert_eq!(e.held_voice_count(), 0);
    }

    #[test]
    fn test_polyphony_up_to_max_voices() {
        let mut e = engine();
        for n in 0..MAX_VOICES {
            e.note_on((36 + n) as u8, 0.8);
        }
        assert_eq!(e.held_voice_count(), MAX_VOICES);
    }

    #[test]
    fn test_voice_stealing_when_full() {
        let mut e = engine();
        for n in 0..MAX_VOICES {
            e.note_on((36 + n) as u8, 0.8);
        }
        e.note_on(96, 0.8);
        assert_eq!(e.held_voice_count(), MAX_VOICES);
        assert!(e.has_steal_voice_for(96));
    }

    #[test]
    fn test_render_produces_output() {
        let mut e = engine();
        e.note_on(60, 0.8);
        let mut buf = vec![0.0f32; 256];
        e.render(&mut buf);
        let energy: f64 = buf.iter().map(|s| (*s as f64) * (*s as f64)).sum();
        assert!(energy > 0.0, "render produced silence after note-on");
    }

    #[test]
    fn test_render_no_notes_is_near_silent() {
        // Idle output has a small chain-startup DC settling transient
        // (preamp/speaker biasing). Threshold mirrors the legacy plugin
        // test (0.03) which carried the same accommodation.
        let mut e = engine();
        let mut buf = vec![0.0f32; 512];
        e.render(&mut buf);
        let peak = buf.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak < 0.05, "idle output peak {peak} too high");
    }

    #[test]
    fn test_reset_clears_voices() {
        let mut e = engine();
        e.note_on(60, 0.8);
        e.note_on(72, 0.8);
        e.reset();
        assert_eq!(e.active_voice_count(), 0);
    }

    #[test]
    fn test_sustain_defers_note_off() {
        let mut e = engine();
        e.set_sustain(true);
        e.note_on(60, 0.8);
        e.note_off(60);
        assert_eq!(e.sustained_voice_count(), 1);
        assert_eq!(e.held_voice_count(), 0);
        e.set_sustain(false);
        // Releasing now (note_off triggered on pedal release)
        assert_eq!(e.sustained_voice_count(), 0);
    }

    #[test]
    fn test_volume_smoother_ramps() {
        let mut e = engine();
        e.set_volume(1.0);
        let mut buf = vec![0.0f32; 1];
        e.render(&mut buf); // advances smoother by 1 sample
        // Default starts at 0.5, ramping toward 1.0 over ~220 samples
        assert!(e.volume.current > 0.5);
        assert!(e.volume.current < 1.0);
    }

    #[test]
    fn test_engine_peak_below_unity_at_vol_1() {
        // PSG sizing invariant (2026-04-26; rebuilt 2026-08-19): worst
        // documented case — chord-ff at vol=1.0, MLP on, tremolo bright —
        // must peak ≤ 1.0 IN PRODUCTION CONDITIONS. That means (a) a WARMED
        // engine (the plugin always warm_up()s; a cold engine is a state no
        // host produces — the pre-2026-08-19 version of this test measured
        // exactly that and passed as false assurance), and (b) swept across
        // tremolo onset phases: the ~40 ms attack ends well inside the
        // 178 ms tremolo period, so onset phase is frozen into the envelope
        // and worst-phase peak exceeds lucky-phase peak by ~1.2 dB. Phase
        // grid covers the measured-broad worst plateau (onset ≈ 160-168 ms
        // at 2 ms density — see tests/peak_window_probe.rs); breadth of the
        // plateau is what licenses a coarse grid here.
        let mut peak = 0.0f32;
        for delay_ms in [0usize, 44, 89, 133, 162] {
            let mut e = engine();
            e.ensure_buffer_capacity(1024);
            e.set_volume(1.0);
            e.set_tremolo_depth(1.0);
            e.set_speaker_character(0.0);
            e.set_mlp_enabled(true);
            e.set_noise_enabled(false);
            e.warm_up();

            // Settle the volume smoother, then place the chord at this
            // tremolo phase.
            let mut buf = vec![0.0f32; 1024];
            for _ in 0..6 {
                e.render(&mut buf);
            }
            let delay = 44_100 * delay_ms / 1000;
            let mut pos = 0;
            while pos < delay {
                let len = 1024.min(delay - pos);
                e.render(&mut buf[..len]);
                pos += len;
            }

            // Canonical worst-case chord (2026-04-25/26 crackle diagnosis):
            // C minor 7 voicing across the register.
            for &n in &[48u8, 55, 60, 63, 67, 70] {
                e.note_on(n, 0.95);
            }

            // 1.0 s capture: the true peak arrives at t ≈ 39 ms (attack
            // transient; verified over 8 s — nothing later).
            let total = 44_100;
            let mut pos2 = 0;
            while pos2 < total {
                let len = 1024.min(total - pos2);
                e.render(&mut buf[..len]);
                for &s in &buf[..len] {
                    peak = peak.max(s.abs());
                }
                pos2 += len;
            }
        }
        // Slack of 0.02 leaves room for harmless f32 rounding / minor
        // stochastic per-voice variation. Crackle threshold is well above 1.0.
        //
        // POST_SPEAKER_GAIN_DB is calibrated against the SHIPPING preamp (the
        // legacy hand solver, the default feature set). The `melange-preamp`
        // study path is a different solver of the same circuit and lands ~0.5 dB
        // hotter at the same PSG — it is a diagnostic path ("not normally needed
        // at runtime", see dk_preamp/mod.rs), not a level-calibrated one, so
        // trimming PSG to suit it would make the shipping instrument quieter for
        // no shipping benefit. The bound below is therefore path-specific: the
        // default build keeps the hard invariant, the study build gets a looser
        // bound that still catches a real level regression.
        //
        // Measured 2026-09-13 at PSG 14.8 after the drawn-topology revision:
        // legacy 0.980, melange-preamp 1.0356.
        #[cfg(not(feature = "melange-preamp"))]
        let (limit, which) = (1.02f32, "shipping (legacy preamp)");
        #[cfg(feature = "melange-preamp")]
        let (limit, which) = (1.10f32, "study path (melange-preamp, NOT level-calibrated)");
        assert!(
            peak <= limit,
            "engine peak {peak:.4} exceeds {limit} at vol=1.0 chord-ff on the {which} \
             (PSG drift suspected — see tables.rs::POST_SPEAKER_GAIN_DB)"
        );
    }

    #[test]
    fn test_user_volume_follows_pot_law() {
        // VOLUME NETWORK (2026-09-23): user volume is the drawn pot between
        // preamp and power amp (tables::volume_pot_gain), so the output must
        // follow that law in the amp's linear region. Compare vol=0.75 and
        // vol=1.0 (both drive the amp above its crossover notch and below
        // its clip knee for a single note); the peak ratio must match the
        // pot-network gain ratio. If this regresses, either the network or
        // the post-amp path picked up an extra volume dependence.
        let render_at = |vol: f64| -> f32 {
            let mut e = engine();
            e.ensure_buffer_capacity(1024);
            e.set_volume(vol);
            e.set_tremolo_depth(0.0);
            e.set_speaker_character(0.0);
            e.set_mlp_enabled(true);
            e.set_noise_enabled(false);
            let mut warmup = vec![0.0f32; 1024];
            for _ in 0..6 {
                e.render(&mut warmup);
            }
            e.note_on(60, 0.95);
            let total = (44_100.0 * 0.5) as usize;
            let mut out = Vec::with_capacity(total);
            let mut buf = vec![0.0f32; 1024];
            let mut pos = 0;
            while pos < total {
                let len = 1024.min(total - pos);
                e.render(&mut buf[..len]);
                out.extend_from_slice(&buf[..len]);
                pos += len;
            }
            out.iter().fold(0.0f32, |a, &s| a.max(s.abs()))
        };
        let p_075 = render_at(0.75);
        let p_10 = render_at(1.0);
        let ratio = (p_10 / p_075) as f64;
        let expected = tables::volume_pot_gain(1.0) / tables::volume_pot_gain(0.75);
        // ±6 % slack: per-voice OU jitter, MLP determinism, and the amp's
        // residual level dependence (crossover notch, shelf) at these drives.
        assert!(
            (ratio / expected - 1.0).abs() < 0.06,
            "user volume should follow the pot network: ratio {ratio:.3}, expected \
             {expected:.3} (p_075={p_075:.4}, p_10={p_10:.4}) — see tables::volume_pot_gain."
        );
    }

    #[test]
    fn test_higher_velocity_louder() {
        let mut e = engine();
        e.set_volume(0.5);
        // pp render
        e.note_on(60, 0.2);
        let mut soft = vec![0.0f32; 4096];
        e.render(&mut soft);
        e.reset();
        // ff render
        e.note_on(60, 1.0);
        let mut loud = vec![0.0f32; 4096];
        e.render(&mut loud);

        let soft_rms: f64 =
            (soft.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / soft.len() as f64).sqrt();
        let loud_rms: f64 =
            (loud.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / loud.len() as f64).sqrt();
        assert!(
            loud_rms > soft_rms,
            "ff RMS ({loud_rms}) should exceed pp RMS ({soft_rms})"
        );
    }

    // ── Note clamping ───────────────────────────────────────────────────

    #[test]
    fn test_note_clamps_to_valid_range() {
        let mut e = engine();
        e.note_on(0, 0.8); // below MIDI_LO
        e.note_on(127, 0.8); // above MIDI_HI
        assert_eq!(
            e.held_voice_count(),
            2,
            "both notes clamped, both allocated"
        );
    }

    // ── Sustain pedal ───────────────────────────────────────────────────

    #[test]
    fn test_sustain_pedal_release_triggers_damping() {
        // Hold pedal, play note, release key (→ Sustained), release pedal
        // (→ Releasing). Voice should be damping, not still ringing.
        let mut e = engine();
        e.set_sustain(true);
        e.note_on(60, 0.8);
        e.note_off(60);
        assert_eq!(e.sustained_voice_count(), 1);
        e.set_sustain(false);
        // Sustained voice transitions to Releasing on pedal release.
        assert_eq!(e.sustained_voice_count(), 0);
        assert_eq!(e.count_voices_in_state(VoiceState::Releasing), 1);
    }

    #[test]
    fn test_sustain_held_voices_still_render() {
        // After pedal-up triggers damping, the voice should still produce
        // output during the release tail (not abruptly silenced).
        let mut e = engine();
        e.set_sustain(true);
        e.note_on(60, 0.8);
        let mut buf = vec![0.0f32; 1024];
        e.render(&mut buf);
        e.note_off(60);
        e.render(&mut buf);
        e.set_sustain(false);
        e.render(&mut buf);
        let energy: f64 = buf.iter().map(|s| (*s as f64).powi(2)).sum();
        assert!(energy > 0.0, "voice silenced too soon after pedal release");
    }

    #[test]
    fn test_no_sustain_normal_note_off() {
        // Pedal up: note_off transitions Held → Releasing immediately.
        let mut e = engine();
        e.note_on(60, 0.8);
        e.note_off(60);
        assert_eq!(e.held_voice_count(), 0);
        assert_eq!(e.count_voices_in_state(VoiceState::Releasing), 1);
    }

    #[test]
    fn test_voice_stealing_prefers_sustained_over_held() {
        // Fill all slots: half Held, half Sustained. Stealing should target
        // a Sustained slot first (less disruptive than a key-down note).
        let mut e = engine();
        e.set_sustain(true);
        for n in 0..(MAX_VOICES / 2) {
            e.note_on((36 + n) as u8, 0.8);
            e.note_off((36 + n) as u8); // → Sustained
        }
        for n in (MAX_VOICES / 2)..MAX_VOICES {
            e.note_on((36 + n) as u8, 0.8); // → Held
        }
        let sustained_before = e.sustained_voice_count();
        let held_before = e.held_voice_count();
        assert_eq!(sustained_before + held_before, MAX_VOICES);

        e.note_on(127, 0.8); // forces a steal
        // The new voice replaces a Sustained slot; held count unchanged.
        assert_eq!(e.held_voice_count(), held_before + 1);
        assert_eq!(e.sustained_voice_count(), sustained_before - 1);
    }

    #[test]
    fn test_reattack_releases_sustained_same_note() {
        // Real 200A has one reed per pitch — re-striking a sustained note
        // releases the old voice rather than accumulating duplicates.
        let mut e = engine();
        e.set_sustain(true);
        e.note_on(60, 0.8);
        e.note_off(60); // → Sustained
        e.note_on(60, 0.8); // re-attack same note
        assert_eq!(
            e.count_voices_with_note_in_state(60, VoiceState::Sustained),
            0,
            "old sustained voice should be released on re-attack"
        );
        assert_eq!(
            e.count_voices_with_note_in_state(60, VoiceState::Held),
            1,
            "new voice should be Held"
        );
    }

    #[test]
    fn test_pedal_up_only_releases_sustained_not_held() {
        // Pedal release should ONLY affect Sustained voices; voices the
        // player is still holding stay Held.
        let mut e = engine();
        e.set_sustain(true);
        e.note_on(60, 0.8);
        e.note_off(60); // → Sustained
        e.note_on(64, 0.8); // → Held (key still down)
        assert_eq!(e.sustained_voice_count(), 1);
        assert_eq!(e.held_voice_count(), 1);
        e.set_sustain(false);
        assert_eq!(e.sustained_voice_count(), 0);
        assert_eq!(e.held_voice_count(), 1, "held voice must survive pedal up");
    }

    #[test]
    fn test_reset_clears_sustain_state() {
        let mut e = engine();
        e.set_sustain(true);
        e.note_on(60, 0.8);
        e.note_off(60);
        e.reset();
        assert!(!e.is_sustain_held(), "sustain flag must clear on reset");
        assert_eq!(e.active_voice_count(), 0);
    }

    #[test]
    fn test_note_off_for_nonexistent_note_is_noop() {
        let mut e = engine();
        e.note_on(60, 0.8);
        e.note_off(72); // never on
        assert_eq!(
            e.held_voice_count(),
            1,
            "wrong-note off should not damage state"
        );
    }

    // ── NaN / divergence guards ─────────────────────────────────────────

    #[test]
    fn test_volume_zero_and_back_no_nan() {
        // Regression: sweeping volume to zero and back caused NaN that
        // crashed PipeWire. The engine smoother + chain must be NaN-free.
        let mut e = engine();
        e.note_on(60, 0.8);
        let mut buf = vec![0.0f32; 512];
        for _ in 0..4 {
            e.set_volume(0.0);
            e.render(&mut buf);
            e.set_volume(0.5);
            e.render(&mut buf);
        }
        assert!(
            buf.iter().all(|s| s.is_finite()),
            "non-finite sample leaked"
        );
    }

    #[test]
    fn test_no_catastrophic_output_spikes_under_continuous_play() {
        // Regression for the melange power-amp divergence guard: under
        // continuous chord transitions the solver intermittently failed to
        // converge and produced +20 dBFS spikes. Engine wraps the same
        // power-amp adapter so the guard still applies.
        //
        // Threshold: +14 dBFS. Normal chord-ff content at default gain
        // (vol=0.5, PSG=+14.5 dB, no DI limiter) peaks around +5 dBFS
        // — set the guard well above that and well below the +20 dBFS
        // catastrophic value the divergence guard exists to prevent.
        let mut e = engine();
        let chords: [[u8; 3]; 4] = [[60, 64, 67], [62, 65, 69], [64, 67, 71], [65, 69, 72]];
        let block = 256;
        let blocks_per_segment = 86; // ~0.5 s per chord at 44.1 k
        let mut buf = vec![0.0f32; block];
        let mut peak = 0.0f32;
        for (i, chord) in chords.iter().cycle().take(8).enumerate() {
            for n in chord {
                e.note_on(*n, 1.0);
            }
            for _ in 0..blocks_per_segment {
                e.render(&mut buf);
                peak = peak.max(buf.iter().map(|s| s.abs()).fold(0.0f32, f32::max));
            }
            for n in chord {
                e.note_off(*n);
            }
            // Small drain between chords
            if i % 2 == 1 {
                for _ in 0..5 {
                    e.render(&mut buf);
                }
            }
        }
        let peak_dbfs = 20.0 * peak.max(1e-12).log10();
        assert!(
            peak_dbfs < 14.0,
            "peak {peak_dbfs:.2} dBFS exceeds +14 dBFS divergence-guard threshold"
        );
    }

    // ── Sample-rate / buffer-size changes ───────────────────────────────

    #[test]
    fn test_sound_after_sample_rate_change() {
        let mut e = engine();
        e.set_sample_rate(48_000.0);
        e.note_on(60, 0.8);
        let mut buf = vec![0.0f32; 1024];
        e.render(&mut buf);
        let energy: f64 = buf.iter().map(|s| (*s as f64).powi(2)).sum();
        assert!(energy > 0.0, "no output after sample-rate change");
    }

    #[test]
    fn test_buffer_capacity_grows_to_render() {
        // Engine should auto-grow its internal scratch buffers when render
        // is called with a larger output slice than initial allocation.
        let mut e = engine();
        e.note_on(60, 0.8);
        let mut big_buf = vec![0.0f32; 16_384]; // > MAX_BLOCK_SIZE default
        e.render(&mut big_buf);
        assert!(big_buf.iter().all(|s| s.is_finite()));
    }

    // ── Tremolo smoothing ────────────────────────────────────────────────

    #[test]
    fn test_tremolo_smoother_does_not_pin_depth_to_zero() {
        // Regression for the legacy nih-plug smoother bug where
        // smoothed.next() returned 0 forever before host init. Engine
        // smoother snaps to its construction default, so even without an
        // explicit set_tremolo_depth() call the depth is the constructor's
        // value (0.5), and a 4 s render must produce audible AM.
        let mut e = engine();
        e.note_on(60, 0.9);
        let sr = 44_100_usize;
        let total = sr * 4;
        let block = 256;
        let mut samples = Vec::with_capacity(total);
        let mut buf = vec![0.0f32; block];
        for _ in 0..(total / block) {
            e.render(&mut buf);
            samples.extend_from_slice(&buf);
        }
        // RMS envelope over 20 ms windows, ignoring the first 0.5 s.
        let win = sr / 50;
        let skip = 25;
        let n_wins = samples.len() / win;
        let mut env_db = Vec::with_capacity(n_wins);
        for i in skip..n_wins {
            let s = i * win;
            let rms = (samples[s..s + win]
                .iter()
                .map(|x| (*x as f64).powi(2))
                .sum::<f64>()
                / win as f64)
                .sqrt();
            env_db.push(20.0 * (rms + 1e-12).log10());
        }
        let env_min = env_db.iter().cloned().fold(f64::INFINITY, f64::min);
        let env_max = env_db.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let swing = env_max - env_min;
        assert!(
            swing > 3.0,
            "Tremolo should produce > 3 dB RMS swing at default depth 0.5: got {swing:.2} dB"
        );
    }

    /// Power-amp drive-headroom probe (Phase-2 closing measurement).
    ///
    /// Reports peak and RMS of the signal actually presented to the power amp
    /// (`preamp_out * volume_pot_gain(vol)`, at vol 0.5 and 1.0) so it can be
    /// compared against the amp's linearity envelope. Reference points:
    ///   * behavioural amp closed-loop gain 68.93x, rails at 22 V
    ///     => hard rail clip at 22 / 68.93 = 319 mV input
    ///   * SPICE reference: THD crosses 1% near 295-300 mV input, razor cliff above
    ///
    /// Since 2026-09-23 the drive follows the drawn volume network, so the
    /// probe runs at the default pot position and at full.
    #[test]
    #[ignore = "diagnostic probe"]
    fn power_amp_drive_headroom_probe() {
        let sr = 44_100.0;
        for vol in [0.5f64, 1.0] {
            println!("  --- pot position {vol} (R-11 mid-travel) ---");
            let mk = || {
                let mut e = WurliEngine::new(sr);
                e.ensure_buffer_capacity(1024);
                e.set_volume(vol);
                e.set_tremolo_depth(1.0);
                e.set_speaker_character(0.0);
                e.set_mlp_enabled(true);
                e.set_noise_enabled(false);
                e.warm_up();
                e
            };
            let run = |e: &mut WurliEngine, total: usize| {
                let mut buf = vec![0.0f32; 1024];
                let mut pos = 0;
                while pos < total {
                    let len = 1024.min(total - pos);
                    e.render(&mut buf[..len]);
                    pos += len;
                }
            };
            println!("  stimulus            peak mV    RMS mV   % of 319 mV clip");
            // (a) single ff notes across the register
            for note in [36u8, 48, 60, 72, 84] {
                let mut best = (0.0f64, 0.0f64);
                for delay_ms in [0usize, 44, 89, 133, 162] {
                    let mut e = mk();
                    run(&mut e, (sr as usize) * delay_ms / 1000);
                    e.drive_peak = 0.0;
                    e.drive_sumsq = 0.0;
                    e.drive_n = 0;
                    e.note_on(note, 0.95);
                    run(&mut e, sr as usize);
                    let rms = (e.drive_sumsq / e.drive_n as f64).sqrt();
                    if e.drive_peak > best.0 {
                        best = (e.drive_peak, rms);
                    }
                }
                println!(
                    "  note {note:>3} ff       {:8.1}  {:8.1}   {:6.1}%",
                    best.0 * 1000.0,
                    best.1 * 1000.0,
                    best.0 / 0.319 * 100.0
                );
            }
            // (b) worst-phase ff chord — same stimulus as the peak invariant
            let mut best = (0.0f64, 0.0f64);
            for delay_ms in [0usize, 44, 89, 133, 162] {
                let mut e = mk();
                run(&mut e, (sr as usize) * delay_ms / 1000);
                e.drive_peak = 0.0;
                e.drive_sumsq = 0.0;
                e.drive_n = 0;
                for &n in &[48u8, 55, 60, 63, 67, 70] {
                    e.note_on(n, 0.95);
                }
                run(&mut e, sr as usize);
                let rms = (e.drive_sumsq / e.drive_n as f64).sqrt();
                if e.drive_peak > best.0 {
                    best = (e.drive_peak, rms);
                }
            }
            println!(
                "  chord ff worst-ph  {:8.1}  {:8.1}   {:6.1}%",
                best.0 * 1000.0,
                best.1 * 1000.0,
                best.0 / 0.319 * 100.0
            );
        }
    }

    /// Behavioural power-amp linearity envelope — where does its nonlinearity
    /// actually engage? Sine sweep straight into `PowerAmp`, THD via DFT.
    /// Reference: SPICE says the real amp crosses 1% THD near 295-300 mV in.
    #[test]
    #[ignore = "diagnostic probe"]
    fn power_amp_linearity_envelope_probe() {
        use crate::power_amp::PowerAmp;
        let sr = 88_200.0;
        let n = 8192usize;
        // COHERENT analysis frequency: the 4096-sample window must hold an
        // integer number of cycles, else leakage from H1 sets a constant
        // apparent-THD floor that masks the real level dependence entirely
        // (an incoherent 220 Hz reads a flat 3.38% from 1 mV to 330 mV).
        let f = 10.0 * sr / (n as f64 / 2.0); // 215.33 Hz, exactly 10 cycles
        let thd_at = |amp_v: f64| {
            let mut pa = PowerAmp::new_at_sample_rate(sr);
            let mut y = vec![0.0f64; n];
            for i in 0..n {
                let t = i as f64 / sr;
                y[i] = pa.process(amp_v * (2.0 * std::f64::consts::PI * f * t).sin());
            }
            let w = &y[n / 2..];
            let mag = |k: f64| {
                let (mut re, mut im) = (0.0, 0.0);
                for (i, &v) in w.iter().enumerate() {
                    let ph = 2.0 * std::f64::consts::PI * k * i as f64 / sr;
                    re += v * ph.cos();
                    im -= v * ph.sin();
                }
                ((re / w.len() as f64).powi(2) + (im / w.len() as f64).powi(2)).sqrt()
            };
            let h1 = mag(f);
            let harm: f64 = (2..=8).map(|h| mag(h as f64 * f).powi(2)).sum();
            100.0 * harm.sqrt() / h1
        };
        println!("   input mV     THD %");
        for mv in [
            1.0f64, 5.0, 10.0, 20.0, 54.5, 100.0, 115.6, 150.0, 200.0, 250.0, 280.0, 295.0, 300.0,
            310.0, 319.0, 330.0,
        ] {
            println!("   {mv:8.1}  {:9.4}", thd_at(mv / 1000.0));
        }
    }
}
