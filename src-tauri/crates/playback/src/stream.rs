//! StreamHandle — owns a single in-flight stream.
//!
//! Phase 0: stub. Phase 4 wraps the rodio `Player` + `Decoder` chain.

#![allow(dead_code)]

pub struct StreamHandle;

impl StreamHandle {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StreamHandle {
    fn default() -> Self {
        Self::new()
    }
}
