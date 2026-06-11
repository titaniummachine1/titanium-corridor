//! Monotonic clock shim — `std::time::Instant` on native, `web_time::Instant`
//! on wasm32 (where `std::time::Instant::now()` panics).

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::{Duration, Instant};

#[cfg(target_arch = "wasm32")]
pub use web_time::{Duration, Instant};
