//! One fixed FACT endpoint plus offline comparison; no generic WMI interface.
use std::path::Path;

pub fn analyze_fact_files(
    capture: &Path,
    dsdt: &Path,
) -> Result<legion_fan::fact::FactComparison, String> {
    use crate::offline_fan_response::read_offline_regular_file;
    let bytes = read_offline_regular_file(capture, legion_fan::fact::FACT_CAPTURE_BYTES)
        .map_err(|error| error.to_string())?;
    let aml =
        read_offline_regular_file(dsdt, 8 * 1024 * 1024).map_err(|error| error.to_string())?;
    let text = std::str::from_utf8(&aml).map_err(|error| error.to_string())?;
    let catalogues =
        legion_fan::parse_aml_fan_curve_catalogues(text).map_err(|error| error.to_string())?;
    legion_fan::fact::compare_fact_capture(&bytes, &catalogues)
}

#[cfg(feature = "experimental-fact-read")]
pub fn approved_fact_capture(confirmation: &str) -> Result<Vec<u8>, String> {
    use rustix::{
        fs::{CWD, FileType, Mode, OFlags, ResolveFlags, fstat, openat2},
        io::read,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    static ATTEMPTED: AtomicBool = AtomicBool::new(false);
    if confirmation != "I-UNDERSTAND-ONE-SHOT-FACT-CAPTURE" {
        return Err("exact FACT capture confirmation required".into());
    }
    ATTEMPTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "FACT batch already attempted; no retry")?;
    let file = openat2(
        CWD,
        "/dev/legion-fact-read-experimental",
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(|error| format!("FACT endpoint open failed: {error}; no fallback"))?;
    let stat = fstat(&file).map_err(|error| error.to_string())?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::CharacterDevice {
        return Err("FACT endpoint must be a character device".into());
    }
    let mut bytes = vec![0; legion_fan::fact::FACT_CAPTURE_BYTES];
    // Exactly one read syscall, including on EINTR/short read: never complete
    // a partial acquisition by triggering another endpoint invocation.
    let count = read(&file, &mut bytes).map_err(|error| format!("FACT read: {error}; no retry"))?;
    bytes.truncate(count);
    Ok(bytes) // Caller preserves error/partial bytes before interpreting them.
}
