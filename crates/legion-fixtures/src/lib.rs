//! Embedded data for hardware-isolated tests and demos.

use legion_core::{
    CapabilityRecord, CompatibilityProfile, CompatibilityRegistry, CompatibilityStatus,
    DeviceDescriptor, DeviceQuery, Evidence, EvidenceKind, ObservationPlatform,
};

pub const VALID: &str = include_str!("../data/valid.snapshot");
pub const MISSING: &str = include_str!("../data/missing.snapshot");
pub const MALFORMED: &str = include_str!("../data/malformed.snapshot");

/// Only named embedded fixtures are available; host paths are never opened.
pub fn named(name: &str) -> Option<&'static str> {
    match name {
        "valid" => Some(VALID),
        "missing" => Some(MISSING),
        "malformed" => Some(MALFORMED),
        _ => None,
    }
}

/// Static declarative profile based on documented observations only.
pub fn legion_slim_5_16arp9_registry() -> CompatibilityRegistry {
    CompatibilityRegistry {
        profiles: vec![CompatibilityProfile {
            id: "lenovo-legion-slim-5-16arp9-83ex-nucn15ww".to_owned(),
            identity: DeviceDescriptor {
                manufacturer: "Lenovo".to_owned(),
                model: "Legion Slim 5 16ARP9".to_owned(),
                machine_type: "83EX".to_owned(),
                bios_version: "NUCN15WW".to_owned(),
                dmi_identifier: None,
            },
            capabilities: vec![
                capability(
                    "platform-profile.read",
                    CompatibilityStatus::Observed,
                    linux_evidence("platform_profile lists low-power through custom"),
                ),
                capability(
                    "power-attributes.read",
                    CompatibilityStatus::Observed,
                    linux_evidence("lenovo_wmi_other attributes observed in sysfs"),
                ),
                capability(
                    "fan-rpm.read",
                    CompatibilityStatus::Unvalidated,
                    linux_evidence("acpi_fan and yogafan reported 0 RPM; meaning is inconclusive"),
                ),
                capability(
                    "fan-table.read",
                    CompatibilityStatus::Observed,
                    windows_evidence(
                        "Fan_Get_Table succeeded in Windows; Linux remains unvalidated",
                    ),
                ),
                capability(
                    "fan-table.write",
                    CompatibilityStatus::Unknown,
                    windows_evidence(
                        "Fan_Set_Table exists but is untested and prohibited in this phase",
                    ),
                ),
                capability(
                    "lenovo-wmi-other.fan-reporting",
                    CompatibilityStatus::NotCompatible,
                    linux_evidence(
                        "kernel reported fan reporting/tuning unsupported for this interface",
                    ),
                ),
            ],
        }],
    }
}

pub fn compatibility_query(name: &str) -> Option<DeviceQuery> {
    match name {
        "known" => Some(DeviceQuery {
            manufacturer: Some("Lenovo".to_owned()),
            model: Some("Legion Slim 5 16ARP9".to_owned()),
            machine_type: Some("83EX".to_owned()),
            bios_version: Some("NUCN15WW".to_owned()),
            dmi_identifier: None,
        }),
        "unknown" => Some(DeviceQuery {
            bios_version: Some("UNKNOWN".to_owned()),
            ..compatibility_query("known")?
        }),
        "incomplete" => Some(DeviceQuery {
            manufacturer: Some("Lenovo".to_owned()),
            ..DeviceQuery::default()
        }),
        _ => None,
    }
}

fn capability(key: &str, status: CompatibilityStatus, evidence: Evidence) -> CapabilityRecord {
    CapabilityRecord::descriptive(key, status, evidence)
}

fn linux_evidence(summary: &str) -> Evidence {
    Evidence {
        platform: ObservationPlatform::Linux,
        kind: EvidenceKind::Observation,
        source: "docs/linux-observations.md".to_owned(),
        summary: summary.to_owned(),
        status: CompatibilityStatus::Observed,
    }
}

fn windows_evidence(summary: &str) -> Evidence {
    Evidence {
        platform: ObservationPlatform::Windows,
        kind: EvidenceKind::Observation,
        source: "docs/windows-wmi-observations.md".to_owned(),
        summary: summary.to_owned(),
        status: CompatibilityStatus::Observed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_host_path_names() {
        assert!(named("/sys").is_none());
        assert!(named("/proc").is_none());
        assert!(named("/dev").is_none());
    }

    #[test]
    fn profile_is_static_and_never_enables_controls() {
        let registry = legion_slim_5_16arp9_registry();
        assert_eq!(registry.profiles.len(), 1);
        assert!(
            registry.profiles[0]
                .capabilities
                .iter()
                .all(|capability| !capability.controls_enabled())
        );
    }

    #[test]
    fn profile_keeps_linux_and_windows_evidence_separate() {
        let profile = &legion_slim_5_16arp9_registry().profiles[0];
        let linux = profile
            .capabilities
            .iter()
            .find(|capability| capability.key == "platform-profile.read")
            .unwrap();
        assert_eq!(linux.status, CompatibilityStatus::Observed);
        assert_eq!(linux.evidence[0].platform, ObservationPlatform::Linux);
        let windows = profile
            .capabilities
            .iter()
            .find(|capability| capability.key == "fan-table.read")
            .unwrap();
        assert_eq!(windows.status, CompatibilityStatus::Observed);
        assert_eq!(windows.evidence[0].platform, ObservationPlatform::Windows);
    }
}
