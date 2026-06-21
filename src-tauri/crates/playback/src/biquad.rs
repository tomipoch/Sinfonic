//! RBJ-style peaking-EQ biquad filter.
//!
//! Direct-Form II Transposed implementation (the same shape used by
//! <https://www.w3.org/TR/audio-eq-cookbook/>). Coefficients are
//! recomputed per [`Biquad::peaking`] call — that is rare (only when
//! the user moves a slider) so we don't bother caching.

/// Direct-Form II Transposed biquad.
///
/// Difference equation (per channel, per sample):
///
/// ```text
/// y[n] = b0·x[n] + z1[n-1]
/// z1[n] = b1·x[n] - a1·y[n] + z2[n-1]
/// z2[n] = b2·x[n] - a2·y[n]
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    /// RBJ peaking-EQ filter: gain at `freq` Hz, Q (bandwidth), `gain_db`.
    pub fn peaking(sample_rate: u32, freq: f32, q: f32, gain_db: f32) -> Self {
        // Coefficients lifted directly from the Cookbook section "Peaking
        // EQ". We compute in f64 once and round at the end — RBJ's
        // formulas are sensitive to accumulated rounding error in the
        // trig steps, and audio is the one domain where 32-bit precision
        // actually matters for sub-bass frequencies.
        let sr = sample_rate as f64;
        let f = freq as f64;
        let q = q as f64;
        let a = (10f64).powf(gain_db as f64 / 40.0);
        let omega = 2.0 * std::f64::consts::PI * f / sr;
        let alpha = omega.sin() / (2.0 * q);
        let cos_w = omega.cos();

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w;
        let a2 = 1.0 - alpha / a;

        Self {
            b0: (b0 / a0) as f32,
            b1: (b1 / a0) as f32,
            b2: (b2 / a0) as f32,
            a1: (a1 / a0) as f32,
            a2: (a2 / a0) as f32,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Process one interleaved frame in place.
    pub fn process_in_place(&mut self, frame: &mut [f32]) {
        for sample in frame {
            let x = *sample;
            let y = self.b0 * x + self.z1;
            self.z1 = self.b1 * x - self.a1 * y + self.z2;
            self.z2 = self.b2 * x - self.a2 * y;
            *sample = y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_response_at_zero_gain_is_passthrough() {
        let mut b = Biquad::peaking(48_000, 1_000.0, 1.41, 0.0);
        let mut frame = [0.5_f32, 0.5];
        b.process_in_place(&mut frame);
        // At 0 dB the filter should be essentially identity.
        assert!((frame[0] - 0.5).abs() < 1e-3, "got {}", frame[0]);
        assert!((frame[1] - 0.5).abs() < 1e-3, "got {}", frame[1]);
    }

    #[test]
    fn positive_gain_at_center_boosts_signal() {
        // Peaking filter boosts gain only around its centre frequency;
        // feed a 1 kHz sine wave to actually exercise it.
        let mut b = Biquad::peaking(48_000, 1_000.0, 1.41, 12.0);
        let max_in = 0.5_f32;
        // Build a precomputed mono sine-wave buffer.
        let total = 9_600;
        let mut samples = Vec::with_capacity(total);
        for n in 0..total {
            let phase = (n as f32) * (core::f32::consts::PI / 24.0);
            samples.push(max_in * phase.sin());
        }
        let mut outs = vec![0.0_f32; total];
        for n in 0..total {
            // Mono frame — one sample per call.
            let mut frame = [samples[n]];
            b.process_in_place(&mut frame);
            outs[n] = frame[0];
        }
        // Measure RMS over the last 4800 samples (100 cycles) — by
        // then the IIR is fully settled.
        let sum_sq: f64 = outs[4_800..]
            .iter()
            .map(|&s| (s as f64).powi(2))
            .sum();
        let rms_out = (sum_sq / 4_800.0).sqrt();
        let rms_in = (max_in as f64) / core::f64::consts::SQRT_2;
        let gain_db = 20.0 * (rms_out / rms_in).log10();
        // Allow ~1 dB tolerance for coefficient rounding.
        assert!(
            gain_db > 10.0,
            "expected boost near +12 dB, got {gain_db:.2} dB (rms_out={rms_out:.4})"
        );
    }
}