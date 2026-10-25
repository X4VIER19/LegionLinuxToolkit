//! One fixed endpoint; no generic WMI method invocation or write interface.
pub use legion_fan::fan_full_speed::FanFullSpeedState;
use legion_fan::fan_full_speed::{FanFullSpeedResult, decode_full_speed_response};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct FullSpeedObservation {
    source: &'static str,
    result: FanFullSpeedResult,
}
impl FullSpeedObservation {
    /// Failure data cannot carry an enabled/disabled value or live provenance.
    pub fn unavailable(error: String) -> Self {
        let mut result = decode_full_speed_response(&[]);
        result.error = Some(error);
        Self {
            source: "acquisition-unavailable; no-valid-observation",
            result,
        }
    }
    pub fn simulation(bytes: &[u8]) -> Self {
        Self {
            source: "static-simulation",
            result: decode_full_speed_response(bytes),
        }
    }
    pub fn result(&self) -> &FanFullSpeedResult {
        &self.result
    }
    pub fn source(&self) -> &'static str {
        self.source
    }
}

#[cfg(feature = "experimental-fan-full-speed-read")]
pub fn approved_one_shot_full_speed_read(
    confirmation: &str,
) -> Result<FullSpeedObservation, String> {
    use rustix::{
        fs::{CWD, FileType, Mode, OFlags, ResolveFlags, fstat, openat2},
        io::read,
    };
    use std::{
        fs::File,
        sync::atomic::{AtomicBool, Ordering},
    };
    static ATTEMPTED: AtomicBool = AtomicBool::new(false);
    if confirmation != "I-UNDERSTAND-ONE-SHOT-FAN-FULL-SPEED-READ" {
        return Err("exact FanFullSpeed read confirmation required".into());
    }
    ATTEMPTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "one-shot read already attempted".to_owned())?;
    let file = openat2(
        CWD,
        "/dev/legion-fan-full-speed-read-experimental",
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
        ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_SYMLINKS,
    )
    .map(File::from)
    .map_err(|error| format!("fixed read endpoint unavailable: {error}"))?;
    let stat = fstat(&file).map_err(|error| error.to_string())?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::CharacterDevice {
        return Err("endpoint is not a character device".into());
    }
    let mut bytes = [0; legion_fan::fan_full_speed::FULL_SPEED_RESPONSE_BYTES];
    let count = read(&file, &mut bytes)
        .map_err(|error| format!("one-shot read failed; no retry: {error}"))?;
    Ok(FullSpeedObservation {
        source: "linux-wmi-one-shot; no-write-authorization",
        result: decode_full_speed_response(&bytes[..count]),
    })
}
