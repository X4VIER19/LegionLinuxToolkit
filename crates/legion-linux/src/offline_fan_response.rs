//! Offline analysis of a captured experimental fan endpoint response.
//!
//! This module opens only a user-selected regular file. It rejects `/dev`,
//! `/proc`, `/sys`, symbolic links, and every non-regular file before reading
//! at most 101 bytes. It neither locates nor opens the kernel endpoint.

use legion_fan::{
    DecodedKernelFanEndpointResponse, FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES,
    KernelFanEndpointError, decode_kernel_endpoint_bytes,
};
use rustix::{
    fs::{CWD, FileType, Mode, OFlags, ResolveFlags, fstat, openat2},
    io::{Errno, read},
};
use serde::Serialize;
use std::{
    fmt,
    fs::File,
    path::{Component, Path},
};

/// This label describes an already captured file, not a live hardware query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OfflineFanResponseProvenance {
    OfflineCapturedUnverified,
}

/// The complete, lossless offline interpretation suitable for text or JSON.
/// Neither table is assigned a physical unit or sensor meaning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OfflineFanResponseAnalysis {
    pub provenance: OfflineFanResponseProvenance,
    pub identity_verified: bool,
    pub original_response: Vec<u8>,
    pub contract_version: u16,
    pub contract_header_size: u16,
    pub contract_status: &'static str,
    pub payload_size: usize,
    pub original_fat2: Vec<u8>,
    pub fan_table_size: u32,
    pub fan_table: Vec<u32>,
    pub sensor_table_size: u32,
    pub sensor_table: Vec<u32>,
}

impl From<DecodedKernelFanEndpointResponse> for OfflineFanResponseAnalysis {
    fn from(decoded: DecodedKernelFanEndpointResponse) -> Self {
        Self {
            provenance: OfflineFanResponseProvenance::OfflineCapturedUnverified,
            identity_verified: false,
            original_response: decoded.original.to_vec(),
            contract_version: decoded.version,
            contract_header_size: decoded.header_size,
            contract_status: "ok",
            payload_size: decoded.payload_size,
            original_fat2: decoded.fat2.original.to_vec(),
            fan_table_size: decoded.fat2.fan_table_size,
            fan_table: decoded.fat2.fan_table.to_vec(),
            sensor_table_size: decoded.fat2.sensor_table_size,
            sensor_table: decoded.fat2.sensor_table.to_vec(),
        }
    }
}

#[derive(Debug)]
pub enum OfflineFanResponseError {
    RelativePath,
    ForbiddenHostInterface { path: String },
    OpenFailed { reason: String },
    UnsafeFile { reason: String },
    ReadInterrupted,
    ReadFailed { reason: String },
    TooLarge { limit: usize },
    UnexpectedLength { actual: usize },
    Contract(KernelFanEndpointError),
}

impl fmt::Display for OfflineFanResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RelativePath => write!(f, "offline response path must be absolute"),
            Self::ForbiddenHostInterface { path } => write!(
                f,
                "offline response path must not refer to a host interface: {path}"
            ),
            Self::OpenFailed { reason } => write!(f, "could not open offline response: {reason}"),
            Self::UnsafeFile { reason } => {
                write!(f, "offline response is not a safe regular file: {reason}")
            }
            Self::ReadInterrupted => write!(f, "offline response read was interrupted before EOF"),
            Self::ReadFailed { reason } => {
                write!(f, "offline response read failed before EOF: {reason}")
            }
            Self::TooLarge { limit } => write!(
                f,
                "offline response exceeds the {limit}-byte acquisition limit"
            ),
            Self::UnexpectedLength { actual } => write!(
                f,
                "offline response has {actual} bytes; expected {FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES}"
            ),
            Self::Contract(error) => {
                write!(f, "offline response contract validation failed: {error}")
            }
        }
    }
}

impl std::error::Error for OfflineFanResponseError {}

/// Opens and validates one previously captured endpoint response. It never
/// performs a kernel, WMI, ACPI, EC, sysfs, procfs, or device read.
pub fn analyze_offline_fan_response_file(
    path: &Path,
) -> Result<OfflineFanResponseAnalysis, OfflineFanResponseError> {
    let bytes = read_offline_regular_file(path, FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES)?;
    if bytes.len() != FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES {
        return Err(OfflineFanResponseError::UnexpectedLength {
            actual: bytes.len(),
        });
    }
    decode_kernel_endpoint_bytes(&bytes)
        .map(OfflineFanResponseAnalysis::from)
        .map_err(OfflineFanResponseError::Contract)
}

/// Shared bounded offline acquisition. Never opens a device or follows links.
pub(crate) fn read_offline_regular_file(
    path: &Path,
    limit: usize,
) -> Result<Vec<u8>, OfflineFanResponseError> {
    if !path.is_absolute() {
        return Err(OfflineFanResponseError::RelativePath);
    }
    if path_targets_forbidden_interface(path) {
        return Err(OfflineFanResponseError::ForbiddenHostInterface {
            path: path.display().to_string(),
        });
    }

    let file = openat2(
        CWD,
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
        ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_SYMLINKS,
    )
    .map(File::from)
    .map_err(|error| OfflineFanResponseError::OpenFailed {
        reason: format_open_error(error),
    })?;

    let stat = fstat(&file).map_err(|error| OfflineFanResponseError::ReadFailed {
        reason: format!("could not inspect opened file: {error}"),
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(OfflineFanResponseError::UnsafeFile {
            reason: "opened object is not a regular file".to_owned(),
        });
    }

    read_bounded_offline_bytes(&file, limit)
}

fn read_bounded_offline_bytes(
    file: &File,
    limit: usize,
) -> Result<Vec<u8>, OfflineFanResponseError> {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0_u8; 4096];
        let requested = chunk.len().min(limit + 1 - bytes.len());
        match read(file, &mut chunk[..requested]) {
            Ok(0) => return Ok(bytes),
            Ok(count) => {
                bytes.extend_from_slice(&chunk[..count]);
                if bytes.len() == limit + 1 {
                    return Err(OfflineFanResponseError::TooLarge { limit });
                }
            }
            Err(Errno::INTR) => return Err(OfflineFanResponseError::ReadInterrupted),
            Err(error) => {
                return Err(OfflineFanResponseError::ReadFailed {
                    reason: error.to_string(),
                });
            }
        }
    }
}

fn path_targets_forbidden_interface(path: &Path) -> bool {
    let mut normalized = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir => normalized.clear(),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(component) => normalized.push(component),
            Component::Prefix(_) => return true,
        }
    }
    matches!(
        normalized.first().and_then(|part| part.to_str()),
        Some("dev" | "proc" | "sys")
    )
}

fn format_open_error(error: Errno) -> String {
    match error {
        Errno::LOOP => "symbolic links are rejected".to_owned(),
        Errno::NOSYS => "openat2 is unavailable; no fallback is permitted".to_owned(),
        error => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::fs::{Mode, mkfifoat};
    use std::{
        fs,
        os::unix::fs::symlink,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    fn temporary_directory() -> PathBuf {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "legion-linux-offline-fan-response-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn valid_response() -> Vec<u8> {
        let mut bytes = vec![0; FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES];
        bytes[0..2].copy_from_slice(&1_u16.to_le_bytes());
        bytes[2..4].copy_from_slice(&12_u16.to_le_bytes());
        bytes[8..12].copy_from_slice(&88_u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&10_u32.to_le_bytes());
        for index in 0..10 {
            let offset = 16 + index * 4;
            bytes[offset..offset + 4].copy_from_slice(&((index + 1) as u32).to_le_bytes());
        }
        bytes[56..60].copy_from_slice(&10_u32.to_le_bytes());
        for index in 0..10 {
            let offset = 60 + index * 4;
            bytes[offset..offset + 4].copy_from_slice(&((index + 11) as u32).to_le_bytes());
        }
        bytes
    }

    #[test]
    fn analyzes_a_regular_complete_fixture_without_claiming_live_hardware() {
        let directory = temporary_directory();
        let path = directory.join("response.bin");
        let bytes = valid_response();
        fs::write(&path, &bytes).unwrap();

        let analysis = analyze_offline_fan_response_file(&path).unwrap();
        assert_eq!(
            analysis.provenance,
            OfflineFanResponseProvenance::OfflineCapturedUnverified
        );
        assert!(!analysis.identity_verified);
        assert_eq!(analysis.original_response, bytes);
        assert_eq!(analysis.fan_table, (1..=10).collect::<Vec<_>>());
        assert_eq!(analysis.sensor_table, (11..=20).collect::<Vec<_>>());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_wrong_lengths_and_contract_errors() {
        let directory = temporary_directory();
        let short = directory.join("short.bin");
        fs::write(&short, [0_u8; 99]).unwrap();
        assert!(matches!(
            analyze_offline_fan_response_file(&short),
            Err(OfflineFanResponseError::UnexpectedLength { actual: 99 })
        ));

        let malformed = directory.join("malformed.bin");
        let mut bytes = valid_response();
        bytes[56..60].copy_from_slice(&9_u32.to_le_bytes());
        fs::write(&malformed, bytes).unwrap();
        assert!(matches!(
            analyze_offline_fan_response_file(&malformed),
            Err(OfflineFanResponseError::Contract(_))
        ));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reads_at_most_101_bytes_and_rejects_excess_content() {
        let directory = temporary_directory();
        let path = directory.join("oversized.bin");
        let mut bytes = valid_response();
        bytes.push(0);
        fs::write(&path, bytes).unwrap();
        assert!(matches!(
            analyze_offline_fan_response_file(&path),
            Err(OfflineFanResponseError::TooLarge { limit: 100 })
        ));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_relative_forbidden_symbolic_and_special_paths() {
        assert_eq!(
            analyze_offline_fan_response_file(Path::new("response.bin"))
                .unwrap_err()
                .to_string(),
            "offline response path must be absolute"
        );
        assert!(matches!(
            analyze_offline_fan_response_file(Path::new("/sys/firmware/response.bin")),
            Err(OfflineFanResponseError::ForbiddenHostInterface { .. })
        ));

        let directory = temporary_directory();
        let target = directory.join("target.bin");
        fs::write(&target, valid_response()).unwrap();
        let link = directory.join("link.bin");
        symlink(&target, &link).unwrap();
        assert!(matches!(
            analyze_offline_fan_response_file(&link),
            Err(OfflineFanResponseError::OpenFailed { .. })
        ));

        let root = openat2(
            CWD,
            &directory,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY,
            Mode::empty(),
            ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        )
        .unwrap();
        mkfifoat(&root, "response.pipe", Mode::RWXU).unwrap();
        assert!(matches!(
            analyze_offline_fan_response_file(&directory.join("response.pipe")),
            Err(OfflineFanResponseError::UnsafeFile { .. })
        ));
        fs::remove_dir_all(directory).unwrap();
    }
}
