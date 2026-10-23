//! Feature-gated, fixed-path Linux hwmon RPM observation.
//!
//! The production entry point is unavailable unless
//! `approved-real-hwmon-rpm-read` is explicitly selected. It accepts no path,
//! configuration, environment, retry, monitor, or write option. It first
//! reads the four fixed DMI attributes through the existing one-shot provider,
//! requires the exact documented 83EX identity, then enumerates fixed
//! `/sys/class/hwmon` entries and opens only `name` plus discovered
//! `fanN_input` attributes beneath an already-open `/sys` directory descriptor.

#[cfg(feature = "approved-real-hwmon-rpm-read")]
use crate::approved_one_shot_dmi_identity_read;
#[cfg(feature = "approved-real-hwmon-rpm-read")]
use crate::dmi_identity::DmiIdentityReading;
#[cfg(feature = "approved-real-hwmon-rpm-read")]
use crate::hwmon::build_linux_hwmon_reading;
use crate::hwmon::{
    HwmonDriverClassification, HwmonFanError, HwmonFanReading, HwmonReadingOrigin,
    HwmonSourceError, build_reading, fan_input_channel, parse_name, parse_rpm,
};
#[cfg(feature = "approved-real-hwmon-rpm-read")]
use legion_core::HostIdentity;
#[cfg(feature = "approved-real-hwmon-rpm-read")]
use rustix::fs::CWD;
use rustix::{
    fs::{FileType, Mode, OFlags, ResolveFlags, fstat, openat2},
    io::{Errno, read},
};
#[cfg(feature = "approved-real-hwmon-rpm-read")]
use serde::Serialize;
#[cfg(test)]
use std::path::Path;
#[cfg(feature = "approved-real-hwmon-rpm-read")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    fs::{self, File},
    path::PathBuf,
};

#[cfg(feature = "approved-real-hwmon-rpm-read")]
const SYSFS_ROOT: &str = "/sys";
#[cfg(feature = "approved-real-hwmon-rpm-read")]
const HWMON_DISCOVERY_DIRECTORY: &str = "/sys/class/hwmon";
const HWMON_RELATIVE_DIRECTORY: &str = "class/hwmon";
const MAXIMUM_HWMON_ATTRIBUTE_BYTES: usize = 128;
#[cfg(feature = "approved-real-hwmon-rpm-read")]
const REQUIRED_CONFIRMATION: &str = "I-UNDERSTAND-READ-ONLY-HWMON-RPM";
#[cfg(feature = "approved-real-hwmon-rpm-read")]
const DMI_CONFIRMATION: &str = "I-UNDERSTAND-READ-ONLY-DMI";

/// The only public result of the feature-gated provider. The included DMI
/// evidence remains observed and unverified; it is a fail-closed identity
/// gate, never a `RealReadAuthorization`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg(feature = "approved-real-hwmon-rpm-read")]
pub struct HwmonLinuxReading {
    pub dmi_identity: DmiIdentityReading,
    pub fans: Vec<HwmonFanReading>,
}

#[derive(Debug)]
#[cfg(feature = "approved-real-hwmon-rpm-read")]
pub enum HwmonRealReadError {
    ConfirmationRequired,
    AlreadyAttempted,
    Dmi(String),
    IdentityMismatch,
    OpenRoot(String),
    Discovery(String),
    Fan(HwmonFanError),
}

#[cfg(feature = "approved-real-hwmon-rpm-read")]
impl std::fmt::Display for HwmonRealReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfirmationRequired => write!(f, "exact hwmon RPM confirmation is required"),
            Self::AlreadyAttempted => write!(f, "hwmon RPM observation was already attempted"),
            Self::Dmi(error) => write!(f, "DMI identity gate failed: {error}"),
            Self::IdentityMismatch => write!(
                f,
                "DMI identity does not exactly match Lenovo 83EX / NUCN15WW"
            ),
            Self::OpenRoot(error) => write!(f, "could not open fixed /sys root: {error}"),
            Self::Discovery(error) => {
                write!(f, "could not enumerate fixed hwmon directory: {error}")
            }
            Self::Fan(error) => write!(f, "hwmon RPM observation failed: {error}"),
        }
    }
}

#[cfg(feature = "approved-real-hwmon-rpm-read")]
impl std::error::Error for HwmonRealReadError {}

#[cfg(feature = "approved-real-hwmon-rpm-read")]
static ONE_SHOT: AtomicBool = AtomicBool::new(false);

/// Executes at most one explicit real hwmon observation per process. This is
/// feature-gated and is intentionally not connected to `legion-cli`'s normal
/// simulated command. It does no writes, retries, polling, or firmware calls.
#[cfg(feature = "approved-real-hwmon-rpm-read")]
pub fn approved_one_shot_hwmon_rpm_read(
    confirmation: &str,
) -> Result<HwmonLinuxReading, HwmonRealReadError> {
    if confirmation != REQUIRED_CONFIRMATION {
        return Err(HwmonRealReadError::ConfirmationRequired);
    }
    ONE_SHOT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| HwmonRealReadError::AlreadyAttempted)?;

    let dmi_identity = approved_one_shot_dmi_identity_read(DMI_CONFIRMATION)
        .map_err(|error| HwmonRealReadError::Dmi(error.to_string()))?;
    if !is_exact_target_identity(&dmi_identity.identity) {
        return Err(HwmonRealReadError::IdentityMismatch);
    }

    let root = openat2(
        CWD,
        SYSFS_ROOT,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_SYMLINKS,
    )
    .map(File::from)
    .map_err(|error| HwmonRealReadError::OpenRoot(error.to_string()))?;
    let source = ConfinedHwmonSource::from_linux_root(root);
    let fans = source
        .read_once(true, None)
        .map_err(HwmonRealReadError::Fan)?;
    Ok(HwmonLinuxReading { dmi_identity, fans })
}

#[cfg(feature = "approved-real-hwmon-rpm-read")]
fn is_exact_target_identity(identity: &HostIdentity) -> bool {
    identity.manufacturer() == Some("Lenovo")
        && identity.product_identifier() == Some("83EX")
        && identity.model() == Some("Legion Slim 5 16ARP9")
        && identity.bios_version() == Some("NUCN15WW")
}

/// Confines every attribute open beneath an existing `/sys` descriptor. The
/// class `hwmonN` entries may legitimately be symlinks into `/sys/devices`; we
/// therefore allow intermediate symlinks but retain `RESOLVE_BENEATH` and
/// `RESOLVE_NO_MAGICLINKS`. `O_NOFOLLOW` protects the final attribute.
struct ConfinedHwmonSource {
    root: File,
    discovery_directory: PathBuf,
}

impl ConfinedHwmonSource {
    #[cfg(feature = "approved-real-hwmon-rpm-read")]
    fn from_linux_root(root: File) -> Self {
        Self {
            root,
            discovery_directory: PathBuf::from(HWMON_DISCOVERY_DIRECTORY),
        }
    }

    #[cfg(test)]
    fn from_test_root(root: File, root_path: &Path) -> Self {
        Self {
            root,
            discovery_directory: root_path.join(HWMON_RELATIVE_DIRECTORY),
        }
    }

    fn read_once(
        &self,
        target_identity_matched: bool,
        simulation_label: Option<&str>,
    ) -> Result<Vec<HwmonFanReading>, HwmonFanError> {
        let entries =
            fs::read_dir(&self.discovery_directory).map_err(|error| HwmonFanError::Source {
                device_id: "<discovery>".to_owned(),
                entry: "class/hwmon".to_owned(),
                error: map_io_error(&error),
            })?;
        let mut devices = entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| is_hwmon_directory_name(name))
            .collect::<Vec<_>>();
        devices.sort();
        if devices.is_empty() {
            return Err(HwmonFanError::NoHwmonDevices);
        }

        let mut readings = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for device_id in devices {
            let name = self.read_text_attribute(&device_id, "name")?;
            if !matches!(name.as_str(), "yogafan" | "acpi_fan") {
                continue;
            }
            let entries = self.list_device_entries(&device_id)?;
            for entry in entries {
                let Some(channel) = fan_input_channel(&entry) else {
                    continue;
                };
                if !seen.insert((name.clone(), channel)) {
                    return Err(HwmonFanError::DuplicateSource {
                        driver_name: name.clone(),
                        channel,
                    });
                }
                let raw = self.read_raw_attribute(&device_id, &entry)?;
                let rpm = parse_rpm(&raw).map_err(|reason| HwmonFanError::InvalidRpm {
                    device_id: device_id.clone(),
                    entry: entry.clone(),
                    reason,
                })?;
                let classification = if name == "yogafan" && target_identity_matched {
                    HwmonDriverClassification::Yogafan83ExCandidateUnverified
                } else {
                    HwmonDriverClassification::GenericUnvalidated
                };
                let reading = if let Some(label) = simulation_label {
                    build_reading(
                        name.clone(),
                        device_id.clone(),
                        channel,
                        rpm,
                        classification,
                        HwmonReadingOrigin::StaticSimulation { root_label: label },
                    )
                } else {
                    #[cfg(feature = "approved-real-hwmon-rpm-read")]
                    {
                        build_linux_hwmon_reading(
                            name.clone(),
                            device_id.clone(),
                            channel,
                            rpm,
                            classification,
                        )
                    }
                    #[cfg(not(feature = "approved-real-hwmon-rpm-read"))]
                    {
                        return Err(HwmonFanError::Source {
                            device_id: device_id.clone(),
                            entry: entry.clone(),
                            error: HwmonSourceError::ReadFailed {
                                reason: "real hwmon provider feature is disabled".to_owned(),
                            },
                        });
                    }
                };
                readings.push(reading);
            }
        }
        Ok(readings)
    }

    fn list_device_entries(&self, device_id: &str) -> Result<Vec<String>, HwmonFanError> {
        let path = self.discovery_directory.join(device_id);
        let entries = fs::read_dir(path).map_err(|error| HwmonFanError::Source {
            device_id: device_id.to_owned(),
            entry: "<entries>".to_owned(),
            error: map_io_error(&error),
        })?;
        let mut entries = entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect::<Vec<_>>();
        entries.sort();
        Ok(entries)
    }

    fn read_text_attribute(&self, device_id: &str, entry: &str) -> Result<String, HwmonFanError> {
        let bytes = self.read_raw_attribute(device_id, entry)?;
        parse_name(&bytes).map_err(|reason| HwmonFanError::InvalidRpm {
            device_id: device_id.to_owned(),
            entry: entry.to_owned(),
            reason,
        })
    }

    fn read_raw_attribute(&self, device_id: &str, entry: &str) -> Result<Vec<u8>, HwmonFanError> {
        let relative = format!("{HWMON_RELATIVE_DIRECTORY}/{device_id}/{entry}");
        let file = openat2(
            &self.root,
            &relative,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS,
        )
        .map(File::from)
        .map_err(|error| HwmonFanError::Source {
            device_id: device_id.to_owned(),
            entry: entry.to_owned(),
            error: map_openat2_error(error),
        })?;
        let stat = fstat(&file).map_err(|error| HwmonFanError::Source {
            device_id: device_id.to_owned(),
            entry: entry.to_owned(),
            error: HwmonSourceError::ReadFailed {
                reason: error.to_string(),
            },
        })?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(HwmonFanError::Source {
                device_id: device_id.to_owned(),
                entry: entry.to_owned(),
                error: HwmonSourceError::ReadFailed {
                    reason: "opened attribute is not a regular file".to_owned(),
                },
            });
        }
        read_to_eof(&file).map_err(|error| HwmonFanError::Source {
            device_id: device_id.to_owned(),
            entry: entry.to_owned(),
            error,
        })
    }
}

fn is_hwmon_directory_name(name: &str) -> bool {
    name.strip_prefix("hwmon").is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn read_to_eof(file: &File) -> Result<Vec<u8>, HwmonSourceError> {
    let mut output = Vec::new();
    loop {
        let mut chunk = [0_u8; 64];
        let remaining = MAXIMUM_HWMON_ATTRIBUTE_BYTES + 1 - output.len();
        let chunk_length = chunk.len();
        let count = read(file, &mut chunk[..remaining.min(chunk_length)]).map_err(|error| {
            if error == Errno::INTR {
                HwmonSourceError::ReadFailed {
                    reason: "read interrupted before EOF".to_owned(),
                }
            } else {
                HwmonSourceError::ReadFailed {
                    reason: error.to_string(),
                }
            }
        })?;
        if count == 0 {
            break;
        }
        output.extend_from_slice(&chunk[..count]);
        if output.len() > MAXIMUM_HWMON_ATTRIBUTE_BYTES {
            return Err(HwmonSourceError::ReadFailed {
                reason: format!("attribute exceeds {MAXIMUM_HWMON_ATTRIBUTE_BYTES} bytes"),
            });
        }
    }
    Ok(output)
}

fn map_openat2_error(error: Errno) -> HwmonSourceError {
    match error {
        Errno::NOENT => HwmonSourceError::Missing,
        Errno::ACCESS | Errno::PERM => HwmonSourceError::PermissionDenied,
        Errno::NOSYS => HwmonSourceError::ReadFailed {
            reason: "openat2 unavailable".to_owned(),
        },
        Errno::LOOP | Errno::XDEV | Errno::AGAIN => HwmonSourceError::ReadFailed {
            reason: format!("openat2 could not guarantee confined resolution: {error}"),
        },
        _ => HwmonSourceError::ReadFailed {
            reason: format!("openat2 failed: {error}"),
        },
    }
}

fn map_io_error(error: &std::io::Error) -> HwmonSourceError {
    match error.kind() {
        std::io::ErrorKind::NotFound => HwmonSourceError::Missing,
        std::io::ErrorKind::PermissionDenied => HwmonSourceError::PermissionDenied,
        _ => HwmonSourceError::ReadFailed {
            reason: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::symlink,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn root(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "legion-hwmon-{name}-{}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("class/hwmon/hwmon7")).unwrap();
        path
    }

    fn source(path: &Path) -> ConfinedHwmonSource {
        ConfinedHwmonSource::from_test_root(File::open(path).unwrap(), path)
    }

    #[test]
    fn reads_dynamic_channels_from_a_confined_fixture_tree() {
        let path = root("two");
        let device = path.join("class/hwmon/hwmon7");
        fs::write(device.join("name"), b"acpi_fan\n").unwrap();
        fs::write(device.join("fan1_input"), b"1900\n").unwrap();
        fs::write(device.join("fan4_input"), b"0\n").unwrap();
        let readings = source(&path)
            .read_once(false, Some("isolated-test"))
            .unwrap();
        assert_eq!(readings.len(), 2);
        assert_eq!(readings[1].channel, 4);
        assert_eq!(readings[1].rpm.value, Some(0));
        assert_eq!(
            readings[0].rpm.provenance.artifact,
            legion_core::TelemetryArtifact::StaticSimulation
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn rejects_symlinked_final_entries_and_invalid_data() {
        let path = root("unsafe");
        let device = path.join("class/hwmon/hwmon7");
        fs::write(device.join("name"), b"yogafan\n").unwrap();
        symlink("/etc/passwd", device.join("fan1_input")).unwrap();
        assert!(matches!(
            source(&path).read_once(true, Some("isolated-test")),
            Err(HwmonFanError::Source { .. })
        ));
        fs::remove_dir_all(path).unwrap();
    }
}
