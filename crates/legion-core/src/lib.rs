//! Domain models and an in-memory parser for simulated snapshots only.
//!
//! This crate never opens files, host paths, firmware, WMI, ACPI, or EC.

use serde::Serialize;
use std::fmt;

mod compatibility;
mod wmi;

pub use compatibility::{
    CapabilityRecord, CompatibilityProfile, CompatibilityRegistry, CompatibilityStatus,
    DeviceDescriptor, DeviceQuery, Evidence, EvidenceKind, MatchResult, ObservationPlatform,
};
pub use wmi::{
    WmiAccess, WmiArgument, WmiArtifact, WmiKnowledge, WmiMethodMetadata, WmiObservationPlatform,
    WmiOperation, WmiProvenance, WmiRequest, WmiResponse, WmiTransport, WmiTransportError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceIdentity {
    pub model: String,
    pub machine_type: String,
    pub bios_version: String,
    pub board_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PowerAttribute {
    pub name: String,
    pub current_value: i64,
    pub default_value: i64,
    pub min_value: i64,
    pub max_value: i64,
    pub unit: String,
    pub kind: AttributeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttributeKind {
    ThermalLimit,
    PowerLimit,
}

impl AttributeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ThermalLimit => "thermal-limit",
            Self::PowerLimit => "power-limit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FanValidation {
    Validated,
    Unvalidated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FanInterpretation {
    ReportedValue,
    Unavailable,
    ReadError,
    NonConclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FanReading {
    /// Numeric firmware or driver value, if a value was reported. `Some(0)` is
    /// deliberately distinct from `None` and does not imply a stopped fan.
    pub reported_rpm: Option<u32>,
    pub source: String,
    pub validation: FanValidation,
    pub interpretation: FanInterpretation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Capability {
    pub name: String,
    pub state: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub identity: DeviceIdentity,
    pub active_profile: String,
    pub profiles: Vec<String>,
    pub attributes: Vec<PowerAttribute>,
    pub fans: Vec<FanReading>,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotError {
    Missing(&'static str),
    Invalid(String),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(field) => write!(f, "missing {field}"),
            Self::Invalid(value) => write!(f, "invalid simulated data: {value}"),
        }
    }
}

impl std::error::Error for SnapshotError {}

/// Parses a fixture passed in memory. Format: sectioned `key=value` text.
pub fn parse_snapshot(input: &str) -> Result<Snapshot, SnapshotError> {
    let mut section = "";
    let mut data = std::collections::BTreeMap::new();
    for raw in input.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| SnapshotError::Invalid(line.into()))?;
        if section.is_empty() {
            return Err(SnapshotError::Invalid(line.into()));
        }
        data.insert(
            (section.to_string(), key.trim().to_string()),
            value.trim().to_string(),
        );
    }

    let get = |section: &str, key: &str, label: &'static str| {
        data.get(&(section.into(), key.into()))
            .cloned()
            .ok_or(SnapshotError::Missing(label))
    };
    let identity = DeviceIdentity {
        model: get("identity", "model", "identity.model")?,
        machine_type: get("identity", "machine_type", "identity.machine_type")?,
        bios_version: get("identity", "bios_version", "identity.bios_version")?,
        board_name: get("identity", "board_name", "identity.board_name")?,
    };
    let active_profile = get("profiles", "active", "profiles.active")?;
    let profiles = get("profiles", "available", "profiles.available")?
        .split(',')
        .map(str::trim)
        .map(str::to_string)
        .collect();
    let attributes = data
        .iter()
        .filter(|((section, _), _)| section == "attributes")
        .map(|((_, name), value)| parse_attribute(name, value))
        .collect::<Result<Vec<_>, _>>()?;
    let fans = data
        .iter()
        .filter(|((section, _), _)| section == "fans")
        .map(|((_, source), value)| parse_fan(source, value))
        .collect::<Result<Vec<_>, _>>()?;
    let capabilities = data
        .iter()
        .filter(|((section, _), _)| section == "capabilities")
        .map(|((_, name), value)| parse_capability(name, value))
        .collect::<Result<Vec<_>, _>>()?;
    if attributes.is_empty() {
        return Err(SnapshotError::Missing("attributes.<name>"));
    }
    if fans.is_empty() {
        return Err(SnapshotError::Missing("fans.<source>"));
    }
    if capabilities.is_empty() {
        return Err(SnapshotError::Missing("capabilities.<name>"));
    }
    Ok(Snapshot {
        identity,
        active_profile,
        profiles,
        attributes,
        fans,
        capabilities,
    })
}

fn parse_attribute(name: &str, value: &str) -> Result<PowerAttribute, SnapshotError> {
    let parts: Vec<_> = value.split(',').collect();
    if parts.len() != 6 {
        return Err(SnapshotError::Invalid(format!("attributes.{name}")));
    }
    let number = |raw: &str| {
        raw.parse()
            .map_err(|_| SnapshotError::Invalid(format!("attributes.{name}")))
    };
    let kind = match parts[5] {
        "thermal-limit" => AttributeKind::ThermalLimit,
        "power-limit" => AttributeKind::PowerLimit,
        _ => return Err(SnapshotError::Invalid(format!("attributes.{name}.kind"))),
    };
    Ok(PowerAttribute {
        name: name.into(),
        current_value: number(parts[0])?,
        default_value: number(parts[1])?,
        min_value: number(parts[2])?,
        max_value: number(parts[3])?,
        unit: parts[4].into(),
        kind,
    })
}

fn parse_fan(source: &str, value: &str) -> Result<FanReading, SnapshotError> {
    let mut parts = value.split(',');
    let reported = parts
        .next()
        .ok_or_else(|| SnapshotError::Invalid(format!("fans.{source}")))?;
    let validation = match parts.next() {
        Some("validated") => FanValidation::Validated,
        Some("unvalidated") => FanValidation::Unvalidated,
        _ => return Err(SnapshotError::Invalid(format!("fans.{source}.validation"))),
    };
    let interpretation = match parts.next() {
        Some("reported-value") => FanInterpretation::ReportedValue,
        Some("unavailable") => FanInterpretation::Unavailable,
        Some("read-error") => FanInterpretation::ReadError,
        Some("non-conclusive") => FanInterpretation::NonConclusive,
        _ => {
            return Err(SnapshotError::Invalid(format!(
                "fans.{source}.interpretation"
            )));
        }
    };
    if parts.next().is_some() {
        return Err(SnapshotError::Invalid(format!("fans.{source}")));
    }
    let reported_rpm = match reported {
        "unavailable" | "error" => None,
        number => Some(
            number
                .parse()
                .map_err(|_| SnapshotError::Invalid(format!("fans.{source}.rpm")))?,
        ),
    };
    match (reported, interpretation) {
        ("unavailable", FanInterpretation::Unavailable) => {}
        ("error", FanInterpretation::ReadError) => {}
        (value, FanInterpretation::ReportedValue | FanInterpretation::NonConclusive)
            if value != "unavailable" && value != "error" => {}
        _ => return Err(SnapshotError::Invalid(format!("fans.{source}.state"))),
    }
    Ok(FanReading {
        reported_rpm,
        source: source.into(),
        validation,
        interpretation,
    })
}

fn parse_capability(name: &str, value: &str) -> Result<Capability, SnapshotError> {
    let (state, evidence) = value
        .split_once(',')
        .ok_or_else(|| SnapshotError::Invalid(format!("capabilities.{name}")))?;
    Ok(Capability {
        name: name.into(),
        state: state.into(),
        evidence: evidence.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = "[identity]\nmodel=Legion\nmachine_type=83EX\nbios_version=NUCN15WW\nboard_name=LNVNB161216\n[profiles]\nactive=balanced\navailable=low-power,balanced\n[attributes]\ncpu_temp=85,90,60,100,C,thermal-limit\n[fans]\nyogafan=0,unvalidated,non-conclusive\nacpi_fan=unavailable,unvalidated,unavailable\ngpu_fan=error,unvalidated,read-error\n[capabilities]\nprofile.read=validated-read,fixture\n";

    #[test]
    fn preserves_zero_without_claiming_a_stopped_fan() {
        let snapshot = parse_snapshot(VALID).unwrap();
        let fan = snapshot
            .fans
            .iter()
            .find(|fan| fan.source == "yogafan")
            .unwrap();
        assert_eq!(fan.reported_rpm, Some(0));
        assert_eq!(fan.validation, FanValidation::Unvalidated);
        assert_eq!(fan.interpretation, FanInterpretation::NonConclusive);
    }

    #[test]
    fn distinguishes_unavailable_data() {
        let snapshot = parse_snapshot(VALID).unwrap();
        let fan = snapshot
            .fans
            .iter()
            .find(|fan| fan.source == "acpi_fan")
            .unwrap();
        assert_eq!(fan.reported_rpm, None);
        assert_eq!(fan.interpretation, FanInterpretation::Unavailable);
    }

    #[test]
    fn distinguishes_a_read_error_from_unavailable_data() {
        let snapshot = parse_snapshot(VALID).unwrap();
        let fan = snapshot
            .fans
            .iter()
            .find(|fan| fan.source == "gpu_fan")
            .unwrap();
        assert_eq!(fan.reported_rpm, None);
        assert_eq!(fan.interpretation, FanInterpretation::ReadError);
    }

    #[test]
    fn rejects_missing_and_malformed_data() {
        assert!(matches!(
            parse_snapshot(&VALID.replace("bios_version=NUCN15WW\n", "")),
            Err(SnapshotError::Missing(_))
        ));
        assert!(matches!(
            parse_snapshot(&VALID.replace("85,90,60,100,C,thermal-limit", "bad")),
            Err(SnapshotError::Invalid(_))
        ));
    }
}
