//! 10-band equalizer wrapper around `dsyneq`.
//!
//! Phase 0: stub. Phase 4 wires the rodio mixer → dsyneq → output chain.

#![allow(dead_code)]

#[derive(Clone, Debug)]
pub struct BandGain {
    pub hz: u32,
    pub gain_db: f32,
}

#[derive(Clone, Debug)]
pub struct Equalizer {
    pub bands: Vec<BandGain>,
}

impl Default for Equalizer {
    fn default() -> Self {
        Self {
            bands: vec![
                BandGain { hz: 60, gain_db: 0.0 },
                BandGain { hz: 170, gain_db: 0.0 },
                BandGain { hz: 310, gain_db: 0.0 },
                BandGain { hz: 600, gain_db: 0.0 },
                BandGain { hz: 1000, gain_db: 0.0 },
                BandGain { hz: 3000, gain_db: 0.0 },
                BandGain { hz: 6000, gain_db: 0.0 },
                BandGain { hz: 12000, gain_db: 0.0 },
                BandGain { hz: 14000, gain_db: 0.0 },
                BandGain { hz: 16000, gain_db: 0.0 },
            ],
        }
    }
}
