//! Hardware-isolated fan domain and transport models.
//!
//! This crate deliberately implements only static simulation. It does not open
//! Linux paths, invoke WMI/ACPI/EC, or expose a write operation. The static
//! AML-confirmed `Fan_Get_Table` buffer can be decoded in memory only; no
//! executable firmware protocol is exposed.

use legion_core::{
    FanInterpretation, FanTelemetry, FanValidation, TelemetryArtifact, TelemetryProvenance,
    TelemetryState, TelemetryUnit, TelemetryValue, ValueClassification, WmiKnowledge,
    WmiObservationPlatform,
};
use serde::Serialize;
use std::{collections::BTreeSet, fmt};

mod fan_get_table;
mod fan_get_table_abi;
mod kernel_contract;
mod kernel_endpoint;
mod wmi_adapter;
mod wmi_schema;

pub use fan_get_table::{
    FanGetTableEvidence, FanGetTableEvidenceParser, FanGetTableParseError, FanTableEvidence,
    OpaqueFanTableField, ParsedFanGetTable, documented_windows_table_evidence,
};
pub use fan_get_table_abi::{
    DecodedFanGetTableResponse, FAN_GET_TABLE_INPUT_SIZE_BYTES, FAN_GET_TABLE_RESPONSE_SIZE_BYTES,
    FAN_GET_TABLE_TABLE_ENTRY_COUNT, FanGetTableAbiError,
    decode_confirmed_83ex_fan_get_table_response, encode_confirmed_83ex_fan_get_table_request,
};
pub use kernel_contract::{
    FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES, FAN_KERNEL_READ_CONTRACT_VERSION,
    KernelFanReadContractError, KernelFanReadStatus, SimulatedKernelFanReadResponse,
    SimulatedKernelFanReadResult, consume_simulated_kernel_fan_read,
};
pub use kernel_endpoint::{
    FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES, KernelFanEndpointError,
    consume_simulated_kernel_endpoint_bytes,
};
pub use wmi_adapter::{
    FanTableWmiAdapter, FanTableWmiError, FanTableWmiEvidenceError, SimulatedFanTableWmiResult,
};
pub use wmi_schema::{
    ConfirmedFanGetTableSchema, FAN_GET_TABLE_METHOD_ID, FAN_SET_TABLE_METHOD_ID,
    FanSetTableInvocation, KnownFanSetTableMetadata, LENOVO_FAN_METHOD_CLASS,
    LENOVO_FAN_METHOD_GUID, LinuxAcpiAbiBoundary, WmiSchemaParameter, WmiSchemaParameterDirection,
    WmiSchemaValueType, confirmed_83ex_fan_get_table_schema, known_83ex_fan_set_table_metadata,
};

pub const DOCUMENTED_FAN_ID: u8 = 1;
pub const DOCUMENTED_SENSOR_ID: u8 = 4;
pub const WINDOWS_TABLE_POINT_COUNT: u8 = 10;
pub const OBSERVED_NOMINAL_RPM_UPPER_BOUND: u32 = 5_000;

/// A request shape observed in Windows. The two values are data points, not a
/// claim about a Linux ABI or a numeric WMI method identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FanTableQuery {
    pub fan_id: u8,
    pub sensor_id: u8,
    pub observed_on: WmiObservationPlatform,
    pub request_knowledge: WmiKnowledge,
}

impl FanTableQuery {
    pub const fn documented_windows_observation() -> Self {
        Self {
            fan_id: DOCUMENTED_FAN_ID,
            sensor_id: DOCUMENTED_SENSOR_ID,
            observed_on: WmiObservationPlatform::Windows,
            request_knowledge: WmiKnowledge::Observed,
        }
    }
}

/// A sensor is not assigned to CPU or GPU unless a later, model-specific
/// validation proves that relationship.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FanSensor {
    pub sensor_id: u8,
    pub temperature: TelemetryValue<f64>,
    pub assignment: FanSensorAssignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FanSensorAssignment {
    Unvalidated,
}

/// A combined runtime observation reuses the shared temperature and RPM domain
/// models. A reported zero RPM remains non-conclusive.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FanRuntimeObservation {
    pub fan_id: u8,
    pub sensor: FanSensor,
    pub speed: FanTelemetry,
}

/// The meaning of curve points has not been decoded from a WMI buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FanCurveSemantics {
    OpaqueOrdinals,
    SyntheticNominalSimulation,
}

/// A table point may retain a configured target RPM without claiming a real RPM
/// measurement. Its temperature coordinate is likewise a curve setpoint, not a
/// live sensor reading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FanCurvePoint {
    pub ordinal: u8,
    pub temperature_setpoint_celsius: Option<u32>,
    pub target_rpm: Option<u32>,
}

/// Immutable table representation. `write_capability` is intentionally
/// descriptive: no write transport or write request exists in this crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FanCurveTable {
    pub query: FanTableQuery,
    pub points: Vec<FanCurvePoint>,
    pub semantics: FanCurveSemantics,
    pub provenance: FanTableProvenance,
    pub write_capability: FanCurveWriteCapability,
}

impl FanCurveTable {
    pub fn validate(&self) -> Result<(), FanBackendError> {
        if self.points.is_empty() || self.points.len() > usize::from(WINDOWS_TABLE_POINT_COUNT) {
            return Err(FanBackendError::InvalidTable {
                reason: format!(
                    "expected from 1 through {WINDOWS_TABLE_POINT_COUNT} points, got {}",
                    self.points.len()
                ),
            });
        }
        let mut ordinals = BTreeSet::new();
        for point in &self.points {
            if !(1..=WINDOWS_TABLE_POINT_COUNT).contains(&point.ordinal)
                || !ordinals.insert(point.ordinal)
            {
                return Err(FanBackendError::InvalidTable {
                    reason: "point ordinals must be unique values from 1 through 10".to_owned(),
                });
            }
            if let Some(rpm) = point.target_rpm
                && rpm > OBSERVED_NOMINAL_RPM_UPPER_BOUND
            {
                return Err(FanBackendError::InvalidTable {
                    reason: "nominal RPM exceeds the documented observed upper bound".to_owned(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FanTableProvenance {
    pub artifact: TelemetryArtifact,
    pub observed_on: WmiObservationPlatform,
    pub source: String,
    pub knowledge: WmiKnowledge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FanCurveWriteCapability {
    Prohibited,
}

/// Read-only transport boundary. No corresponding write trait is defined.
pub trait FanTableReadTransport {
    fn read_table(&self, query: FanTableQuery) -> Result<FanCurveTable, FanBackendError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulatedFanScenario {
    ObservedWindowsShape,
    SyntheticNominalCurve,
    Unavailable,
}

/// In-memory transport only. It never delegates to WMI or a host fallback.
#[derive(Debug, Clone, Copy)]
pub struct SimulatedFanTransport {
    scenario: SimulatedFanScenario,
}

impl SimulatedFanTransport {
    pub const fn new(scenario: SimulatedFanScenario) -> Self {
        Self { scenario }
    }
}

impl FanTableReadTransport for SimulatedFanTransport {
    fn read_table(&self, query: FanTableQuery) -> Result<FanCurveTable, FanBackendError> {
        if query != FanTableQuery::documented_windows_observation() {
            return Err(FanBackendError::UnsupportedQuery);
        }
        if self.scenario == SimulatedFanScenario::Unavailable {
            return Err(FanBackendError::Unavailable {
                reason: "static simulation models an unavailable table; no host fallback exists"
                    .to_owned(),
            });
        }

        let (semantics, points, knowledge) = match self.scenario {
            SimulatedFanScenario::ObservedWindowsShape => (
                FanCurveSemantics::OpaqueOrdinals,
                (1..=WINDOWS_TABLE_POINT_COUNT)
                    .map(|ordinal| FanCurvePoint {
                        ordinal,
                        temperature_setpoint_celsius: None,
                        target_rpm: None,
                    })
                    .collect(),
                WmiKnowledge::Observed,
            ),
            SimulatedFanScenario::SyntheticNominalCurve => (
                FanCurveSemantics::SyntheticNominalSimulation,
                (1..=WINDOWS_TABLE_POINT_COUNT)
                    .map(|ordinal| FanCurvePoint {
                        ordinal,
                        temperature_setpoint_celsius: Some(30 + u32::from(ordinal) * 5),
                        target_rpm: Some(u32::from(ordinal) * 500),
                    })
                    .collect(),
                WmiKnowledge::Hypothesis,
            ),
            SimulatedFanScenario::Unavailable => unreachable!("handled above"),
        };
        let table = FanCurveTable {
            query,
            points,
            semantics,
            provenance: FanTableProvenance {
                artifact: TelemetryArtifact::StaticSimulation,
                observed_on: WmiObservationPlatform::Windows,
                source: "in-memory fan transport; docs/windows-wmi-observations.md".to_owned(),
                knowledge,
            },
            write_capability: FanCurveWriteCapability::Prohibited,
        };
        table.validate()?;
        Ok(table)
    }
}

/// Simulated runtime data with explicitly unvalidated, non-conclusive zero RPM.
pub fn simulated_runtime_observation() -> FanRuntimeObservation {
    let provenance = TelemetryProvenance {
        artifact: TelemetryArtifact::StaticSimulation,
        source: "in-memory fan backend fixture".to_owned(),
        confidence: legion_core::ConfidenceLevel::Low,
    };
    FanRuntimeObservation {
        fan_id: DOCUMENTED_FAN_ID,
        sensor: FanSensor {
            sensor_id: DOCUMENTED_SENSOR_ID,
            temperature: TelemetryValue {
                value: None,
                unit: TelemetryUnit::Celsius,
                classification: ValueClassification::InstantaneousMeasurement,
                state: TelemetryState::Unvalidated,
                provenance: provenance.clone(),
            },
            assignment: FanSensorAssignment::Unvalidated,
        },
        speed: FanTelemetry {
            fan: "simulated-fan-1".to_owned(),
            reported_speed: TelemetryValue {
                value: Some(0),
                unit: TelemetryUnit::Rpm,
                classification: ValueClassification::InstantaneousMeasurement,
                state: TelemetryState::Unvalidated,
                provenance,
            },
            validation: FanValidation::Unvalidated,
            interpretation: FanInterpretation::NonConclusive,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FanBackendError {
    UnsupportedQuery,
    Unavailable { reason: String },
    InvalidTable { reason: String },
}

impl fmt::Display for FanBackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedQuery => write!(f, "unsupported simulated fan table query"),
            Self::Unavailable { reason } => write!(f, "simulated fan table unavailable: {reason}"),
            Self::InvalidTable { reason } => write!(f, "invalid simulated fan table: {reason}"),
        }
    }
}

impl std::error::Error for FanBackendError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_shape_is_static_simulation_without_abi_decoding() {
        let table = SimulatedFanTransport::new(SimulatedFanScenario::ObservedWindowsShape)
            .read_table(FanTableQuery::documented_windows_observation())
            .unwrap();
        assert_eq!(table.points.len(), 10);
        assert!(table.points.iter().all(|point| point.target_rpm.is_none()));
        assert_eq!(table.semantics, FanCurveSemantics::OpaqueOrdinals);
        assert_eq!(
            table.provenance.artifact,
            TelemetryArtifact::StaticSimulation
        );
        assert_eq!(table.write_capability, FanCurveWriteCapability::Prohibited);
    }

    #[test]
    fn synthetic_curve_is_explicitly_not_an_observed_wmi_decoding() {
        let table = SimulatedFanTransport::new(SimulatedFanScenario::SyntheticNominalCurve)
            .read_table(FanTableQuery::documented_windows_observation())
            .unwrap();
        assert_eq!(
            table.semantics,
            FanCurveSemantics::SyntheticNominalSimulation
        );
        assert_eq!(table.points.last().unwrap().target_rpm, Some(5_000));
        assert_eq!(table.provenance.knowledge, WmiKnowledge::Hypothesis);
    }

    #[test]
    fn only_the_documented_query_shape_is_accepted() {
        let result = SimulatedFanTransport::new(SimulatedFanScenario::ObservedWindowsShape)
            .read_table(FanTableQuery {
                fan_id: 2,
                ..FanTableQuery::documented_windows_observation()
            });
        assert_eq!(result, Err(FanBackendError::UnsupportedQuery));
    }

    #[test]
    fn unavailable_transport_has_no_host_fallback() {
        let result = SimulatedFanTransport::new(SimulatedFanScenario::Unavailable)
            .read_table(FanTableQuery::documented_windows_observation());
        assert!(matches!(result, Err(FanBackendError::Unavailable { .. })));
    }

    #[test]
    fn invalid_curve_points_are_rejected() {
        let mut table = SimulatedFanTransport::new(SimulatedFanScenario::ObservedWindowsShape)
            .read_table(FanTableQuery::documented_windows_observation())
            .unwrap();
        table.points[0].ordinal = 2;
        assert!(matches!(
            table.validate(),
            Err(FanBackendError::InvalidTable { .. })
        ));
    }

    #[test]
    fn zero_rpm_and_sensor_assignment_remain_unvalidated() {
        let runtime = simulated_runtime_observation();
        assert_eq!(runtime.speed.reported_speed.value, Some(0));
        assert_eq!(runtime.speed.validation, FanValidation::Unvalidated);
        assert_eq!(
            runtime.speed.interpretation,
            FanInterpretation::NonConclusive
        );
        assert_eq!(runtime.sensor.assignment, FanSensorAssignment::Unvalidated);
        assert_eq!(
            runtime.speed.reported_speed.provenance.artifact,
            TelemetryArtifact::StaticSimulation
        );
    }
}
