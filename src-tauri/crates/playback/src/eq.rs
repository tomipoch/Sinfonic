//! 10-band graphic equalizer.
//!
//! Wraps a stream of `f32` samples and applies a cascade of RBJ-style
//! peaking-EQ biquad filters — the same architecture that every
//! hardware graphic EQ and most DAWs use. Each band has an independent
//! `(frequency, Q, gain_db)` configuration that the user can tweak at
//! runtime; coefficients are recomputed on demand via
//! [`Equalizer::set_band`].
//!
//! # Why custom biquads
//!
//! The architecture called for a `dsyneq` dependency, but no such crate
//! exists on crates.io. RBJ biquad formulas are a handful of lines per
//! filter type, well-documented in the audio engineering Cookbook
//! (<https://www.w3.org/TR/audio-eq-cookbook/>), and avoid pulling in
//! another crate just to multiply samples by filtered coefficients.

use std::sync::Arc;

use crate::biquad::Biquad;
use parking_lot::Mutex;
use rodio::Source;

/// ISO-standard 10-band graphic-EQ layout: 60 Hz, 170 Hz, 310 Hz, 600 Hz,
/// 1 kHz, 3 kHz, 6 kHz, 12 kHz, 14 kHz, 16 kHz.
pub const DEFAULT_BANDS: &[(u32, f32)] = &[
    (60, 0.0),
    (170, 0.0),
    (310, 0.0),
    (600, 0.0),
    (1_000, 0.0),
    (3_000, 0.0),
    (6_000, 0.0),
    (12_000, 0.0),
    (14_000, 0.0),
    (16_000, 0.0),
];

/// Q factor for the peaking filters. √2 ≈ 1.41 is the textbook "musical"
/// Q for graphic EQs.
const PEAK_Q: f32 = 1.41;

/// One band of the equalizer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandGain {
    /// Centre frequency in Hz (e.g. `1000.0`).
    pub hz: f32,
    /// Gain in decibels applied at the centre frequency. Range: `[-12.0, +12.0]`.
    pub gain_db: f32,
}

impl BandGain {
    pub fn new(hz: u32, gain_db: f32) -> Self {
        Self {
            hz: hz as f32,
            gain_db,
        }
    }
}

/// A full per-channel biquad cascade: 10 peaking filters, one per
/// default band. Each channel of a stereo source has its own
/// [`ChannelEq`] so the IIR state stays independent — sharing state
/// across channels would treat the stream as a single mono source at
/// N× the sample rate, which gives wrong frequency response.
#[derive(Clone, Debug)]
pub struct ChannelEq {
    bands: Vec<Biquad>,
}

impl ChannelEq {
    fn flat(sample_rate: u32) -> Self {
        let bands = DEFAULT_BANDS
            .iter()
            .map(|(hz, _)| Biquad::peaking(sample_rate, *hz as f32, PEAK_Q, 0.0))
            .collect();
        Self { bands }
    }

    fn apply_band(&mut self, band_index: usize, sample_rate: u32, hz: f32, gain_db: f32) {
        if let Some(slot) = self.bands.get_mut(band_index) {
            *slot = Biquad::peaking(sample_rate, hz, PEAK_Q, gain_db);
        }
    }

    fn process_frame(&mut self, frame: &mut [f32]) {
        for biquad in &mut self.bands {
            biquad.process_in_place(frame);
        }
    }
}

/// Mutable equalizer state shared between the `AudioPlayer` and every
/// in-flight [`EqSource`] reading from it. Holds one [`ChannelEq`] per
/// channel — see the type docs on [`ChannelEq`] for why.
#[derive(Debug)]
pub struct Equalizer {
    bands: Vec<BandGain>,
    sample_rate: u32,
    channel_eqs: Vec<ChannelEq>,
}

impl Equalizer {
    /// Build a flat equalizer using the project's default band layout.
    pub fn flat() -> Self {
        Self {
            bands: DEFAULT_BANDS
                .iter()
                .map(|(hz, gain_db)| BandGain {
                    hz: *hz as f32,
                    gain_db: *gain_db,
                })
                .collect(),
            sample_rate: 44_100,
            channel_eqs: Vec::new(),
        }
    }

    /// Replace the gain on one band. The band is matched by Hz so that
    /// the band layout can be reordered without breaking callers.
    /// `gain_db` is clamped to `[-12.0, +12.0]`.
    pub fn set_band(&mut self, hz: u32, gain_db: f32) {
        let clamped = gain_db.clamp(-12.0, 12.0);
        if let Some(b) = self.bands.iter_mut().find(|b| b.hz as u32 == hz) {
            b.gain_db = clamped;
            if let Some(index) = DEFAULT_BANDS.iter().position(|(band_hz, _)| *band_hz == hz) {
                for eq in &mut self.channel_eqs {
                    eq.apply_band(index, self.sample_rate, b.hz, clamped);
                }
            }
        }
    }

    /// All current band gains, in declaration order.
    pub fn bands(&self) -> &[BandGain] {
        &self.bands
    }

    /// Current sample rate used for biquad coefficient calculation.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Update the sample rate and rebuild every per-channel biquad
    /// chain with the new rate + current gains. Called by [`EqSource`]
    /// the first time it sees a real frame, and on every sample-rate
    /// change.
    fn rebuild_for_sample_rate(&mut self, rate: u32) {
        if self.sample_rate == rate && !self.channel_eqs.is_empty() {
            return;
        }
        self.sample_rate = rate;
        for eq in &mut self.channel_eqs {
            *eq = ChannelEq::flat(rate);
            for (i, band) in self.bands.iter().enumerate() {
                eq.apply_band(i, rate, band.hz, band.gain_db);
            }
        }
    }

    /// Ensure `channels` [`ChannelEq`]s exist, allocating new ones
    /// for any channels we haven't seen before.
    fn ensure_channels(&mut self, channels: usize) {
        if channels == 0 {
            return;
        }
        while self.channel_eqs.len() < channels {
            let mut eq = ChannelEq::flat(self.sample_rate);
            for (i, band) in self.bands.iter().enumerate() {
                eq.apply_band(i, self.sample_rate, band.hz, band.gain_db);
            }
            self.channel_eqs.push(eq);
        }
    }

    /// Process one interleaved frame in place. `channel` is the
    /// channel index for the first sample in `frame` (i.e. the whole
    /// frame is `[ch, ch+1, ch+2, ...]`).
    fn process_frame(&mut self, channel: usize, frame: &mut [f32]) {
        if let Some(eq) = self.channel_eqs.get_mut(channel) {
            eq.process_frame(frame);
        }
    }
}

impl Default for Equalizer {
    fn default() -> Self {
        Self::flat()
    }
}

/// `Arc<Mutex<Equalizer>>` — the shareable handle passed to every
/// `EqSource`.
pub type SharedEqualizer = Arc<Mutex<Equalizer>>;

/// Apply the equalizer to a `rodio::Source`. Each frame (one sample per
/// channel) cascades through all 10 peaking biquads on its respective
/// channel.
pub fn apply<S>(source: S, eq: SharedEqualizer) -> EqSource<S>
where
    S: Source<Item = f32>,
{
    EqSource::new(source, eq)
}

/// `Source` adapter that pipes samples through an [`Equalizer`].
pub struct EqSource<S>
where
    S: Source<Item = f32>,
{
    inner: S,
    eq: SharedEqualizer,
    /// Scratch buffer for one frame. Sized lazily once we know the
    /// channel count.
    scratch: Vec<f32>,
    /// Index of the first sample of `scratch` within the source's
    /// interleaved stream. We pull one full frame's worth of samples
    /// into `scratch` per refill.
    cursor: usize,
    channel: usize,
}

impl<S> EqSource<S>
where
    S: Source<Item = f32>,
{
    fn new(inner: S, eq: SharedEqualizer) -> Self {
        Self {
            inner,
            eq,
            scratch: Vec::new(),
            cursor: 0,
            channel: 0,
        }
    }

    /// Pull one full frame of `channels` samples from `inner` into
    /// `scratch` and run them through the EQ. Returns `false` if the
    /// underlying source ran out mid-frame.
    fn refill(&mut self) -> bool {
        let channels = self.inner.channels() as usize;
        if channels == 0 {
            return false;
        }
        let sample_rate = self.inner.sample_rate();
        {
            let mut eq = self.eq.lock();
            eq.rebuild_for_sample_rate(sample_rate);
            eq.ensure_channels(channels);
        }
        if self.scratch.len() != channels {
            self.scratch.resize(channels, 0.0);
        }
        for slot in self.scratch.iter_mut() {
            match self.inner.next() {
                Some(s) => *slot = s,
                None => return false,
            }
        }
        let mut eq = self.eq.lock();
        eq.process_frame(self.channel, &mut self.scratch);
        self.channel = (self.channel + 1) % channels;
        self.cursor = 0;
        true
    }
}

impl<S> Iterator for EqSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.scratch.len() && !self.refill() {
            return None;
        }
        let sample = self.scratch[self.cursor];
        self.cursor += 1;
        Some(sample)
    }
}

impl<S> Source for EqSource<S>
where
    S: Source<Item = f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        self.inner.total_duration()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_equalizer_passes_frame_through_unchanged() {
        let mut eq = Equalizer::flat();
        eq.rebuild_for_sample_rate(48_000);
        eq.ensure_channels(2);
        let mut frame = [0.25_f32, -0.25];
        eq.process_frame(0, &mut frame);
        // At 0 dB the biquad coefficients are very close to identity;
        // allow a small tolerance for IIR coefficient rounding.
        assert!((frame[0] - 0.25).abs() < 1e-3, "got {}", frame[0]);
        assert!((frame[1] + 0.25).abs() < 1e-3, "got {}", frame[1]);
    }

    #[test]
    fn set_band_clamps_extreme_gains() {
        let mut eq = Equalizer::flat();
        eq.set_band(1000, 50.0);
        let b = eq.bands().iter().find(|b| b.hz == 1000.0).unwrap();
        assert!(b.gain_db <= 12.0);
        eq.set_band(1000, -50.0);
        let b = eq.bands().iter().find(|b| b.hz == 1000.0).unwrap();
        assert!(b.gain_db >= -12.0);
    }

    #[test]
    fn set_band_ignores_unknown_frequencies() {
        let mut eq = Equalizer::flat();
        let before = eq.bands().to_vec();
        eq.set_band(42_000, 6.0);
        assert_eq!(eq.bands(), before.as_slice());
    }

    #[test]
    fn default_layout_has_ten_bands() {
        let eq = Equalizer::flat();
        assert_eq!(eq.bands().len(), DEFAULT_BANDS.len());
        assert_eq!(DEFAULT_BANDS.len(), 10);
    }
}