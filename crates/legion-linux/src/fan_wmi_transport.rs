//! Feature-gated, fail-closed preparation for the GMZN fan WMI method.
//!
//! This is deliberately not a Linux WMI implementation. The target MOF
//! confirms the method schema, but not the ACPI byte encoding of its inputs or
//! outputs. Consequently this transport cannot construct a buffer, locate a
//! WMI device, or invoke firmware; it only supplies a typed failure to the
//! shared simulated fan adapter.

use legion_core::{WmiOperation, WmiRequest, WmiResponse, WmiTransport, WmiTransportError};
#[cfg(test)]
use legion_fan::{
    FanTableQuery, FanTableWmiAdapter, FanTableWmiError, SimulatedFanTableWmiResult,
    documented_windows_table_evidence,
};

/// Experimental transport shape with no production constructor.
///
/// The type is public only so that the feature-gated transport boundary is
/// visible to downstream architecture review. Its private field prevents
/// external crates from constructing it. This crate exposes no non-test
/// constructor, so normal applications, including `legion-cli`, cannot select
/// it.
#[derive(Debug, Clone, Copy)]
pub struct ExperimentalLinuxFanWmiTransport {
    _private: (),
}

impl ExperimentalLinuxFanWmiTransport {
    #[cfg(test)]
    const fn disabled() -> Self {
        Self { _private: () }
    }
}

impl WmiTransport for ExperimentalLinuxFanWmiTransport {
    fn execute(&self, request: &WmiRequest) -> Result<WmiResponse, WmiTransportError> {
        if !matches!(request.operation, WmiOperation::FanGetTable) {
            return Err(WmiTransportError::UnsupportedOperation {
                operation: request.operation.label().to_owned(),
            });
        }

        // The only supported operation is intentionally refused. The target
        // MOF gives names, IDs, types and logical parameter IDs, but neither
        // an ACPI input layout nor a binary response contract.
        Err(WmiTransportError::MethodUnavailable {
            operation: request.operation.label().to_owned(),
            reason: "experimental Linux fan WMI transport is disabled: GMZN ACPI buffer ABI remains unknown".to_owned(),
        })
    }
}

/// Test-only composition point proving that the shared fan adapter receives a
/// transport failure rather than a synthesized buffer or an apparent reading.
#[cfg(test)]
fn simulated_attempt() -> Result<SimulatedFanTableWmiResult, FanTableWmiError> {
    FanTableWmiAdapter::new(ExperimentalLinuxFanWmiTransport::disabled()).query(
        FanTableQuery::documented_windows_observation(),
        documented_windows_table_evidence(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_transport_fails_closed_without_constructing_an_acpi_buffer() {
        assert!(matches!(
            simulated_attempt(),
            Err(FanTableWmiError::Transport(WmiTransportError::MethodUnavailable {
                operation,
                ..
            })) if operation == "Fan_Get_Table"
        ));
    }

    #[test]
    fn fan_transport_rejects_every_non_fan_get_table_operation() {
        let transport = ExperimentalLinuxFanWmiTransport::disabled();
        let request = WmiRequest {
            operation: WmiOperation::OtherReadOnly {
                label: "other".to_owned(),
            },
            access: legion_core::WmiAccess::ReadOnly,
            arguments: Vec::new(),
            method: legion_core::WmiMethodMetadata {
                observed_name: None,
                method_identifier: None,
                request_abi: legion_core::WmiKnowledge::Unknown,
                response_abi: legion_core::WmiKnowledge::Unknown,
            },
            provenance: legion_core::WmiProvenance {
                artifact: legion_core::WmiArtifact::StaticSimulation,
                observed_on: legion_core::WmiObservationPlatform::NotApplicable,
                source: "test".to_owned(),
                bios_version: None,
                knowledge: legion_core::WmiKnowledge::Unknown,
            },
        };
        assert!(matches!(
            transport.execute(&request),
            Err(WmiTransportError::UnsupportedOperation { .. })
        ));
    }
}
