//! Feature-gated, fixed-path Linux hwmon RPM observation.
//!
//! The production entry point is unavailable unless
//! `approved-real-hwmon-rpm-read` is explicitly selected. It accepts no path,
//! configuration, environment, retry, monitor, or write option. A separately
//! gated bounded rehearsal reuses its private acquisition helpers. It first
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
    HwmonDriverClassification, HwmonFanError, HwmonFanReading, HwmonModuleProvenance,
    HwmonReadingOrigin, HwmonSourceError, build_reading, fan_input_channel, parse_name, parse_rpm,
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
pub(crate) fn is_exact_target_identity(identity: &HostIdentity) -> bool {
    identity.manufacturer() == Some("Lenovo")
        && identity.product_identifier() == Some("83EX")
        && identity.model() == Some("Legion Slim 5 16ARP9")
        && identity.bios_version() == Some("NUCN15WW")
}

/// Confines every attribute open beneath an existing `/sys` descriptor. The
/// class `hwmonN` entries may legitimately be symlinks into `/sys/devices`; we
/// therefore allow intermediate symlinks but retain `RESOLVE_BENEATH` and
/// `RESOLVE_NO_MAGICLINKS`. `O_NOFOLLOW` protects the final attribute.
pub(crate) struct ConfinedHwmonSource {
    root: File,
    discovery_directory: PathBuf,
}

#[cfg(any(test, feature = "experimental-fan-trial-monitor"))]
#[derive(Debug, serde::Serialize)]
pub(crate) struct SupervisionSysfsReading {
    pub cpu_c: f64,
    pub cpu_original: Vec<u8>,
    pub cpu_source: String,
    pub fans: [HwmonFanReading; 2],
    pub rpm_original: [Vec<u8>; 2],
    pub ac_online: bool,
    pub ac_original: Vec<u8>,
}

#[cfg(feature = "experimental-fan-trial-monitor")]
pub(crate) struct TrialSysfsSource(ConfinedHwmonSource);

#[cfg(feature = "experimental-fan-trial-monitor")]
impl TrialSysfsSource {
    pub(crate) fn open() -> Result<Self, String> {
        let root =
            crate::open_fixed_sysfs_root().map_err(|error| format!("sysfs root: {error:?}"))?;
        Ok(Self(ConfinedHwmonSource::from_linux_root(root)))
    }
    pub(crate) fn read_once(&self) -> Result<SupervisionSysfsReading, String> {
        self.0.read_supervision_once(None)
    }
}

impl ConfinedHwmonSource {
    #[cfg(feature = "approved-real-hwmon-rpm-read")]
    pub(crate) fn from_linux_root(root: File) -> Self {
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

    pub(crate) fn read_once(
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
                let module_provenance = self.module_provenance(&device_id);
                let classification =
                    classify_driver(&name, target_identity_matched, &module_provenance);
                let reading = if let Some(label) = simulation_label {
                    build_reading(
                        name.clone(),
                        device_id.clone(),
                        channel,
                        rpm,
                        classification,
                        module_provenance,
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
                            module_provenance,
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
        self.read_relative_attribute(&relative, device_id, entry)
    }

    fn read_relative_attribute(
        &self,
        relative: &str,
        device_id: &str,
        entry: &str,
    ) -> Result<Vec<u8>, HwmonFanError> {
        let file = openat2(
            &self.root,
            relative,
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

    /// Fixed monitor acquisition: reject original/ambiguous yogafan BEFORE
    /// fan input reads. acpi_fan is not touched by this collector.
    #[cfg(any(test, feature = "experimental-fan-trial-monitor"))]
    fn read_supervision_once(
        &self,
        simulation_label: Option<&str>,
    ) -> Result<SupervisionSysfsReading, String> {
        let mut cpu_devices = Vec::new();
        let mut fan_devices = Vec::new();
        for entry in fs::read_dir(&self.discovery_directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "non-UTF8 hwmon entry")?;
            if !is_hwmon_directory_name(&name) {
                continue;
            }
            let driver = self
                .read_text_attribute(&name, "name")
                .map_err(|e| e.to_string())?;
            if driver == "k10temp" {
                cpu_devices.push(name.clone());
            }
            if driver == "yogafan" {
                fan_devices.push(name);
            }
        }
        if cpu_devices.len() != 1 || fan_devices.len() != 1 {
            return Err(
                "exactly one k10temp and one identified yogafan_83ex source required".into(),
            );
        }
        let cpu_device = &cpu_devices[0];
        let fan_device = &fan_devices[0];
        let module = self.module_provenance(fan_device);
        if classify_driver("yogafan", true, &module)
            != HwmonDriverClassification::Verified83ExExperimentalQuirk
        {
            return Err("yogafan_83ex not bound; original/missing/ambiguous provider rejected before RPM read; no automatic substitution".into());
        }
        let entries = self
            .list_device_entries(fan_device)
            .map_err(|e| e.to_string())?;
        let channels: Vec<_> = entries
            .iter()
            .filter_map(|e| fan_input_channel(e))
            .collect();
        if channels != [1, 2] {
            return Err("quirk must expose exactly fan1_input and fan2_input".into());
        }
        let (cpu_c, cpu_original, cpu_source) = self.read_cpu_device_once(cpu_device)?;
        let read_fan = |channel| -> Result<(HwmonFanReading, Vec<u8>), String> {
            let entry = format!("fan{channel}_input");
            let original = self
                .read_raw_attribute(fan_device, &entry)
                .map_err(|e| e.to_string())?;
            let rpm = parse_rpm(&original)?;
            let reading = if let Some(label) = simulation_label {
                build_reading(
                    "yogafan".into(),
                    fan_device.clone(),
                    channel,
                    rpm,
                    HwmonDriverClassification::Validated83ExQuirkSimulation,
                    HwmonModuleProvenance::StaticSimulation,
                    HwmonReadingOrigin::StaticSimulation { root_label: label },
                )
            } else {
                #[cfg(feature = "experimental-fan-trial-monitor")]
                {
                    build_linux_hwmon_reading(
                        "yogafan".into(),
                        fan_device.clone(),
                        channel,
                        rpm,
                        HwmonDriverClassification::Verified83ExExperimentalQuirk,
                        module.clone(),
                    )
                }
                #[cfg(not(feature = "experimental-fan-trial-monitor"))]
                {
                    return Err("live monitor feature disabled".into());
                }
            };
            Ok((reading, original))
        };
        let (fan1, raw1) = read_fan(1)?;
        let (fan2, raw2) = read_fan(2)?;
        if self.module_provenance(fan_device) != module {
            return Err("quirk module provenance changed during sample".into());
        }
        let (ac_online, ac_original) = self.read_ac_once()?;
        Ok(SupervisionSysfsReading {
            cpu_c,
            cpu_original,
            cpu_source,
            fans: [fan1, fan2],
            rpm_original: [raw1, raw2],
            ac_online,
            ac_original,
        })
    }

    #[cfg(any(
        test,
        feature = "linux-status",
        feature = "experimental-fan-trial-monitor"
    ))]
    fn read_cpu_device_once(&self, cpu_device: &str) -> Result<(f64, Vec<u8>, String), String> {
        let mut labels = Vec::new();
        for entry in self
            .list_device_entries(cpu_device)
            .map_err(|e| e.to_string())?
        {
            if entry.len() > 10
                && entry.starts_with("temp")
                && entry.ends_with("_label")
                && entry[4..entry.len() - 6]
                    .bytes()
                    .all(|c| c.is_ascii_digit())
                && self
                    .read_text_attribute(cpu_device, &entry)
                    .map_err(|e| e.to_string())?
                    == "Tctl"
            {
                labels.push(entry.replace("_label", "_input"));
            }
        }
        if labels.len() != 1 {
            return Err("exactly one k10temp:Tctl label required".into());
        }
        let cpu_original = self
            .read_raw_attribute(cpu_device, &labels[0])
            .map_err(|e| e.to_string())?;
        let milli = parse_rpm(&cpu_original)?;
        if !(5000..=110000).contains(&milli) {
            return Err("Tctl outside trial range".into());
        }
        Ok((
            f64::from(milli) / 1000.0,
            cpu_original,
            format!("k10temp:Tctl:{cpu_device}:{}", labels[0]),
        ))
    }

    #[cfg(any(
        test,
        feature = "linux-status",
        feature = "experimental-fan-trial-monitor"
    ))]
    pub(crate) fn read_ac_once(&self) -> Result<(bool, Vec<u8>), String> {
        let supply_type = self
            .read_relative_attribute("class/power_supply/ACAD/type", "ACAD", "type")
            .map_err(|e| e.to_string())?;
        if supply_type != b"Mains\n" && supply_type != b"Mains" {
            return Err("ACAD is not Mains".into());
        }
        let ac_original = self
            .read_relative_attribute("class/power_supply/ACAD/online", "ACAD", "online")
            .map_err(|e| e.to_string())?;
        let ac_online = match ac_original.as_slice() {
            b"1\n" | b"1" => true,
            b"0\n" | b"0" => false,
            _ => return Err("invalid ACAD/online".into()),
        };
        Ok((ac_online, ac_original))
    }

    #[cfg(any(test, feature = "linux-status"))]
    pub(crate) fn read_status_cpu_once(&self) -> Result<(f64, Vec<u8>, String), String> {
        let mut devices = Vec::new();
        for entry in fs::read_dir(&self.discovery_directory).map_err(|e| e.to_string())? {
            let name = entry
                .map_err(|e| e.to_string())?
                .file_name()
                .into_string()
                .map_err(|_| "non-UTF8 hwmon entry")?;
            if is_hwmon_directory_name(&name)
                && self
                    .read_text_attribute(&name, "name")
                    .map_err(|e| e.to_string())?
                    == "k10temp"
            {
                devices.push(name);
            }
        }
        if devices.len() != 1 {
            return Err("exactly one k10temp source required; no guess or fallback".into());
        }
        self.read_cpu_device_once(&devices[0])
    }

    /// Reads only the final component of the fixed sysfs `module` link. The
    /// link is evidence about driver provenance, not a writable control path.
    /// Failure to obtain it does not manufacture a module identity.
    fn module_provenance(&self, device_id: &str) -> HwmonModuleProvenance {
        let link = self
            .discovery_directory
            .join(device_id)
            .join("device/driver/module");
        let Ok(target) = fs::read_link(link) else {
            return HwmonModuleProvenance::Missing;
        };
        let Some(name) = target.file_name().and_then(|name| name.to_str()) else {
            return HwmonModuleProvenance::Missing;
        };
        match name {
            "yogafan_83ex" => HwmonModuleProvenance::Verified83ExExperimentalQuirk {
                module_name: name.to_owned(),
            },
            "yogafan" => HwmonModuleProvenance::OriginalYogafan {
                module_name: name.to_owned(),
            },
            _ => HwmonModuleProvenance::Unexpected {
                module_name: name.to_owned(),
            },
        }
    }
}

fn classify_driver(
    driver_name: &str,
    target_identity_matched: bool,
    module_provenance: &HwmonModuleProvenance,
) -> HwmonDriverClassification {
    match (driver_name, target_identity_matched, module_provenance) {
        ("yogafan", true, HwmonModuleProvenance::Verified83ExExperimentalQuirk { .. }) => {
            HwmonDriverClassification::Verified83ExExperimentalQuirk
        }
        ("yogafan", true, _) => HwmonDriverClassification::Yogafan83ExCandidateUnverified,
        _ => HwmonDriverClassification::GenericUnvalidated,
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

    #[test]
    fn distinguishes_verified_quirk_original_and_ambiguous_module_evidence() {
        let path = root("module-evidence");
        let device = path.join("class/hwmon/hwmon7");
        fs::create_dir_all(device.join("device/driver")).unwrap();
        symlink(
            "/sys/module/yogafan_83ex",
            device.join("device/driver/module"),
        )
        .unwrap();
        let source = source(&path);
        let verified = source.module_provenance("hwmon7");
        assert!(matches!(
            verified,
            HwmonModuleProvenance::Verified83ExExperimentalQuirk { .. }
        ));
        assert_eq!(
            classify_driver("yogafan", true, &verified),
            HwmonDriverClassification::Verified83ExExperimentalQuirk
        );
        let original = HwmonModuleProvenance::OriginalYogafan {
            module_name: "yogafan".to_owned(),
        };
        assert_eq!(
            classify_driver("yogafan", true, &original),
            HwmonDriverClassification::Yogafan83ExCandidateUnverified
        );
        assert_eq!(
            classify_driver("yogafan", true, &HwmonModuleProvenance::Missing),
            HwmonDriverClassification::Yogafan83ExCandidateUnverified
        );
        fs::remove_dir_all(path).unwrap();
    }

    fn supervision_fixture(name: &str) -> PathBuf {
        let path = root(name);
        let fan = path.join("class/hwmon/hwmon7");
        fs::write(fan.join("name"), b"yogafan\n").unwrap();
        fs::create_dir_all(fan.join("device/driver")).unwrap();
        symlink("/sys/module/yogafan_83ex", fan.join("device/driver/module")).unwrap();
        fs::write(fan.join("fan1_input"), b"1500\n").unwrap();
        fs::write(fan.join("fan2_input"), b"1500\n").unwrap();
        let cpu = path.join("class/hwmon/hwmon9");
        fs::create_dir_all(&cpu).unwrap();
        fs::write(cpu.join("name"), b"k10temp\n").unwrap();
        fs::write(cpu.join("temp1_label"), b"Tctl\n").unwrap();
        fs::write(cpu.join("temp1_input"), b"43500\n").unwrap();
        let ac = path.join("class/power_supply/ACAD");
        fs::create_dir_all(&ac).unwrap();
        fs::write(ac.join("type"), b"Mains\n").unwrap();
        fs::write(ac.join("online"), b"1\n").unwrap();
        path
    }

    #[test]
    fn supervision_collects_only_identified_quirk_tctl_and_ac_in_fixtures() {
        let path = supervision_fixture("supervision");
        let reading = source(&path)
            .read_supervision_once(Some("isolated-supervision"))
            .unwrap();
        assert_eq!(reading.cpu_c, 43.5);
        assert_eq!(reading.cpu_original, b"43500\n");
        assert_eq!(reading.fans[0].rpm.value, Some(1500));
        assert!(reading.ac_online);
        assert_eq!(
            reading.fans[1].rpm.provenance.artifact,
            legion_core::TelemetryArtifact::StaticSimulation
        );
        assert_eq!(reading.rpm_original[1], b"1500\n");
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn status_reuses_tctl_ac_and_keeps_original_and_acpi_fans_distinct() {
        let path = supervision_fixture("status");
        let fan = path.join("class/hwmon/hwmon7");
        fs::remove_file(fan.join("device/driver/module")).unwrap();
        symlink("/sys/module/yogafan", fan.join("device/driver/module")).unwrap();
        fs::write(fan.join("fan1_input"), b"0\n").unwrap();
        fs::write(fan.join("fan2_input"), b"0\n").unwrap();
        let acpi = path.join("class/hwmon/hwmon15");
        fs::create_dir_all(&acpi).unwrap();
        fs::write(acpi.join("name"), b"acpi_fan\n").unwrap();
        fs::write(acpi.join("fan1_input"), b"1500\n").unwrap();
        fs::write(path.join("class/power_supply/ACAD/online"), b"0\n").unwrap();
        let source = source(&path);
        assert_eq!(source.read_status_cpu_once().unwrap().0, 43.5);
        assert!(!source.read_ac_once().unwrap().0);
        let fans = source.read_once(true, Some("status-simulation")).unwrap();
        assert_eq!(fans.len(), 3);
        let original = fans.iter().find(|f| f.driver_name == "yogafan").unwrap();
        assert_eq!(original.rpm.value, Some(0));
        assert_eq!(original.validation, legion_core::FanValidation::Unvalidated);
        assert_eq!(
            original.interpretation,
            legion_core::FanInterpretation::NonConclusive
        );
        assert_eq!(
            fans.iter()
                .find(|f| f.driver_name == "acpi_fan")
                .unwrap()
                .rpm
                .value,
            Some(1500)
        );
        fs::write(path.join("class/hwmon/hwmon9/temp1_input"), b"invalid\n").unwrap();
        assert!(source.read_status_cpu_once().is_err());
        fs::remove_file(path.join("class/power_supply/ACAD/online")).unwrap();
        assert!(source.read_ac_once().is_err());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn supervision_rejects_original_before_any_rpm_input_or_fallback() {
        let path = supervision_fixture("supervision-original");
        let fan = path.join("class/hwmon/hwmon7");
        fs::remove_file(fan.join("device/driver/module")).unwrap();
        symlink("/sys/module/yogafan", fan.join("device/driver/module")).unwrap();
        fs::remove_file(fan.join("fan1_input")).unwrap();
        assert!(
            source(&path)
                .read_supervision_once(Some("fixture"))
                .unwrap_err()
                .contains("before RPM read")
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn supervision_rejects_disappearing_channels_and_invalid_cpu_data() {
        let path = supervision_fixture("supervision-invalid");
        fs::write(path.join("class/hwmon/hwmon9/temp1_input"), b"N/A\n").unwrap();
        assert!(
            source(&path)
                .read_supervision_once(Some("fixture"))
                .is_err()
        );
        fs::write(path.join("class/hwmon/hwmon9/temp1_input"), b"43500\n").unwrap();
        fs::remove_file(path.join("class/hwmon/hwmon7/fan2_input")).unwrap();
        assert!(
            source(&path)
                .read_supervision_once(Some("fixture"))
                .unwrap_err()
                .contains("exactly fan1_input")
        );
        fs::remove_dir_all(path).unwrap();
    }
}
