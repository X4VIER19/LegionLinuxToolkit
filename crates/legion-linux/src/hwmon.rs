//! Isolated `hwmon` fan-RPM discovery.
//!
//! This module deliberately models the documented `/sys/class/hwmon` shape
//! through an injected, sealed byte source.  The only source implemented in
//! this phase is an in-memory simulation: there is no host-path constructor,
//! no environment lookup, no CLI path argument, and no fallback to `/sys`.
//! A future real source must be separately reviewed and explicitly gated.

use legion_core::{
    ConfidenceLevel, FanInterpretation, FanValidation, TelemetryArtifact, TelemetryProvenance,
    TelemetryState, TelemetryUnit, TelemetryValue, ValueClassification,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

const HWMON_NAME: &str = "name";
const INPUT_SUFFIX: &str = "_input";

/// How the driver identity was established for this *simulated* source.
///
/// `yogafan` by itself is never enough to select `Validated83ExQuirk`: the
/// latter represents fixtures that explicitly model the separately reviewed
/// 83EX FA1S/FA2S ×100 quirk.  A future host implementation must establish an
/// equivalent independently of the hwmon `name` before it may use that label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HwmonDriverClassification {
    #[serde(rename = "validated-83ex-quirk-simulation")]
    Validated83ExQuirkSimulation,
    /// Exact 83EX DMI plus the hwmon `yogafan` name identify only a candidate:
    /// sysfs does not prove that the reviewed FA1S/FA2S ×100 quirk is loaded.
    Yogafan83ExCandidateUnverified,
    GenericUnvalidated,
}

/// One discovered, channel-neutral fan input. `channel` is the hwmon channel
/// number; it is intentionally not assigned to CPU or GPU.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HwmonFanReading {
    pub driver_name: String,
    /// A simulated discovery identifier, not a host path or a stable hwmon
    /// number.
    pub device_id: String,
    pub channel: u32,
    pub source_classification: HwmonDriverClassification,
    pub rpm: TelemetryValue<u32>,
    pub validation: FanValidation,
    pub interpretation: FanInterpretation,
}

/// Errors from discovery/acquisition remain distinct from an invalid numeric
/// representation and duplicate source identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum HwmonFanError {
    NoHwmonDevices,
    Source {
        device_id: String,
        entry: String,
        error: HwmonSourceError,
    },
    InvalidRpm {
        device_id: String,
        entry: String,
        reason: String,
    },
    DuplicateSource {
        driver_name: String,
        channel: u32,
    },
}

impl fmt::Display for HwmonFanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHwmonDevices => write!(f, "no hwmon devices were discovered"),
            Self::Source {
                device_id,
                entry,
                error,
            } => {
                write!(f, "hwmon source {device_id}/{entry} failed: {error}")
            }
            Self::InvalidRpm {
                device_id,
                entry,
                reason,
            } => {
                write!(f, "invalid simulated RPM in {device_id}/{entry}: {reason}")
            }
            Self::DuplicateSource {
                driver_name,
                channel,
            } => write!(
                f,
                "duplicate simulated hwmon source for {driver_name} fan{channel}_input"
            ),
        }
    }
}

impl std::error::Error for HwmonFanError {}

/// Query interface shared by a future GUI or CLI. It is intentionally a
/// single explicit operation: it has no timer, retry loop, or monitoring API.
pub trait HwmonFanQuery: private::Sealed {
    fn read_once(&self) -> Result<Vec<HwmonFanReading>, HwmonFanError>;
}

/// Linux-shaped reader over a sealed byte source. It has no public host
/// filesystem constructor in this phase.
#[derive(Debug, Clone)]
pub struct LinuxHwmonFanReader {
    source: SimulatedHwmonTree,
}

impl LinuxHwmonFanReader {
    pub fn new(source: SimulatedHwmonTree) -> Self {
        Self { source }
    }
}

impl private::Sealed for LinuxHwmonFanReader {}

impl HwmonFanQuery for LinuxHwmonFanReader {
    fn read_once(&self) -> Result<Vec<HwmonFanReading>, HwmonFanError> {
        let devices = self
            .source
            .list_devices()
            .map_err(|error| HwmonFanError::Source {
                device_id: "<discovery>".to_owned(),
                entry: "<devices>".to_owned(),
                error,
            })?;
        if devices.is_empty() {
            return Err(HwmonFanError::NoHwmonDevices);
        }

        let mut readings = Vec::new();
        let mut seen = BTreeSet::new();
        for device_id in devices {
            let name = self.read_name(&device_id)?;
            if !matches!(name.as_str(), "yogafan" | "acpi_fan") {
                continue;
            }

            let entries =
                self.source
                    .list_entries(&device_id)
                    .map_err(|error| HwmonFanError::Source {
                        device_id: device_id.clone(),
                        entry: "<entries>".to_owned(),
                        error,
                    })?;
            for (channel, entry) in entries
                .iter()
                .filter_map(|entry| fan_input_channel(entry).map(|channel| (channel, entry)))
            {
                if !seen.insert((name.clone(), channel)) {
                    return Err(HwmonFanError::DuplicateSource {
                        driver_name: name.clone(),
                        channel,
                    });
                }
                let raw = self.source.read_entry(&device_id, entry).map_err(|error| {
                    HwmonFanError::Source {
                        device_id: device_id.clone(),
                        entry: entry.clone(),
                        error,
                    }
                })?;
                let rpm = parse_rpm(&raw).map_err(|reason| HwmonFanError::InvalidRpm {
                    device_id: device_id.clone(),
                    entry: entry.clone(),
                    reason,
                })?;
                let classification = self.source.classification(&device_id).map_err(|error| {
                    HwmonFanError::Source {
                        device_id: device_id.clone(),
                        entry: "<classification>".to_owned(),
                        error,
                    }
                })?;
                readings.push(build_reading(
                    name.clone(),
                    device_id.clone(),
                    channel,
                    rpm,
                    classification,
                    HwmonReadingOrigin::StaticSimulation {
                        root_label: self.source.root_label(),
                    },
                ));
            }
        }
        Ok(readings)
    }
}

impl LinuxHwmonFanReader {
    fn read_name(&self, device_id: &str) -> Result<String, HwmonFanError> {
        let raw = self
            .source
            .read_entry(device_id, HWMON_NAME)
            .map_err(|error| HwmonFanError::Source {
                device_id: device_id.to_owned(),
                entry: HWMON_NAME.to_owned(),
                error,
            })?;
        parse_name(&raw).map_err(|reason| HwmonFanError::InvalidRpm {
            device_id: device_id.to_owned(),
            entry: HWMON_NAME.to_owned(),
            reason,
        })
    }
}

pub(crate) fn build_reading(
    driver_name: String,
    device_id: String,
    channel: u32,
    value: u32,
    source_classification: HwmonDriverClassification,
    origin: HwmonReadingOrigin<'_>,
) -> HwmonFanReading {
    let (state, validation, interpretation, confidence) = match source_classification {
        HwmonDriverClassification::Validated83ExQuirkSimulation => (
            TelemetryState::Available,
            FanValidation::Validated,
            FanInterpretation::ReportedValue,
            ConfidenceLevel::Low,
        ),
        // A generic `yogafan` source is deliberately not promoted by its
        // `name`: 83EX's original driver path is known to be incompatible.
        HwmonDriverClassification::GenericUnvalidated => (
            TelemetryState::Unvalidated,
            FanValidation::Unvalidated,
            FanInterpretation::NonConclusive,
            ConfidenceLevel::Low,
        ),
        HwmonDriverClassification::Yogafan83ExCandidateUnverified => (
            TelemetryState::Unvalidated,
            FanValidation::Unvalidated,
            FanInterpretation::NonConclusive,
            ConfidenceLevel::Low,
        ),
    };
    let (artifact, source) = match origin {
        HwmonReadingOrigin::StaticSimulation { root_label } => (
            TelemetryArtifact::StaticSimulation,
            format!(
                "legion-linux::simulated-hwmon root={root_label}; driver={driver_name}; device={device_id}; channel=fan{channel}_input"
            ),
        ),
        #[cfg(feature = "approved-real-hwmon-rpm-read")]
        HwmonReadingOrigin::LinuxReadOnly => (
            TelemetryArtifact::LinuxReadOnly,
            format!(
                "linux-hwmon-read-only; driver={driver_name}; device={device_id}; channel=fan{channel}_input"
            ),
        ),
    };
    HwmonFanReading {
        driver_name: driver_name.clone(),
        device_id: device_id.clone(),
        channel,
        source_classification,
        rpm: TelemetryValue {
            value: Some(value),
            unit: TelemetryUnit::Rpm,
            classification: ValueClassification::InstantaneousMeasurement,
            state,
            provenance: TelemetryProvenance {
                artifact,
                source,
                confidence,
            },
        },
        validation,
        interpretation,
    }
}

/// Only crate-internal production code can select `LinuxReadOnly`; the public
/// in-memory fixture constructors always select `StaticSimulation`.
pub(crate) enum HwmonReadingOrigin<'a> {
    StaticSimulation {
        root_label: &'a str,
    },
    #[cfg(feature = "approved-real-hwmon-rpm-read")]
    LinuxReadOnly,
}

#[cfg(feature = "approved-real-hwmon-rpm-read")]
pub(crate) fn build_linux_hwmon_reading(
    driver_name: String,
    device_id: String,
    channel: u32,
    value: u32,
    source_classification: HwmonDriverClassification,
) -> HwmonFanReading {
    build_reading(
        driver_name,
        device_id,
        channel,
        value,
        source_classification,
        HwmonReadingOrigin::LinuxReadOnly,
    )
}

pub(crate) fn parse_name(bytes: &[u8]) -> Result<String, String> {
    let value = std::str::from_utf8(bytes).map_err(|_| "name is not valid UTF-8".to_owned())?;
    let value = value.strip_suffix('\n').unwrap_or(value);
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err("name is empty or contains an unexpected control character".to_owned());
    }
    Ok(value.to_owned())
}

pub(crate) fn parse_rpm(bytes: &[u8]) -> Result<u32, String> {
    let value = std::str::from_utf8(bytes).map_err(|_| "input is not valid UTF-8".to_owned())?;
    let value = value.strip_suffix('\n').unwrap_or(value);
    if value.is_empty()
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(
            "input is empty or contains unexpected whitespace/control characters".to_owned(),
        );
    }
    value
        .parse::<u32>()
        .map_err(|_| "input is not an unsigned RPM integer".to_owned())
}

pub(crate) fn fan_input_channel(entry: &str) -> Option<u32> {
    let digits = entry.strip_prefix("fan")?.strip_suffix(INPUT_SUFFIX)?;
    let channel = digits.parse::<u32>().ok()?;
    (channel > 0).then_some(channel)
}

/// All acquisition outcomes the in-memory source can model. They stay
/// explicit rather than being silently converted into RPM zero or absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum HwmonSourceError {
    Missing,
    PermissionDenied,
    DeviceDisappeared,
    ReadFailed { reason: String },
}

impl fmt::Display for HwmonSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => write!(f, "entry is absent"),
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::DeviceDisappeared => write!(f, "device disappeared during discovery"),
            Self::ReadFailed { reason } => write!(f, "read failed: {reason}"),
        }
    }
}

/// In-memory entry contents and explicit source failures for isolated tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimulatedHwmonEntry {
    Regular(Vec<u8>),
    Missing,
    PermissionDenied,
    DeviceDisappeared,
    ReadFailed(String),
}

#[derive(Debug, Clone)]
pub struct SimulatedHwmonDevice {
    classification: HwmonDriverClassification,
    entries: BTreeMap<String, SimulatedHwmonEntry>,
}

impl SimulatedHwmonDevice {
    pub fn generic(name: impl Into<Vec<u8>>) -> Self {
        Self::new(HwmonDriverClassification::GenericUnvalidated, name)
    }

    pub fn validated_83ex_quirk(name: impl Into<Vec<u8>>) -> Self {
        Self::new(
            HwmonDriverClassification::Validated83ExQuirkSimulation,
            name,
        )
    }

    fn new(classification: HwmonDriverClassification, name: impl Into<Vec<u8>>) -> Self {
        Self {
            classification,
            entries: BTreeMap::from([(
                HWMON_NAME.to_owned(),
                SimulatedHwmonEntry::Regular(name.into()),
            )]),
        }
    }

    pub fn with_entry(mut self, name: impl Into<String>, value: SimulatedHwmonEntry) -> Self {
        self.entries.insert(name.into(), value);
        self
    }
}

/// A named in-memory hwmon tree. It cannot reference a host directory.
#[derive(Debug, Clone)]
pub struct SimulatedHwmonTree {
    root_label: String,
    devices: BTreeMap<String, SimulatedHwmonDevice>,
}

impl SimulatedHwmonTree {
    pub fn new(root_label: impl Into<String>) -> Self {
        Self {
            root_label: root_label.into(),
            devices: BTreeMap::new(),
        }
    }

    pub fn with_device(
        mut self,
        device_id: impl Into<String>,
        device: SimulatedHwmonDevice,
    ) -> Self {
        self.devices.insert(device_id.into(), device);
        self
    }
}

mod private {
    pub trait Sealed {}
}

impl SimulatedHwmonTree {
    fn list_devices(&self) -> Result<Vec<String>, HwmonSourceError> {
        Ok(self.devices.keys().cloned().collect())
    }

    fn list_entries(&self, device_id: &str) -> Result<Vec<String>, HwmonSourceError> {
        let device = self
            .devices
            .get(device_id)
            .ok_or(HwmonSourceError::Missing)?;
        Ok(device.entries.keys().cloned().collect())
    }

    fn read_entry(&self, device_id: &str, entry: &str) -> Result<Vec<u8>, HwmonSourceError> {
        let device = self
            .devices
            .get(device_id)
            .ok_or(HwmonSourceError::Missing)?;
        match device.entries.get(entry).ok_or(HwmonSourceError::Missing)? {
            SimulatedHwmonEntry::Regular(value) => Ok(value.clone()),
            SimulatedHwmonEntry::Missing => Err(HwmonSourceError::Missing),
            SimulatedHwmonEntry::PermissionDenied => Err(HwmonSourceError::PermissionDenied),
            SimulatedHwmonEntry::DeviceDisappeared => Err(HwmonSourceError::DeviceDisappeared),
            SimulatedHwmonEntry::ReadFailed(reason) => Err(HwmonSourceError::ReadFailed {
                reason: reason.clone(),
            }),
        }
    }

    fn classification(
        &self,
        device_id: &str,
    ) -> Result<HwmonDriverClassification, HwmonSourceError> {
        self.devices
            .get(device_id)
            .map(|device| device.classification)
            .ok_or(HwmonSourceError::Missing)
    }

    fn root_label(&self) -> &str {
        &self.root_label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(tree: SimulatedHwmonTree) -> Result<Vec<HwmonFanReading>, HwmonFanError> {
        LinuxHwmonFanReader::new(tree).read_once()
    }

    #[test]
    fn discovers_two_explicitly_modelled_83ex_quirk_channels() {
        let readings = query(
            SimulatedHwmonTree::new("two-channel").with_device(
                "synthetic-a",
                SimulatedHwmonDevice::validated_83ex_quirk(b"yogafan\n".to_vec())
                    .with_entry(
                        "fan1_input",
                        SimulatedHwmonEntry::Regular(b"1900\n".to_vec()),
                    )
                    .with_entry(
                        "fan2_input",
                        SimulatedHwmonEntry::Regular(b"1900\n".to_vec()),
                    ),
            ),
        )
        .unwrap();
        assert_eq!(readings.len(), 2);
        assert_eq!(readings[0].channel, 1);
        assert_eq!(readings[1].channel, 2);
        assert_eq!(readings[0].rpm.value, Some(1900));
        assert_eq!(
            readings[0].rpm.provenance.artifact,
            TelemetryArtifact::StaticSimulation
        );
        assert_eq!(readings[0].validation, FanValidation::Validated);
    }

    #[test]
    fn generic_yogafan_zero_is_numeric_but_not_a_stopped_fan() {
        let readings = query(
            SimulatedHwmonTree::new("zero").with_device(
                "synthetic-a",
                SimulatedHwmonDevice::generic(b"yogafan\n".to_vec())
                    .with_entry("fan1_input", SimulatedHwmonEntry::Regular(b"0\n".to_vec())),
            ),
        )
        .unwrap();
        assert_eq!(readings[0].rpm.value, Some(0));
        assert_eq!(readings[0].rpm.state, TelemetryState::Unvalidated);
        assert_eq!(readings[0].interpretation, FanInterpretation::NonConclusive);
    }

    #[test]
    fn discovers_one_channel_without_assuming_two() {
        let readings = query(SimulatedHwmonTree::new("one-channel").with_device(
            "synthetic-a",
            SimulatedHwmonDevice::generic(b"acpi_fan\n".to_vec()).with_entry(
                "fan3_input",
                SimulatedHwmonEntry::Regular(b"1900\n".to_vec()),
            ),
        ))
        .unwrap();
        assert_eq!(readings.len(), 1);
        assert_eq!(readings[0].channel, 3);
    }

    #[test]
    fn distinguishes_invalid_values_and_source_errors() {
        let invalid = query(SimulatedHwmonTree::new("invalid").with_device(
            "synthetic-a",
            SimulatedHwmonDevice::generic(b"acpi_fan\n".to_vec()).with_entry(
                "fan1_input",
                SimulatedHwmonEntry::Regular(b"fast\n".to_vec()),
            ),
        ))
        .unwrap_err();
        assert!(matches!(invalid, HwmonFanError::InvalidRpm { .. }));

        let permission = query(
            SimulatedHwmonTree::new("permission").with_device(
                "synthetic-a",
                SimulatedHwmonDevice::generic(b"acpi_fan\n".to_vec())
                    .with_entry("fan1_input", SimulatedHwmonEntry::PermissionDenied),
            ),
        )
        .unwrap_err();
        assert!(matches!(
            permission,
            HwmonFanError::Source {
                error: HwmonSourceError::PermissionDenied,
                ..
            }
        ));
    }

    #[test]
    fn rejects_duplicate_sources_and_disappearing_devices() {
        let duplicate = query(
            SimulatedHwmonTree::new("duplicate")
                .with_device(
                    "synthetic-a",
                    SimulatedHwmonDevice::generic(b"acpi_fan\n".to_vec()).with_entry(
                        "fan1_input",
                        SimulatedHwmonEntry::Regular(b"1900\n".to_vec()),
                    ),
                )
                .with_device(
                    "synthetic-b",
                    SimulatedHwmonDevice::generic(b"acpi_fan\n".to_vec()).with_entry(
                        "fan1_input",
                        SimulatedHwmonEntry::Regular(b"1900\n".to_vec()),
                    ),
                ),
        )
        .unwrap_err();
        assert!(matches!(duplicate, HwmonFanError::DuplicateSource { .. }));

        let disappearing = query(
            SimulatedHwmonTree::new("disappearing").with_device(
                "synthetic-a",
                SimulatedHwmonDevice::generic(b"acpi_fan\n".to_vec())
                    .with_entry("fan1_input", SimulatedHwmonEntry::DeviceDisappeared),
            ),
        )
        .unwrap_err();
        assert!(matches!(
            disappearing,
            HwmonFanError::Source {
                error: HwmonSourceError::DeviceDisappeared,
                ..
            }
        ));
    }

    #[test]
    fn reports_absence_without_inventing_zero() {
        assert!(matches!(
            query(SimulatedHwmonTree::new("empty")),
            Err(HwmonFanError::NoHwmonDevices)
        ));
    }
}
