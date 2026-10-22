//! Injected Linux host-identity acquisition.
//!
//! This module deliberately has no host reader. Its only public source is an
//! in-memory simulation, and every successful result is constructed through
//! `HostIdentity::declared_simulation`.

use legion_core::{HostIdentity, HostIdentityError, HostIdentityInput};
use std::fmt;

/// Fields a future, separately approved Linux adapter may acquire. The enum
/// describes acquisition targets; it does not contain a path or a reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostIdentityField {
    Manufacturer,
    ProductIdentifier,
    Model,
    BiosVersion,
    KernelVersion,
    Distribution,
}

impl HostIdentityField {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Manufacturer => "manufacturer",
            Self::ProductIdentifier => "product_identifier",
            Self::Model => "model",
            Self::BiosVersion => "bios_version",
            Self::KernelVersion => "kernel_version",
            Self::Distribution => "distribution",
        }
    }
}

/// Acquisition failures remain distinct from identity validation and
/// compatibility evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostIdentitySourceError {
    Missing { field: HostIdentityField },
    Unavailable { reason: String },
}

impl fmt::Display for HostIdentitySourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { field } => {
                write!(f, "missing identity source field: {}", field.label())
            }
            Self::Unavailable { reason } => write!(f, "identity source is unavailable: {reason}"),
        }
    }
}

impl std::error::Error for HostIdentitySourceError {}

/// Error boundary for the acquisition and validation stages. Compatibility is
/// intentionally not evaluated here: callers pass a valid static identity to
/// the existing `HostCompatibilityEvaluator` in `legion-core`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxHostIdentityError {
    Source(HostIdentitySourceError),
    Validation(HostIdentityError),
}

impl fmt::Display for LinuxHostIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(f),
            Self::Validation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for LinuxHostIdentityError {}

impl From<HostIdentitySourceError> for LinuxHostIdentityError {
    fn from(error: HostIdentitySourceError) -> Self {
        Self::Source(error)
    }
}

impl From<HostIdentityError> for LinuxHostIdentityError {
    fn from(error: HostIdentityError) -> Self {
        Self::Validation(error)
    }
}

/// Supplies an identity input without accepting paths, environment values, or
/// configuration. It is sealed so downstream code cannot add a host-reading
/// provider before that work is specifically reviewed.
pub trait HostIdentitySource: private::Sealed {
    fn acquire(&self) -> Result<HostIdentityInput, HostIdentitySourceError>;

    /// This phase permits only a diagnostic label for static simulation.
    fn simulation_label(&self) -> &str;
}

mod private {
    pub trait Sealed {}
}

/// One explicit acquisition over an injected source. It has no polling,
/// retries, cache, real-origin constructor, or authorization API.
#[derive(Debug, Clone)]
pub struct LinuxHostIdentityProvider<S> {
    source: S,
}

impl<S> LinuxHostIdentityProvider<S>
where
    S: HostIdentitySource,
{
    pub fn new(source: S) -> Self {
        Self { source }
    }

    pub fn read_once(&self) -> Result<HostIdentity, LinuxHostIdentityError> {
        let input = self.source.acquire()?;
        HostIdentity::declared_simulation(input, self.source.simulation_label()).map_err(Into::into)
    }
}

/// Closed in-memory source for tests and future UI/CLI simulation flows.
/// Its label is provenance only and never a filesystem path.
#[derive(Debug, Clone)]
pub struct SimulatedLinuxHostIdentitySource {
    label: String,
    outcome: Result<HostIdentityInput, HostIdentitySourceError>,
}

impl SimulatedLinuxHostIdentitySource {
    pub fn complete(label: impl Into<String>, input: HostIdentityInput) -> Self {
        Self {
            label: label.into(),
            outcome: Ok(input),
        }
    }

    pub fn failing(label: impl Into<String>, error: HostIdentitySourceError) -> Self {
        Self {
            label: label.into(),
            outcome: Err(error),
        }
    }
}

impl HostIdentitySource for SimulatedLinuxHostIdentitySource {
    fn acquire(&self) -> Result<HostIdentityInput, HostIdentitySourceError> {
        self.outcome.clone()
    }

    fn simulation_label(&self) -> &str {
        &self.label
    }
}

impl private::Sealed for SimulatedLinuxHostIdentitySource {}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_core::HostIdentityOrigin;

    fn complete_input() -> HostIdentityInput {
        HostIdentityInput {
            manufacturer: Some("Lenovo".to_owned()),
            product_identifier: Some("83EX".to_owned()),
            model: Some("Legion Slim 5 16ARP9".to_owned()),
            bios_version: Some("NUCN15WW".to_owned()),
            kernel_version: Some("7.2.0-202.nobara.fc44.x86_64".to_owned()),
            distribution: Some("Nobara Linux".to_owned()),
        }
    }

    #[test]
    fn complete_injected_source_produces_only_static_simulation() {
        let provider = LinuxHostIdentityProvider::new(SimulatedLinuxHostIdentitySource::complete(
            "test::complete",
            complete_input(),
        ));
        let identity = provider.read_once().unwrap();
        assert!(matches!(
            identity.origin(),
            HostIdentityOrigin::StaticSimulation { label } if label == "test::complete"
        ));
    }

    #[test]
    fn missing_source_field_is_not_rewritten_as_an_identity_value() {
        let provider = LinuxHostIdentityProvider::new(SimulatedLinuxHostIdentitySource::failing(
            "test::missing",
            HostIdentitySourceError::Missing {
                field: HostIdentityField::BiosVersion,
            },
        ));
        assert!(matches!(
            provider.read_once(),
            Err(LinuxHostIdentityError::Source(
                HostIdentitySourceError::Missing {
                    field: HostIdentityField::BiosVersion,
                }
            ))
        ));
    }

    #[test]
    fn absent_input_field_remains_absent_for_compatibility_evaluation() {
        let mut input = complete_input();
        input.kernel_version = None;
        let provider = LinuxHostIdentityProvider::new(SimulatedLinuxHostIdentitySource::complete(
            "test::absent-input",
            input,
        ));
        assert_eq!(provider.read_once().unwrap().kernel_version(), None);
    }

    #[test]
    fn acquisition_error_is_distinct_from_validation_error() {
        let unavailable =
            LinuxHostIdentityProvider::new(SimulatedLinuxHostIdentitySource::failing(
                "test::failure",
                HostIdentitySourceError::Unavailable {
                    reason: "synthetic failure".to_owned(),
                },
            ));
        assert!(matches!(
            unavailable.read_once(),
            Err(LinuxHostIdentityError::Source(
                HostIdentitySourceError::Unavailable { .. }
            ))
        ));

        let mut malformed = complete_input();
        malformed.model = Some("invalid\nmodel".to_owned());
        let invalid = LinuxHostIdentityProvider::new(SimulatedLinuxHostIdentitySource::complete(
            "test::invalid",
            malformed,
        ));
        assert!(matches!(
            invalid.read_once(),
            Err(LinuxHostIdentityError::Validation(
                HostIdentityError::Malformed { field: "model", .. }
            ))
        ));
    }
}
