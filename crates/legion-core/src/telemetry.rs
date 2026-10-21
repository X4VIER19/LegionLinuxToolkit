//! Platform-neutral telemetry domain models.
//!
//! This module models already-provided data. It does not read host paths,
//! invoke firmware, or select an adapter.

use crate::{DeviceIdentity, FanInterpretation, FanValidation};
use serde::Serialize;
use std::fmt;

/// Explicit unit used by a telemetry value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TelemetryUnit {
    #[serde(rename = "°C")]
    Celsius,
    #[serde(rename = "MHz")]
    Megahertz,
    #[serde(rename = "W")]
    Watts,
    #[serde(rename = "RPM")]
    Rpm,
}

impl TelemetryUnit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Celsius => "°C",
            Self::Megahertz => "MHz",
            Self::Watts => "W",
            Self::Rpm => "RPM",
        }
    }
}

/// The meaning of a number, kept separate from its unit and availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ValueClassification {
    InstantaneousMeasurement,
    ConfiguredLimit,
    NominalValue,
}

impl ValueClassification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InstantaneousMeasurement => "instantaneous measurement",
            Self::ConfiguredLimit => "configured limit",
            Self::NominalValue => "nominal value",
        }
    }
}

/// Availability of one reading. It does not imply that an interface is safe to
/// invoke on the current machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TelemetryState {
    Available,
    Unavailable,
    Unvalidated,
    ReadError,
}

impl TelemetryState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
            Self::Unvalidated => "unvalidated",
            Self::ReadError => "read error",
        }
    }
}

/// Confidence in a value's representation, not a statement of hardware support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfidenceLevel {
    Low,
    Medium,
    High,
}

impl ConfidenceLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// Origin class. Phase 5A deliberately exposes only static simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TelemetryArtifact {
    StaticSimulation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TelemetryProvenance {
    pub artifact: TelemetryArtifact,
    pub source: String,
    pub confidence: ConfidenceLevel,
}

/// A scalar value whose semantic role, unit, state, and provenance travel with
/// it. `Some(0)` remains a reported value and is distinct from `None`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TelemetryValue<T> {
    pub value: Option<T>,
    pub unit: TelemetryUnit,
    pub classification: ValueClassification,
    pub state: TelemetryState,
    pub provenance: TelemetryProvenance,
}

impl<T> TelemetryValue<T> {
    fn validate_presence(&self, label: &str) -> Result<(), TelemetryError> {
        match (self.state, self.value.is_some()) {
            (TelemetryState::Available, false) => Err(TelemetryError::InconsistentValue {
                metric: label.to_owned(),
                reason: "an available reading requires a numeric value".to_owned(),
            }),
            (TelemetryState::Unavailable | TelemetryState::ReadError, true) => {
                Err(TelemetryError::InconsistentValue {
                    metric: label.to_owned(),
                    reason: "unavailable and read-error readings cannot carry a numeric value"
                        .to_owned(),
                })
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TemperatureReading {
    pub sensor: String,
    pub reading: TelemetryValue<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FrequencyReading {
    pub component: String,
    pub reading: TelemetryValue<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PowerReading {
    pub component: String,
    pub kind: PowerReadingKind,
    pub reading: TelemetryValue<f64>,
}

/// Explicitly distinguishes actual power consumption from a configured power
/// limit. A snapshot rejects a mismatch with `TelemetryValue::classification`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PowerReadingKind {
    InstantaneousConsumption,
    ConfiguredLimit,
}

impl PowerReadingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InstantaneousConsumption => "instantaneous consumption",
            Self::ConfiguredLimit => "configured limit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FanTelemetry {
    pub fan: String,
    /// This is a reported speed, not an inferred operational state.
    pub reported_speed: TelemetryValue<u32>,
    pub validation: FanValidation,
    pub interpretation: FanInterpretation,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EnergyProfile {
    pub name: String,
    pub active: bool,
    pub state: TelemetryState,
    pub provenance: TelemetryProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EnergyProfiles {
    pub profiles: Vec<EnergyProfile>,
}

impl EnergyProfiles {
    pub fn active(&self) -> Option<&EnergyProfile> {
        self.profiles.iter().find(|profile| profile.active)
    }
}

/// Immutable data consumed equally by a CLI or a future GUI.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TelemetrySnapshot {
    pub artifact: TelemetryArtifact,
    pub identity: DeviceIdentity,
    pub profiles: EnergyProfiles,
    pub temperatures: Vec<TemperatureReading>,
    pub frequencies: Vec<FrequencyReading>,
    pub power: Vec<PowerReading>,
    pub fans: Vec<FanTelemetry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelemetryError {
    InconsistentValue { metric: String, reason: String },
}

impl fmt::Display for TelemetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InconsistentValue { metric, reason } => {
                write!(f, "inconsistent simulated telemetry for {metric}: {reason}")
            }
        }
    }
}

impl std::error::Error for TelemetryError {}

impl TelemetrySnapshot {
    /// Validates representation invariants only. It does not validate a
    /// hardware protocol or make a compatibility claim.
    pub fn validate(&self) -> Result<(), TelemetryError> {
        for reading in &self.temperatures {
            validate_unit(&reading.sensor, &reading.reading, TelemetryUnit::Celsius)?;
        }
        for reading in &self.frequencies {
            validate_unit(
                &reading.component,
                &reading.reading,
                TelemetryUnit::Megahertz,
            )?;
        }
        for reading in &self.power {
            validate_unit(&reading.component, &reading.reading, TelemetryUnit::Watts)?;
            let expected = match reading.kind {
                PowerReadingKind::InstantaneousConsumption => {
                    ValueClassification::InstantaneousMeasurement
                }
                PowerReadingKind::ConfiguredLimit => ValueClassification::ConfiguredLimit,
            };
            if reading.reading.classification != expected {
                return Err(TelemetryError::InconsistentValue {
                    metric: reading.component.clone(),
                    reason: format!(
                        "{} must not be represented as {}",
                        reading.kind.as_str(),
                        reading.reading.classification.as_str()
                    ),
                });
            }
        }
        for fan in &self.fans {
            validate_unit(&fan.fan, &fan.reported_speed, TelemetryUnit::Rpm)?;
            match (fan.reported_speed.state, fan.interpretation) {
                (TelemetryState::Unavailable, FanInterpretation::Unavailable)
                | (TelemetryState::ReadError, FanInterpretation::ReadError)
                | (
                    TelemetryState::Available | TelemetryState::Unvalidated,
                    FanInterpretation::ReportedValue | FanInterpretation::NonConclusive,
                ) => {}
                _ => {
                    return Err(TelemetryError::InconsistentValue {
                        metric: fan.fan.clone(),
                        reason: "fan state and interpretation disagree".to_owned(),
                    });
                }
            }
        }
        if self
            .profiles
            .profiles
            .iter()
            .filter(|profile| profile.active)
            .count()
            > 1
        {
            return Err(TelemetryError::InconsistentValue {
                metric: "energy profiles".to_owned(),
                reason: "more than one profile is active".to_owned(),
            });
        }
        Ok(())
    }
}

fn validate_unit<T>(
    label: &str,
    reading: &TelemetryValue<T>,
    expected: TelemetryUnit,
) -> Result<(), TelemetryError> {
    reading.validate_presence(label)?;
    if reading.unit != expected {
        return Err(TelemetryError::InconsistentValue {
            metric: label.to_owned(),
            reason: format!(
                "expected unit {}, found {}",
                expected.as_str(),
                reading.unit.as_str()
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance() -> TelemetryProvenance {
        TelemetryProvenance {
            artifact: TelemetryArtifact::StaticSimulation,
            source: "inline test fixture".to_owned(),
            confidence: ConfidenceLevel::Low,
        }
    }

    fn value<T>(value: Option<T>, unit: TelemetryUnit, state: TelemetryState) -> TelemetryValue<T> {
        TelemetryValue {
            value,
            unit,
            classification: ValueClassification::InstantaneousMeasurement,
            state,
            provenance: provenance(),
        }
    }

    #[test]
    fn preserves_an_unvalidated_zero_rpm_without_claiming_a_stopped_fan() {
        let fan = FanTelemetry {
            fan: "fixture fan".to_owned(),
            reported_speed: value(Some(0), TelemetryUnit::Rpm, TelemetryState::Unvalidated),
            validation: FanValidation::Unvalidated,
            interpretation: FanInterpretation::NonConclusive,
        };
        assert_eq!(fan.reported_speed.value, Some(0));
        assert_eq!(fan.interpretation, FanInterpretation::NonConclusive);
    }

    #[test]
    fn rejects_invalid_value_presence_while_preserving_unvalidated_values() {
        assert!(
            value::<f64>(None, TelemetryUnit::Watts, TelemetryState::Available)
                .validate_presence("available")
                .is_err()
        );
        assert!(
            value(Some(1.0), TelemetryUnit::Watts, TelemetryState::Unavailable)
                .validate_presence("unavailable")
                .is_err()
        );
        assert!(
            value(Some(1.0), TelemetryUnit::Watts, TelemetryState::ReadError)
                .validate_presence("read-error")
                .is_err()
        );
        assert!(
            value(Some(0_u32), TelemetryUnit::Rpm, TelemetryState::Unvalidated)
                .validate_presence("unvalidated zero")
                .is_ok()
        );
    }

    #[test]
    fn rejects_a_power_limit_with_a_frequency_unit() {
        let snapshot = TelemetrySnapshot {
            artifact: TelemetryArtifact::StaticSimulation,
            identity: DeviceIdentity {
                model: "fixture".to_owned(),
                machine_type: "fixture".to_owned(),
                bios_version: "fixture".to_owned(),
                board_name: "fixture".to_owned(),
            },
            profiles: EnergyProfiles { profiles: vec![] },
            temperatures: vec![],
            frequencies: vec![],
            power: vec![PowerReading {
                component: "configured power limit".to_owned(),
                kind: PowerReadingKind::ConfiguredLimit,
                reading: TelemetryValue {
                    value: Some(42.0),
                    unit: TelemetryUnit::Megahertz,
                    classification: ValueClassification::ConfiguredLimit,
                    state: TelemetryState::Available,
                    provenance: provenance(),
                },
            }],
            fans: vec![],
        };
        assert!(matches!(
            snapshot.validate(),
            Err(TelemetryError::InconsistentValue { .. })
        ));
    }

    #[test]
    fn keeps_a_configured_limit_distinct_from_consumption() {
        let reading = value(Some(55.0), TelemetryUnit::Watts, TelemetryState::Available);
        assert_ne!(reading.classification, ValueClassification::ConfiguredLimit);
        let configured = TelemetryValue {
            classification: ValueClassification::ConfiguredLimit,
            ..reading
        };
        assert_eq!(
            configured.classification,
            ValueClassification::ConfiguredLimit
        );
        assert_ne!(
            configured.classification,
            ValueClassification::InstantaneousMeasurement
        );
    }

    #[test]
    fn rejects_a_limit_labelled_as_instantaneous_consumption() {
        let snapshot = TelemetrySnapshot {
            artifact: TelemetryArtifact::StaticSimulation,
            identity: DeviceIdentity {
                model: "fixture".to_owned(),
                machine_type: "fixture".to_owned(),
                bios_version: "fixture".to_owned(),
                board_name: "fixture".to_owned(),
            },
            profiles: EnergyProfiles { profiles: vec![] },
            temperatures: vec![],
            frequencies: vec![],
            power: vec![PowerReading {
                component: "power limit".to_owned(),
                kind: PowerReadingKind::ConfiguredLimit,
                reading: value(Some(55.0), TelemetryUnit::Watts, TelemetryState::Available),
            }],
            fans: vec![],
        };
        assert!(matches!(
            snapshot.validate(),
            Err(TelemetryError::InconsistentValue { .. })
        ));
    }
}
