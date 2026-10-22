//! Static bridge from the generic WMI transport contract to fan-table evidence.
//!
//! This module is not a Linux WMI implementation. Its transport is injected,
//! and the opaque response is preserved without decoding its bytes.

use crate::{
    FanGetTableEvidence, FanGetTableEvidenceParser, FanGetTableParseError, FanTableProvenance,
    FanTableQuery, ParsedFanGetTable,
};
use legion_core::{
    TelemetryArtifact, WmiAccess, WmiArgument, WmiArtifact, WmiKnowledge, WmiMethodMetadata,
    WmiOperation, WmiProvenance, WmiRequest, WmiResponse, WmiTransport, WmiTransportError,
};
use std::fmt;

/// A successful simulated request keeps both the opaque response and the
/// separately interpreted evidence. Neither is a firmware response claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedFanTableWmiResult {
    pub response: WmiResponse,
    pub parsed: ParsedFanGetTable,
}

/// Adapter over an injected WMI transport. It has no constructor for a real
/// transport and cannot locate a host WMI device.
#[derive(Debug, Clone)]
pub struct FanTableWmiAdapter<T> {
    transport: T,
}

impl<T> FanTableWmiAdapter<T> {
    pub const fn new(transport: T) -> Self {
        Self { transport }
    }
}

impl<T: WmiTransport> FanTableWmiAdapter<T> {
    /// Queries the injected transport once, then combines its opaque response
    /// with separately supplied simulated evidence.
    pub fn query(
        &self,
        query: FanTableQuery,
        mut evidence: FanGetTableEvidence,
    ) -> Result<SimulatedFanTableWmiResult, FanTableWmiError> {
        validate_evidence_template(query, &evidence)?;
        let response = self
            .transport
            .execute(&wmi_request(query))
            .map_err(FanTableWmiError::Transport)?;
        response
            .validate_buffer_size()
            .map_err(FanTableWmiError::Transport)?;
        if response.provenance.artifact != WmiArtifact::StaticSimulation {
            return Err(FanTableWmiError::Evidence(
                FanTableWmiEvidenceError::NonSimulatedResponse,
            ));
        }

        evidence.raw_response = Some(response.bytes.clone());
        evidence.provenance = fan_provenance(&response.provenance);
        let parsed =
            FanGetTableEvidenceParser::parse(evidence).map_err(FanTableWmiError::Interpretation)?;
        Ok(SimulatedFanTableWmiResult { response, parsed })
    }
}

fn validate_evidence_template(
    query: FanTableQuery,
    evidence: &FanGetTableEvidence,
) -> Result<(), FanTableWmiError> {
    if query != FanTableQuery::documented_windows_observation() || evidence.query != query {
        return Err(FanTableWmiError::Evidence(
            FanTableWmiEvidenceError::UnsupportedQuery,
        ));
    }
    if evidence.raw_response.is_some() {
        return Err(FanTableWmiError::Evidence(
            FanTableWmiEvidenceError::RawResponseAlreadyPresent,
        ));
    }
    if evidence.tables.is_empty() {
        return Err(FanTableWmiError::Evidence(
            FanTableWmiEvidenceError::InsufficientStructuredEvidence,
        ));
    }
    if evidence.provenance.artifact != TelemetryArtifact::StaticSimulation {
        return Err(FanTableWmiError::Evidence(
            FanTableWmiEvidenceError::NonSimulatedEvidence,
        ));
    }
    Ok(())
}

fn wmi_request(query: FanTableQuery) -> WmiRequest {
    WmiRequest {
        operation: WmiOperation::FanGetTable,
        access: WmiAccess::ReadOnly,
        arguments: vec![
            WmiArgument {
                name: "FanID".to_owned(),
                value: query.fan_id,
                knowledge: WmiKnowledge::Observed,
            },
            WmiArgument {
                name: "SensorID".to_owned(),
                value: query.sensor_id,
                knowledge: WmiKnowledge::Observed,
            },
        ],
        method: WmiMethodMetadata {
            observed_name: Some("Fan_Get_Table".to_owned()),
            method_identifier: None,
            request_abi: WmiKnowledge::Unknown,
            response_abi: WmiKnowledge::Unknown,
        },
        provenance: WmiProvenance {
            artifact: WmiArtifact::StaticSimulation,
            observed_on: query.observed_on,
            source: "in-memory Fan_Get_Table adapter request".to_owned(),
            bios_version: Some("NUCN15WW".to_owned()),
            knowledge: WmiKnowledge::Observed,
        },
    }
}

fn fan_provenance(wmi: &WmiProvenance) -> FanTableProvenance {
    FanTableProvenance {
        artifact: TelemetryArtifact::StaticSimulation,
        observed_on: wmi.observed_on,
        source: wmi.source.clone(),
        knowledge: wmi.knowledge,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FanTableWmiEvidenceError {
    UnsupportedQuery,
    RawResponseAlreadyPresent,
    InsufficientStructuredEvidence,
    NonSimulatedEvidence,
    NonSimulatedResponse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FanTableWmiError {
    Transport(WmiTransportError),
    Evidence(FanTableWmiEvidenceError),
    Interpretation(FanGetTableParseError),
}

impl fmt::Display for FanTableWmiEvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedQuery => {
                write!(f, "fan-table query is not the documented observation")
            }
            Self::RawResponseAlreadyPresent => {
                write!(f, "evidence template already contains a raw response")
            }
            Self::InsufficientStructuredEvidence => {
                write!(f, "fan-table evidence has no structured table data")
            }
            Self::NonSimulatedEvidence => {
                write!(f, "fan-table evidence is not a static simulation")
            }
            Self::NonSimulatedResponse => write!(f, "WMI response is not a static simulation"),
        }
    }
}

impl fmt::Display for FanTableWmiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => write!(f, "fan-table transport error: {error}"),
            Self::Evidence(error) => write!(f, "fan-table evidence error: {error}"),
            Self::Interpretation(error) => write!(f, "fan-table interpretation error: {error}"),
        }
    }
}

impl std::error::Error for FanTableWmiEvidenceError {}
impl std::error::Error for FanTableWmiError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documented_windows_table_evidence;
    use legion_core::{WmiObservationPlatform, WmiTransport};

    #[derive(Debug, Clone)]
    struct InjectedTransport(Result<WmiResponse, WmiTransportError>);

    impl WmiTransport for InjectedTransport {
        fn execute(&self, request: &WmiRequest) -> Result<WmiResponse, WmiTransportError> {
            assert!(matches!(request.operation, WmiOperation::FanGetTable));
            assert_eq!(request.access, WmiAccess::ReadOnly);
            assert_eq!(request.method.method_identifier, None);
            assert_eq!(request.method.request_abi, WmiKnowledge::Unknown);
            assert_eq!(request.method.response_abi, WmiKnowledge::Unknown);
            self.0.clone()
        }
    }

    fn response(bytes: Vec<u8>, reported_buffer_size: usize) -> WmiResponse {
        WmiResponse {
            reported_buffer_size,
            bytes,
            provenance: WmiProvenance {
                artifact: WmiArtifact::StaticSimulation,
                observed_on: WmiObservationPlatform::Windows,
                source: "injected static WMI response".to_owned(),
                bios_version: Some("NUCN15WW".to_owned()),
                knowledge: WmiKnowledge::Unknown,
            },
        }
    }

    #[test]
    fn success_preserves_opaque_response_and_keeps_unknown_format_opaque() {
        let response = response(vec![0xff, 0x00, 0xa5], 3);
        let adapter = FanTableWmiAdapter::new(InjectedTransport(Ok(response.clone())));
        let result = adapter
            .query(
                FanTableQuery::documented_windows_observation(),
                documented_windows_table_evidence(),
            )
            .unwrap();
        assert_eq!(result.response, response);
        assert_eq!(
            result.parsed.original.raw_response,
            Some(vec![0xff, 0x00, 0xa5])
        );
        assert!(
            result
                .parsed
                .curves
                .iter()
                .all(|curve| { curve.points.iter().all(|point| point.target_rpm.is_none()) })
        );
        assert_eq!(
            result.parsed.original.provenance.knowledge,
            WmiKnowledge::Unknown
        );
    }

    #[test]
    fn incomplete_response_is_a_transport_error() {
        let adapter = FanTableWmiAdapter::new(InjectedTransport(Ok(response(vec![1, 2], 3))));
        assert!(matches!(
            adapter.query(
                FanTableQuery::documented_windows_observation(),
                documented_windows_table_evidence()
            ),
            Err(FanTableWmiError::Transport(
                WmiTransportError::InconsistentBufferSize { .. }
            ))
        ));
    }

    #[test]
    fn transport_failure_is_not_reclassified_as_interpretation() {
        let adapter = FanTableWmiAdapter::new(InjectedTransport(Err(
            WmiTransportError::MethodUnavailable {
                operation: "Fan_Get_Table".to_owned(),
                reason: "fixture failure".to_owned(),
            },
        )));
        assert!(matches!(
            adapter.query(
                FanTableQuery::documented_windows_observation(),
                documented_windows_table_evidence()
            ),
            Err(FanTableWmiError::Transport(
                WmiTransportError::MethodUnavailable { .. }
            ))
        ));
    }

    #[test]
    fn missing_or_invalid_structured_evidence_is_distinct_from_transport() {
        let adapter = FanTableWmiAdapter::new(InjectedTransport(Ok(response(vec![], 0))));
        let mut missing = documented_windows_table_evidence();
        missing.tables.clear();
        assert_eq!(
            adapter.query(FanTableQuery::documented_windows_observation(), missing),
            Err(FanTableWmiError::Evidence(
                FanTableWmiEvidenceError::InsufficientStructuredEvidence
            ))
        );

        let mut malformed = documented_windows_table_evidence();
        malformed.tables[0].target_rpm = Some(vec![500; 10]);
        assert!(matches!(
            adapter.query(FanTableQuery::documented_windows_observation(), malformed),
            Err(FanTableWmiError::Interpretation(
                FanGetTableParseError::IncompleteCoordinates
            ))
        ));
    }
}
