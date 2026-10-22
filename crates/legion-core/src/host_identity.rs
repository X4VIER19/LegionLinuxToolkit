//! Injected host-identity models and strict compatibility evaluation.
//!
//! These types represent declared data only. They intentionally contain no
//! Linux discovery, filesystem access, command execution, or authorization for
//! a real hardware operation.

use crate::{CompatibilityRegistry, DeviceQuery, MatchResult};
use serde::Serialize;
use std::fmt;

/// Caller-supplied identity fields. `None` means that a field was not supplied;
/// it is never replaced with a default value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct HostIdentityInput {
    pub manufacturer: Option<String>,
    pub product_identifier: Option<String>,
    pub model: Option<String>,
    pub bios_version: Option<String>,
    pub kernel_version: Option<String>,
    pub distribution: Option<String>,
}

/// Origin of identity data. Host observation is deliberately modelled as
/// unavailable until a separately approved adapter exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum HostIdentityOrigin {
    StaticSimulation { label: String },
    LinuxDmiObservedUnverified,
    HostObservationUnavailable,
}

/// A valid, injected identity. The fields remain private so construction must
/// validate supplied strings and retain the origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostIdentity {
    manufacturer: Option<String>,
    product_identifier: Option<String>,
    model: Option<String>,
    bios_version: Option<String>,
    kernel_version: Option<String>,
    distribution: Option<String>,
    origin: HostIdentityOrigin,
}

impl HostIdentity {
    /// Creates an identity from in-memory declared data only. This constructor
    /// cannot produce an observed or verified host identity.
    pub fn declared_simulation(
        input: HostIdentityInput,
        label: impl Into<String>,
    ) -> Result<Self, HostIdentityError> {
        let label = label.into();
        validate_value("simulation_label", &label)?;
        Ok(Self {
            manufacturer: validate_optional("manufacturer", input.manufacturer)?,
            product_identifier: validate_optional("product_identifier", input.product_identifier)?,
            model: validate_optional("model", input.model)?,
            bios_version: validate_optional("bios_version", input.bios_version)?,
            kernel_version: validate_optional("kernel_version", input.kernel_version)?,
            distribution: validate_optional("distribution", input.distribution)?,
            origin: HostIdentityOrigin::StaticSimulation { label },
        })
    }

    /// Creates an observed-but-unverified identity from a Linux DMI adapter.
    /// It is intentionally not an authorization and cannot produce one.
    pub fn observed_linux_dmi_unverified(
        input: HostIdentityInput,
    ) -> Result<Self, HostIdentityError> {
        Ok(Self {
            manufacturer: validate_optional("manufacturer", input.manufacturer)?,
            product_identifier: validate_optional("product_identifier", input.product_identifier)?,
            model: validate_optional("model", input.model)?,
            bios_version: validate_optional("bios_version", input.bios_version)?,
            kernel_version: validate_optional("kernel_version", input.kernel_version)?,
            distribution: validate_optional("distribution", input.distribution)?,
            origin: HostIdentityOrigin::LinuxDmiObservedUnverified,
        })
    }

    pub fn manufacturer(&self) -> Option<&str> {
        self.manufacturer.as_deref()
    }

    pub fn product_identifier(&self) -> Option<&str> {
        self.product_identifier.as_deref()
    }

    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn bios_version(&self) -> Option<&str> {
        self.bios_version.as_deref()
    }

    pub fn kernel_version(&self) -> Option<&str> {
        self.kernel_version.as_deref()
    }

    pub fn distribution(&self) -> Option<&str> {
        self.distribution.as_deref()
    }

    pub fn origin(&self) -> &HostIdentityOrigin {
        &self.origin
    }

    fn device_query(&self) -> DeviceQuery {
        DeviceQuery {
            manufacturer: self.manufacturer.clone(),
            model: self.model.clone(),
            machine_type: self.product_identifier.clone(),
            bios_version: self.bios_version.clone(),
            dmi_identifier: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostIdentityError {
    Malformed { field: &'static str, value: String },
}

impl fmt::Display for HostIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { field, .. } => write!(f, "malformed host identity field: {field}"),
        }
    }
}

impl std::error::Error for HostIdentityError {}

/// Static environmental evidence associated with a profile already present in
/// `CompatibilityRegistry`. It is not a second hardware catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostCompatibilityEnvironment {
    pub profile_id: String,
    pub kernel_version: String,
    pub distribution: String,
}

impl HostCompatibilityEnvironment {
    pub fn declared_simulation(
        profile_id: impl Into<String>,
        kernel_version: impl Into<String>,
        distribution: impl Into<String>,
    ) -> Result<Self, HostIdentityError> {
        let profile_id = profile_id.into();
        let kernel_version = kernel_version.into();
        let distribution = distribution.into();
        validate_value("profile_id", &profile_id)?;
        validate_value("kernel_version", &kernel_version)?;
        validate_value("distribution", &distribution)?;
        Ok(Self {
            profile_id,
            kernel_version,
            distribution,
        })
    }
}

/// Compatibility conclusions from supplied data. A compatible simulated match
/// is descriptive only and carries an unforgeable-in-this-phase verification
/// marker rather than a caller-controlled boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostIdentityVerification {
    UnverifiedStaticSimulation,
    ObservedUnverifiedLinuxDmi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "result", rename_all = "kebab-case")]
pub enum HostCompatibilityResult {
    Compatible {
        profile_id: String,
        origin: HostIdentityOrigin,
        verification: HostIdentityVerification,
    },
    Incompatible {
        reason: String,
    },
    Unknown {
        reason: String,
    },
    InsufficientEvidence {
        missing_fields: Vec<String>,
    },
    Contradictory {
        reason: String,
    },
}

impl HostCompatibilityResult {
    pub fn host_identity_verified(&self) -> bool {
        false
    }
}

/// Reserved for a future, separately reviewed host-observation phase. Its
/// private field and absence of constructors keep it impossible to fabricate
/// from safe external code; this evaluator never returns one.
#[derive(Debug)]
pub struct RealReadAuthorization {
    _private: (),
}

/// Evaluates an injected identity against the existing strict registry.
#[derive(Debug, Clone, Copy)]
pub struct HostCompatibilityEvaluator<'a> {
    registry: &'a CompatibilityRegistry,
    environment: &'a HostCompatibilityEnvironment,
}

impl<'a> HostCompatibilityEvaluator<'a> {
    pub fn new(
        registry: &'a CompatibilityRegistry,
        environment: &'a HostCompatibilityEnvironment,
    ) -> Self {
        Self {
            registry,
            environment,
        }
    }

    pub fn evaluate(&self, identity: &HostIdentity) -> HostCompatibilityResult {
        let query = identity.device_query();
        match self.registry.match_query(&query) {
            MatchResult::Exact { profile } => {
                if profile.id != self.environment.profile_id {
                    return HostCompatibilityResult::Unknown {
                        reason:
                            "the injected environment is not associated with the matched profile"
                                .to_owned(),
                    };
                }
                let environment_missing = [
                    ("kernel_version", identity.kernel_version().is_none()),
                    ("distribution", identity.distribution().is_none()),
                ]
                .into_iter()
                .filter_map(|(field, missing)| missing.then_some(field.to_owned()))
                .collect::<Vec<_>>();
                if !environment_missing.is_empty() {
                    return HostCompatibilityResult::InsufficientEvidence {
                        missing_fields: environment_missing,
                    };
                }
                if identity.kernel_version() != Some(self.environment.kernel_version.as_str())
                    || identity.distribution() != Some(self.environment.distribution.as_str())
                {
                    return HostCompatibilityResult::Unknown {
                        reason: "injected kernel or distribution is not covered by the declared profile evidence"
                            .to_owned(),
                    };
                }
                HostCompatibilityResult::Compatible {
                    profile_id: profile.id,
                    origin: identity.origin.clone(),
                    verification: match identity.origin() {
                        HostIdentityOrigin::LinuxDmiObservedUnverified => {
                            HostIdentityVerification::ObservedUnverifiedLinuxDmi
                        }
                        _ => HostIdentityVerification::UnverifiedStaticSimulation,
                    },
                }
            }
            MatchResult::Incomplete { missing_fields } => {
                HostCompatibilityResult::InsufficientEvidence { missing_fields }
            }
            MatchResult::Ambiguous { .. } => HostCompatibilityResult::Unknown {
                reason: "multiple static compatibility profiles matched".to_owned(),
            },
            MatchResult::Unknown { .. } => self.classify_non_match(identity),
        }
    }

    fn classify_non_match(&self, identity: &HostIdentity) -> HostCompatibilityResult {
        let same_manufacturer =
            self.registry.profiles.iter().any(|profile| {
                identity.manufacturer() == Some(profile.identity.manufacturer.as_str())
            });
        if !same_manufacturer {
            return HostCompatibilityResult::Incompatible {
                reason: "manufacturer does not match any static compatibility profile".to_owned(),
            };
        }

        let shares_product_or_model = self.registry.profiles.iter().any(|profile| {
            identity.product_identifier() == Some(profile.identity.machine_type.as_str())
                || identity.model() == Some(profile.identity.model.as_str())
        });
        if shares_product_or_model {
            HostCompatibilityResult::Contradictory {
                reason:
                    "declared identity conflicts with a static profile on another required field"
                        .to_owned(),
            }
        } else {
            HostCompatibilityResult::Unknown {
                reason: "no static compatibility profile matches the declared model".to_owned(),
            }
        }
    }
}

fn validate_optional(
    field: &'static str,
    value: Option<String>,
) -> Result<Option<String>, HostIdentityError> {
    value
        .map(|value| {
            validate_value(field, &value)?;
            Ok(value)
        })
        .transpose()
}

fn validate_value(field: &'static str, value: &str) -> Result<(), HostIdentityError> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(HostIdentityError::Malformed {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompatibilityProfile, DeviceDescriptor};

    fn registry() -> CompatibilityRegistry {
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
                capabilities: vec![],
            }],
        }
    }

    fn environment() -> HostCompatibilityEnvironment {
        HostCompatibilityEnvironment::declared_simulation(
            "lenovo-legion-slim-5-16arp9-83ex-nucn15ww",
            "7.2.0-202.nobara.fc44.x86_64",
            "Nobara Linux",
        )
        .unwrap()
    }

    fn exact_identity() -> HostIdentity {
        HostIdentity::declared_simulation(
            HostIdentityInput {
                manufacturer: Some("Lenovo".to_owned()),
                product_identifier: Some("83EX".to_owned()),
                model: Some("Legion Slim 5 16ARP9".to_owned()),
                bios_version: Some("NUCN15WW".to_owned()),
                kernel_version: Some("7.2.0-202.nobara.fc44.x86_64".to_owned()),
                distribution: Some("Nobara Linux".to_owned()),
            },
            "host-identity::exact",
        )
        .unwrap()
    }

    #[test]
    fn exact_declared_identity_is_compatible_but_never_verified() {
        let identity = exact_identity();
        let result =
            HostCompatibilityEvaluator::new(&registry(), &environment()).evaluate(&identity);
        assert!(matches!(
            result,
            HostCompatibilityResult::Compatible {
                verification: HostIdentityVerification::UnverifiedStaticSimulation,
                origin: HostIdentityOrigin::StaticSimulation { .. },
                ..
            }
        ));
        assert!(!result.host_identity_verified());
    }

    #[test]
    fn missing_identity_fields_are_insufficient_evidence() {
        let identity =
            HostIdentity::declared_simulation(HostIdentityInput::default(), "missing").unwrap();
        assert!(matches!(
            HostCompatibilityEvaluator::new(&registry(), &environment()).evaluate(&identity),
            HostCompatibilityResult::InsufficientEvidence { .. }
        ));
    }

    #[test]
    fn absent_environment_is_insufficient_evidence_not_incompatible() {
        let mut input = exact_identity_input();
        input.kernel_version = None;
        let identity = HostIdentity::declared_simulation(input, "missing-environment").unwrap();
        assert!(matches!(
            HostCompatibilityEvaluator::new(&registry(), &environment()).evaluate(&identity),
            HostCompatibilityResult::InsufficientEvidence { missing_fields }
                if missing_fields == ["kernel_version"]
        ));
    }

    #[test]
    fn malformed_values_are_rejected_without_defaults() {
        assert!(matches!(
            HostIdentity::declared_simulation(
                HostIdentityInput {
                    model: Some("Legion\nSlim".to_owned()),
                    ..HostIdentityInput::default()
                },
                "malformed",
            ),
            Err(HostIdentityError::Malformed { field: "model", .. })
        ));
    }

    #[test]
    fn different_bios_is_contradictory_not_compatible() {
        let mut input = exact_identity_input();
        input.bios_version = Some("OTHER".to_owned());
        let identity = HostIdentity::declared_simulation(input, "bios-mismatch").unwrap();
        assert!(matches!(
            HostCompatibilityEvaluator::new(&registry(), &environment()).evaluate(&identity),
            HostCompatibilityResult::Contradictory { .. }
        ));
    }

    #[test]
    fn different_product_identifier_is_contradictory_not_compatible() {
        let mut input = exact_identity_input();
        input.product_identifier = Some("OTHER".to_owned());
        let identity = HostIdentity::declared_simulation(input, "product-mismatch").unwrap();
        assert!(matches!(
            HostCompatibilityEvaluator::new(&registry(), &environment()).evaluate(&identity),
            HostCompatibilityResult::Contradictory { .. }
        ));
    }

    #[test]
    fn unknown_model_is_not_promoted_to_compatible() {
        let mut input = exact_identity_input();
        input.product_identifier = Some("UNKNOWN".to_owned());
        input.model = Some("Unknown Legion".to_owned());
        let identity = HostIdentity::declared_simulation(input, "unknown-model").unwrap();
        assert!(matches!(
            HostCompatibilityEvaluator::new(&registry(), &environment()).evaluate(&identity),
            HostCompatibilityResult::Unknown { .. }
        ));
    }

    #[test]
    fn another_manufacturer_is_incompatible() {
        let mut input = exact_identity_input();
        input.manufacturer = Some("Other".to_owned());
        let identity = HostIdentity::declared_simulation(input, "other-manufacturer").unwrap();
        assert!(matches!(
            HostCompatibilityEvaluator::new(&registry(), &environment()).evaluate(&identity),
            HostCompatibilityResult::Incompatible { .. }
        ));
    }

    fn exact_identity_input() -> HostIdentityInput {
        HostIdentityInput {
            manufacturer: Some("Lenovo".to_owned()),
            product_identifier: Some("83EX".to_owned()),
            model: Some("Legion Slim 5 16ARP9".to_owned()),
            bios_version: Some("NUCN15WW".to_owned()),
            kernel_version: Some("7.2.0-202.nobara.fc44.x86_64".to_owned()),
            distribution: Some("Nobara Linux".to_owned()),
        }
    }
}
