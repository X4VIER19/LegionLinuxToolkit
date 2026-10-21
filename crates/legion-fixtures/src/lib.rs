//! Embedded data for hardware-isolated tests and demos.

use legion_core::{
    CapabilityRecord, CompatibilityProfile, CompatibilityRegistry, CompatibilityStatus,
    DeviceDescriptor, DeviceQuery, Evidence, EvidenceKind, ObservationPlatform, WmiArgument,
    WmiArtifact, WmiKnowledge, WmiMethodMetadata, WmiObservationPlatform, WmiOperation,
    WmiProvenance, WmiRequest, WmiResponse, WmiTransport, WmiTransportError,
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

/// Named simulated WMI outcomes. They select in-memory fixtures only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WmiScenario {
    Valid,
    Unavailable,
    InconsistentBuffer,
    UnsupportedOperation,
}

pub fn wmi_scenario(name: &str) -> Option<WmiScenario> {
    match name {
        "valid" => Some(WmiScenario::Valid),
        "unavailable" => Some(WmiScenario::Unavailable),
        "inconsistent-buffer" => Some(WmiScenario::InconsistentBuffer),
        "unsupported" => Some(WmiScenario::UnsupportedOperation),
        _ => None,
    }
}

/// A documented Windows request represented as data, not a callable method.
pub fn documented_fan_get_table_request() -> WmiRequest {
    WmiRequest {
        operation: WmiOperation::FanGetTable,
        access: legion_core::WmiAccess::ReadOnly,
        arguments: vec![
            WmiArgument {
                name: "FanID".to_owned(),
                value: 1,
                knowledge: WmiKnowledge::Observed,
            },
            WmiArgument {
                name: "SensorID".to_owned(),
                value: 4,
                knowledge: WmiKnowledge::Observed,
            },
        ],
        method: WmiMethodMetadata {
            observed_name: Some("Fan_Get_Table".to_owned()),
            method_identifier: None,
            request_abi: WmiKnowledge::Unknown,
            response_abi: WmiKnowledge::Unknown,
        },
        provenance: windows_wmi_provenance(WmiKnowledge::Observed),
    }
}

/// An isolated transport that returns synthetic bytes and never reaches the host.
#[derive(Debug, Clone, Copy)]
pub struct SimulatedWmiTransport {
    scenario: WmiScenario,
}

impl SimulatedWmiTransport {
    pub fn new(scenario: WmiScenario) -> Self {
        Self { scenario }
    }
}

impl WmiTransport for SimulatedWmiTransport {
    fn execute(&self, request: &WmiRequest) -> Result<WmiResponse, WmiTransportError> {
        if !matches!(&request.operation, WmiOperation::FanGetTable) {
            return Err(WmiTransportError::UnsupportedOperation {
                operation: request.operation.label().to_owned(),
            });
        }
        match self.scenario {
            WmiScenario::Valid => {
                // Deliberately synthetic opaque bytes, not a captured WMI buffer.
                let bytes = vec![0x4c, 0x4c, 0x54, 0x2d, 0x53, 0x49, 0x4d];
                Ok(WmiResponse {
                    reported_buffer_size: bytes.len(),
                    bytes,
                    provenance: WmiProvenance {
                        artifact: WmiArtifact::StaticSimulation,
                        observed_on: WmiObservationPlatform::Windows,
                        source: "docs/windows-wmi-observations.md".to_owned(),
                        bios_version: Some("NUCN15WW".to_owned()),
                        knowledge: WmiKnowledge::Hypothesis,
                    },
                })
            }
            WmiScenario::Unavailable => Err(WmiTransportError::MethodUnavailable {
                operation: request.operation.label().to_owned(),
                reason: "fixture simulates an unavailable method; no host fallback exists"
                    .to_owned(),
            }),
            WmiScenario::InconsistentBuffer => {
                let response = WmiResponse {
                    reported_buffer_size: 8,
                    bytes: vec![0x00, 0x01],
                    provenance: windows_wmi_provenance(WmiKnowledge::Hypothesis),
                };
                response.validate_buffer_size()?;
                Ok(response)
            }
            WmiScenario::UnsupportedOperation => Err(WmiTransportError::UnsupportedOperation {
                operation: request.operation.label().to_owned(),
            }),
        }
    }
}

fn windows_wmi_provenance(knowledge: WmiKnowledge) -> WmiProvenance {
    WmiProvenance {
        artifact: WmiArtifact::StaticSimulation,
        observed_on: WmiObservationPlatform::Windows,
        source: "docs/windows-wmi-observations.md".to_owned(),
        bios_version: Some("NUCN15WW".to_owned()),
        knowledge,
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

    #[test]
    fn valid_wmi_fixture_is_synthetic_and_has_a_consistent_buffer() {
        let response = SimulatedWmiTransport::new(WmiScenario::Valid)
            .execute(&documented_fan_get_table_request())
            .unwrap();
        assert_eq!(response.provenance.artifact, WmiArtifact::StaticSimulation);
        assert_eq!(response.provenance.knowledge, WmiKnowledge::Hypothesis);
        assert_eq!(response.validate_buffer_size(), Ok(()));
    }

    #[test]
    fn wmi_fixtures_report_errors_without_a_host_fallback() {
        let request = documented_fan_get_table_request();
        assert!(matches!(
            SimulatedWmiTransport::new(WmiScenario::Unavailable).execute(&request),
            Err(WmiTransportError::MethodUnavailable { .. })
        ));
        assert!(matches!(
            SimulatedWmiTransport::new(WmiScenario::InconsistentBuffer).execute(&request),
            Err(WmiTransportError::InconsistentBufferSize { .. })
        ));
        assert!(matches!(
            SimulatedWmiTransport::new(WmiScenario::UnsupportedOperation).execute(&request),
            Err(WmiTransportError::UnsupportedOperation { .. })
        ));
    }

    #[test]
    fn refuses_an_operation_not_modelled_by_the_fixture() {
        let mut request = documented_fan_get_table_request();
        request.operation = WmiOperation::OtherReadOnly {
            label: "unknown-read".to_owned(),
        };
        assert!(matches!(
            SimulatedWmiTransport::new(WmiScenario::Valid).execute(&request),
            Err(WmiTransportError::UnsupportedOperation { .. })
        ));
    }
}
