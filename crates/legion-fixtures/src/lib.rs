//! Embedded data for hardware-isolated tests and demos.

use legion_core::{
    CapabilityRecord, CompatibilityProfile, CompatibilityRegistry, CompatibilityStatus,
    ConfidenceLevel, DeviceDescriptor, DeviceIdentity, DeviceQuery, EnergyProfile, EnergyProfiles,
    Evidence, EvidenceKind, FanInterpretation, FanTelemetry, FanValidation, FrequencyReading,
    ObservationPlatform, PlatformProfileAdapter, PlatformProfileError, PlatformProfileInput,
    PlatformProfileRawInput, PlatformProfileReading, PowerReading, PowerReadingKind,
    TelemetryArtifact, TelemetryError, TelemetryProvenance, TelemetrySnapshot, TelemetryState,
    TelemetryUnit, TelemetryValue, TemperatureReading, ValueClassification, WmiArgument,
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

/// Named in-memory telemetry scenarios. They never open files or host paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryScenario {
    Valid,
    Absent,
    Unvalidated,
    Inconsistent,
    ReadError,
}

pub fn telemetry_scenario(name: &str) -> Option<TelemetryScenario> {
    match name {
        "valid" => Some(TelemetryScenario::Valid),
        "absent" => Some(TelemetryScenario::Absent),
        "unvalidated" => Some(TelemetryScenario::Unvalidated),
        "inconsistent" => Some(TelemetryScenario::Inconsistent),
        "read-error" => Some(TelemetryScenario::ReadError),
        _ => None,
    }
}

/// Named, embedded text scenarios for the platform-profile parser. They are
/// not files and cannot select a host path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformProfileScenario {
    Valid,
    ActiveOnly,
    MissingActive,
    EmptyActive,
    EmptyChoices,
    Unknown,
    Duplicate,
    Inconsistent,
    Malformed,
}

pub fn platform_profile_scenario(name: &str) -> Option<PlatformProfileScenario> {
    match name {
        "valid" => Some(PlatformProfileScenario::Valid),
        "active-only" => Some(PlatformProfileScenario::ActiveOnly),
        "missing-active" => Some(PlatformProfileScenario::MissingActive),
        "empty-active" => Some(PlatformProfileScenario::EmptyActive),
        "empty-choices" => Some(PlatformProfileScenario::EmptyChoices),
        "unknown" => Some(PlatformProfileScenario::Unknown),
        "duplicate" => Some(PlatformProfileScenario::Duplicate),
        "inconsistent" => Some(PlatformProfileScenario::Inconsistent),
        "malformed" => Some(PlatformProfileScenario::Malformed),
        _ => None,
    }
}

/// Parses only static strings created below. No code in this crate opens a
/// file, directory, device, or firmware interface.
pub fn simulated_platform_profile(
    scenario: PlatformProfileScenario,
) -> Result<PlatformProfileReading, PlatformProfileError> {
    let (active, choices) = match scenario {
        PlatformProfileScenario::Valid => (
            Some("balanced\n"),
            Some("low-power balanced performance max-power custom\n"),
        ),
        PlatformProfileScenario::ActiveOnly => (Some("balanced\n"), None),
        PlatformProfileScenario::MissingActive => (None, Some("low-power balanced performance\n")),
        PlatformProfileScenario::EmptyActive => (Some("\n"), Some("balanced performance\n")),
        PlatformProfileScenario::EmptyChoices => (Some("balanced\n"), Some("\n")),
        PlatformProfileScenario::Unknown => {
            (Some("vendor-turbo\n"), Some("balanced vendor-turbo\n"))
        }
        PlatformProfileScenario::Duplicate => (Some("balanced\n"), Some("balanced balanced\n")),
        PlatformProfileScenario::Inconsistent => (Some("balanced\n"), Some("performance\n")),
        PlatformProfileScenario::Malformed => (Some("balanced profile\n"), None),
    };
    PlatformProfileAdapter.parse(PlatformProfileInput {
        raw: PlatformProfileRawInput {
            active: active.map(str::to_owned),
            choices: choices.map(str::to_owned),
        },
        provenance: simulated_telemetry_provenance("legion-fixtures::platform_profile"),
    })
}

/// Creates static demonstration data. Values are intentionally not readings
/// from the host and are marked with `static-simulation` provenance.
pub fn simulated_telemetry(
    scenario: TelemetryScenario,
) -> Result<TelemetrySnapshot, TelemetryError> {
    let mut snapshot = base_telemetry();
    match scenario {
        TelemetryScenario::Valid => {}
        TelemetryScenario::Absent => {
            snapshot.power[1].reading.value = None;
            snapshot.power[1].reading.state = TelemetryState::Unavailable;
        }
        TelemetryScenario::Unvalidated => {
            snapshot.temperatures[0].reading.value = None;
            snapshot.temperatures[0].reading.state = TelemetryState::Unvalidated;
        }
        TelemetryScenario::Inconsistent => {
            snapshot.power[0].reading.unit = TelemetryUnit::Megahertz;
        }
        TelemetryScenario::ReadError => {
            snapshot.power[1].reading.value = None;
            snapshot.power[1].reading.state = TelemetryState::ReadError;
        }
    }
    snapshot.validate()?;
    Ok(snapshot)
}

fn base_telemetry() -> TelemetrySnapshot {
    let provenance = simulated_telemetry_provenance("legion-fixtures::base_telemetry");
    TelemetrySnapshot {
        artifact: TelemetryArtifact::StaticSimulation,
        identity: DeviceIdentity {
            model: "Legion Slim 5 16ARP9".to_owned(),
            machine_type: "83EX".to_owned(),
            bios_version: "NUCN15WW".to_owned(),
            board_name: "LNVNB161216".to_owned(),
        },
        profiles: EnergyProfiles {
            profiles: vec![
                EnergyProfile {
                    name: "balanced".to_owned(),
                    active: true,
                    state: TelemetryState::Available,
                    provenance: provenance.clone(),
                },
                EnergyProfile {
                    name: "performance".to_owned(),
                    active: false,
                    state: TelemetryState::Available,
                    provenance: provenance.clone(),
                },
            ],
        },
        temperatures: vec![TemperatureReading {
            sensor: "cpu-package-demo".to_owned(),
            reading: value(
                Some(68.0),
                TelemetryUnit::Celsius,
                ValueClassification::InstantaneousMeasurement,
                TelemetryState::Available,
                provenance.clone(),
            ),
        }],
        frequencies: vec![
            FrequencyReading {
                component: "cpu-demo".to_owned(),
                reading: value(
                    Some(3800.0),
                    TelemetryUnit::Megahertz,
                    ValueClassification::InstantaneousMeasurement,
                    TelemetryState::Available,
                    provenance.clone(),
                ),
            },
            FrequencyReading {
                component: "cpu-reference-demo".to_owned(),
                reading: value(
                    Some(3500.0),
                    TelemetryUnit::Megahertz,
                    ValueClassification::NominalValue,
                    TelemetryState::Available,
                    provenance.clone(),
                ),
            },
        ],
        power: vec![
            PowerReading {
                component: "cpu-package-limit-demo".to_owned(),
                kind: PowerReadingKind::ConfiguredLimit,
                reading: value(
                    Some(55.0),
                    TelemetryUnit::Watts,
                    ValueClassification::ConfiguredLimit,
                    TelemetryState::Available,
                    provenance.clone(),
                ),
            },
            PowerReading {
                component: "cpu-package-consumption-demo".to_owned(),
                kind: PowerReadingKind::InstantaneousConsumption,
                reading: value(
                    Some(31.5),
                    TelemetryUnit::Watts,
                    ValueClassification::InstantaneousMeasurement,
                    TelemetryState::Available,
                    provenance.clone(),
                ),
            },
        ],
        fans: vec![FanTelemetry {
            fan: "cpu-fan-demo".to_owned(),
            reported_speed: value(
                Some(0),
                TelemetryUnit::Rpm,
                ValueClassification::InstantaneousMeasurement,
                TelemetryState::Unvalidated,
                provenance,
            ),
            validation: FanValidation::Unvalidated,
            interpretation: FanInterpretation::NonConclusive,
        }],
    }
}

fn value<T>(
    value: Option<T>,
    unit: TelemetryUnit,
    classification: ValueClassification,
    state: TelemetryState,
    provenance: TelemetryProvenance,
) -> TelemetryValue<T> {
    TelemetryValue {
        value,
        unit,
        classification,
        state,
        provenance,
    }
}

fn simulated_telemetry_provenance(source: &str) -> TelemetryProvenance {
    TelemetryProvenance {
        artifact: TelemetryArtifact::StaticSimulation,
        source: source.to_owned(),
        confidence: ConfidenceLevel::Low,
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

    #[test]
    fn telemetry_fixture_keeps_limits_and_consumption_distinct() {
        let snapshot = simulated_telemetry(TelemetryScenario::Valid).unwrap();
        assert_eq!(snapshot.artifact, TelemetryArtifact::StaticSimulation);
        assert_eq!(
            snapshot.power[0].reading.classification,
            ValueClassification::ConfiguredLimit
        );
        assert_eq!(
            snapshot.power[1].reading.classification,
            ValueClassification::InstantaneousMeasurement
        );
    }

    #[test]
    fn telemetry_fixtures_cover_absent_unvalidated_and_read_errors() {
        let absent = simulated_telemetry(TelemetryScenario::Absent).unwrap();
        assert_eq!(absent.power[1].reading.value, None);
        assert_eq!(absent.power[1].reading.state, TelemetryState::Unavailable);

        let unvalidated = simulated_telemetry(TelemetryScenario::Unvalidated).unwrap();
        assert_eq!(unvalidated.temperatures[0].reading.value, None);
        assert_eq!(
            unvalidated.temperatures[0].reading.state,
            TelemetryState::Unvalidated
        );

        let error = simulated_telemetry(TelemetryScenario::ReadError).unwrap();
        assert_eq!(error.power[1].reading.value, None);
        assert_eq!(error.power[1].reading.state, TelemetryState::ReadError);
    }

    #[test]
    fn telemetry_fixture_rejects_inconsistent_units() {
        assert!(matches!(
            simulated_telemetry(TelemetryScenario::Inconsistent),
            Err(TelemetryError::InconsistentValue { .. })
        ));
    }

    #[test]
    fn telemetry_fixture_keeps_zero_rpm_non_conclusive() {
        let snapshot = simulated_telemetry(TelemetryScenario::Valid).unwrap();
        let fan = &snapshot.fans[0];
        assert_eq!(fan.reported_speed.value, Some(0));
        assert_eq!(fan.reported_speed.state, TelemetryState::Unvalidated);
        assert_eq!(fan.interpretation, FanInterpretation::NonConclusive);
    }

    #[test]
    fn telemetry_fixture_marks_every_source_as_static_simulation() {
        let snapshot = simulated_telemetry(TelemetryScenario::Valid).unwrap();
        assert_eq!(snapshot.artifact, TelemetryArtifact::StaticSimulation);
        assert!(
            snapshot
                .profiles
                .profiles
                .iter()
                .all(|profile| profile.provenance.artifact == TelemetryArtifact::StaticSimulation)
        );
        assert!(snapshot.temperatures.iter().all(|reading| {
            reading.reading.provenance.artifact == TelemetryArtifact::StaticSimulation
        }));
        assert!(snapshot.frequencies.iter().all(|reading| {
            reading.reading.provenance.artifact == TelemetryArtifact::StaticSimulation
        }));
        assert!(snapshot.power.iter().all(|reading| {
            reading.reading.provenance.artifact == TelemetryArtifact::StaticSimulation
        }));
        assert!(snapshot.fans.iter().all(|fan| {
            fan.reported_speed.provenance.artifact == TelemetryArtifact::StaticSimulation
        }));
    }

    #[test]
    fn platform_profile_fixture_preserves_simulated_raw_and_normalized_data() {
        let reading = simulated_platform_profile(PlatformProfileScenario::Valid).unwrap();
        assert_eq!(reading.raw.active.as_deref(), Some("balanced\n"));
        assert_eq!(reading.normalized.active.unwrap().name, "balanced");
        assert_eq!(
            reading.provenance.artifact,
            TelemetryArtifact::StaticSimulation
        );
    }

    #[test]
    fn platform_profile_fixture_covers_absent_and_unknown_data() {
        let missing = simulated_platform_profile(PlatformProfileScenario::MissingActive).unwrap();
        assert_eq!(missing.state, TelemetryState::Unavailable);
        assert_eq!(missing.normalized.active, None);

        let unknown = simulated_platform_profile(PlatformProfileScenario::Unknown).unwrap();
        assert_eq!(unknown.state, TelemetryState::Unvalidated);
    }

    #[test]
    fn platform_profile_fixture_rejects_empty_duplicate_and_inconsistent_text() {
        for scenario in [
            PlatformProfileScenario::EmptyActive,
            PlatformProfileScenario::EmptyChoices,
            PlatformProfileScenario::Duplicate,
            PlatformProfileScenario::Inconsistent,
            PlatformProfileScenario::Malformed,
        ] {
            assert!(simulated_platform_profile(scenario).is_err());
        }
    }

    #[test]
    fn platform_profile_scenarios_reject_host_path_names() {
        assert!(platform_profile_scenario("/sys").is_none());
        assert!(platform_profile_scenario("/proc").is_none());
        assert!(platform_profile_scenario("/dev").is_none());
    }
}
