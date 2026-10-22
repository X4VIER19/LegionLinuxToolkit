//! Interpretation of structured, simulated `Fan_Get_Table` evidence.
//!
//! The Linux WMI ABI and response layout are unknown for the target machine.
//! Consequently this parser does not decode an opaque WMI byte buffer. It only
//! validates an explicit evidence envelope supplied by a fixture or a future,
//! separately reviewed decoder, preserving that envelope verbatim.

use crate::{
    FanCurvePoint, FanCurveSemantics, FanCurveTable, FanCurveWriteCapability, FanTableProvenance,
    FanTableQuery, WINDOWS_TABLE_POINT_COUNT,
};
use legion_core::{TelemetryArtifact, WmiKnowledge, WmiObservationPlatform};
use serde::Serialize;
use std::fmt;

/// A field present in evidence but not assigned a firmware meaning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpaqueFanTableField {
    pub name: String,
    pub original: Vec<u8>,
}

/// One table represented by a source that has already named its fields. The
/// parser does not derive these fields from `raw_response`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FanTableEvidence {
    /// Ordinal values observed in a table. They are not temperature points.
    pub point_ordinals: Vec<u8>,
    /// Only a source with an explicit, reviewed Celsius contract may populate
    /// these. They are setpoints, never live temperature readings.
    pub temperature_setpoints_celsius: Option<Vec<u32>>,
    /// Only a source with an explicit, reviewed RPM-target contract may
    /// populate these. They are not measured fan speeds.
    pub target_rpm: Option<Vec<u32>>,
    pub unknown_fields: Vec<OpaqueFanTableField>,
}

/// Input envelope that keeps the raw buffer separate from source-level fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FanGetTableEvidence {
    pub query: FanTableQuery,
    /// `None` means that no byte-for-byte WMI response was captured.
    pub raw_response: Option<Vec<u8>>,
    pub tables: Vec<FanTableEvidence>,
    pub provenance: FanTableProvenance,
}

/// The parser result preserves the complete evidence input alongside the
/// interpreted curves, so consumers cannot mistake interpretation for capture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParsedFanGetTable {
    pub original: FanGetTableEvidence,
    pub curves: Vec<FanCurveTable>,
}

#[derive(Debug, Clone, Copy)]
pub struct FanGetTableEvidenceParser;

impl FanGetTableEvidenceParser {
    pub fn parse(
        evidence: FanGetTableEvidence,
    ) -> Result<ParsedFanGetTable, FanGetTableParseError> {
        if evidence.provenance.artifact != TelemetryArtifact::StaticSimulation {
            return Err(FanGetTableParseError::NonSimulatedEvidence);
        }
        if evidence.tables.is_empty() {
            return Err(FanGetTableParseError::MissingTables);
        }
        let curves = evidence
            .tables
            .iter()
            .map(|table| Self::parse_table(evidence.query, table, &evidence.provenance))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ParsedFanGetTable {
            original: evidence,
            curves,
        })
    }

    fn parse_table(
        query: FanTableQuery,
        table: &FanTableEvidence,
        provenance: &FanTableProvenance,
    ) -> Result<FanCurveTable, FanGetTableParseError> {
        let point_count = table.point_ordinals.len();
        if point_count == 0 || point_count > usize::from(WINDOWS_TABLE_POINT_COUNT) {
            return Err(FanGetTableParseError::UnsupportedPointCount { point_count });
        }
        let coordinates = match (&table.temperature_setpoints_celsius, &table.target_rpm) {
            (None, None) => None,
            (Some(temperatures), Some(targets))
                if temperatures.len() == point_count && targets.len() == point_count =>
            {
                Some((temperatures, targets))
            }
            (Some(_), Some(_)) => {
                return Err(FanGetTableParseError::MismatchedCoordinateLengths { point_count });
            }
            _ => return Err(FanGetTableParseError::IncompleteCoordinates),
        };

        let points = table
            .point_ordinals
            .iter()
            .enumerate()
            .map(|(index, ordinal)| FanCurvePoint {
                ordinal: *ordinal,
                temperature_setpoint_celsius: coordinates
                    .map(|(temperatures, _)| temperatures[index]),
                target_rpm: coordinates.map(|(_, targets)| targets[index]),
            })
            .collect();
        let semantics = if coordinates.is_some() {
            FanCurveSemantics::SyntheticNominalSimulation
        } else {
            FanCurveSemantics::OpaqueOrdinals
        };
        let mut curve_provenance = provenance.clone();
        if coordinates.is_some() {
            // These coordinates are accepted only from an explicit fixture
            // envelope. They are not part of the Windows observation.
            curve_provenance.knowledge = WmiKnowledge::Hypothesis;
            curve_provenance.source = "in-memory synthetic fan curve fixture".to_owned();
        }
        let curve = FanCurveTable {
            query,
            points,
            semantics,
            provenance: curve_provenance,
            write_capability: FanCurveWriteCapability::Prohibited,
        };
        curve
            .validate()
            .map_err(|error| FanGetTableParseError::InvalidCurve {
                reason: error.to_string(),
            })?;
        Ok(curve)
    }
}

/// Fixture based solely on the documented Windows observation: two tables with
/// ordinal values from 1 through 10. No raw ABI bytes, temperatures, or RPM
/// targets were captured, so all of those remain unknown.
pub fn documented_windows_table_evidence() -> FanGetTableEvidence {
    let table = FanTableEvidence {
        point_ordinals: (1..=WINDOWS_TABLE_POINT_COUNT).collect(),
        temperature_setpoints_celsius: None,
        target_rpm: None,
        unknown_fields: Vec::new(),
    };
    FanGetTableEvidence {
        query: FanTableQuery::documented_windows_observation(),
        raw_response: None,
        tables: vec![table.clone(), table],
        provenance: FanTableProvenance {
            artifact: TelemetryArtifact::StaticSimulation,
            observed_on: WmiObservationPlatform::Windows,
            source: "docs/windows-wmi-observations.md".to_owned(),
            knowledge: WmiKnowledge::Observed,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FanGetTableParseError {
    NonSimulatedEvidence,
    MissingTables,
    UnsupportedPointCount { point_count: usize },
    IncompleteCoordinates,
    MismatchedCoordinateLengths { point_count: usize },
    InvalidCurve { reason: String },
}

impl fmt::Display for FanGetTableParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonSimulatedEvidence => {
                write!(
                    f,
                    "fan table evidence must be an explicit static simulation"
                )
            }
            Self::MissingTables => write!(f, "fan table evidence contains no tables"),
            Self::UnsupportedPointCount { point_count } => write!(
                f,
                "fan table has unsupported point count {point_count}; expected from 1 through {WINDOWS_TABLE_POINT_COUNT}"
            ),
            Self::IncompleteCoordinates => write!(
                f,
                "temperature setpoints and RPM targets must be supplied together"
            ),
            Self::MismatchedCoordinateLengths { point_count } => write!(
                f,
                "curve coordinate lengths must both match the {point_count} ordinals"
            ),
            Self::InvalidCurve { reason } => write!(f, "invalid fan curve evidence: {reason}"),
        }
    }
}

impl std::error::Error for FanGetTableParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_windows_evidence_preserves_unknown_protocol_fields() {
        let evidence = documented_windows_table_evidence();
        let parsed = FanGetTableEvidenceParser::parse(evidence.clone()).unwrap();
        assert_eq!(parsed.original, evidence);
        assert!(parsed.original.raw_response.is_none());
        assert_eq!(parsed.curves.len(), 2);
        assert!(parsed.curves.iter().all(|curve| {
            curve.semantics == FanCurveSemantics::OpaqueOrdinals
                && curve.points.iter().all(|point| {
                    point.temperature_setpoint_celsius.is_none() && point.target_rpm.is_none()
                })
        }));
    }

    #[test]
    fn explicit_simulated_coordinates_become_targets_not_measurements() {
        let mut evidence = documented_windows_table_evidence();
        evidence.tables.truncate(1);
        evidence.tables[0].temperature_setpoints_celsius = Some((30..40).collect());
        evidence.tables[0].target_rpm = Some((1..=10).map(|point| point * 500).collect());
        let parsed = FanGetTableEvidenceParser::parse(evidence).unwrap();
        let curve = &parsed.curves[0];
        assert_eq!(
            curve.semantics,
            FanCurveSemantics::SyntheticNominalSimulation
        );
        assert_eq!(curve.points[0].temperature_setpoint_celsius, Some(30));
        assert_eq!(curve.points[9].target_rpm, Some(5_000));
        assert_eq!(curve.provenance.knowledge, WmiKnowledge::Hypothesis);
    }

    #[test]
    fn rejects_incomplete_or_mismatched_coordinates() {
        let mut incomplete = documented_windows_table_evidence();
        incomplete.tables[0].target_rpm = Some(vec![500; 10]);
        assert_eq!(
            FanGetTableEvidenceParser::parse(incomplete),
            Err(FanGetTableParseError::IncompleteCoordinates)
        );

        let mut mismatched = documented_windows_table_evidence();
        mismatched.tables[0].temperature_setpoints_celsius = Some(vec![30; 9]);
        mismatched.tables[0].target_rpm = Some(vec![500; 10]);
        assert_eq!(
            FanGetTableEvidenceParser::parse(mismatched),
            Err(FanGetTableParseError::MismatchedCoordinateLengths { point_count: 10 })
        );
    }

    #[test]
    fn rejects_excessive_duplicate_and_non_simulated_evidence() {
        let mut excessive = documented_windows_table_evidence();
        excessive.tables[0].point_ordinals = (1..=11).collect();
        assert_eq!(
            FanGetTableEvidenceParser::parse(excessive),
            Err(FanGetTableParseError::UnsupportedPointCount { point_count: 11 })
        );

        let mut duplicate = documented_windows_table_evidence();
        duplicate.tables[0].point_ordinals[0] = 2;
        assert!(matches!(
            FanGetTableEvidenceParser::parse(duplicate),
            Err(FanGetTableParseError::InvalidCurve { .. })
        ));

        let mut non_simulated = documented_windows_table_evidence();
        non_simulated.provenance.artifact = TelemetryArtifact::LinuxReadOnly;
        assert_eq!(
            FanGetTableEvidenceParser::parse(non_simulated),
            Err(FanGetTableParseError::NonSimulatedEvidence)
        );
    }

    #[test]
    fn preserves_opaque_unknown_field_bytes() {
        let mut evidence = documented_windows_table_evidence();
        evidence.tables[0].unknown_fields.push(OpaqueFanTableField {
            name: "vendor-extension".to_owned(),
            original: vec![0, 255, 10],
        });
        let parsed = FanGetTableEvidenceParser::parse(evidence).unwrap();
        assert_eq!(
            parsed.original.tables[0].unknown_fields[0].original,
            vec![0, 255, 10]
        );
    }
}
