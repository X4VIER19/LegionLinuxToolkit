//! Planning-only Linux boundary for the AML-confirmed GMZN fan WMI read.
//!
//! The target DSDT proves the request and response byte shapes, but Linux does
//! not provide a userspace interface for `wmidev_evaluate_method()`. A future
//! implementation needs a separately reviewed kernel WMI driver bound to the
//! target GUID. This module deliberately supplies neither that driver nor a
//! host constructor, so it cannot invoke WMI, ACPI, EC, or firmware.

use legion_core::{WmiOperation, WmiRequest, WmiResponse, WmiTransport, WmiTransportError};
use legion_fan::{
    FAN_GET_TABLE_INPUT_SIZE_BYTES, FAN_GET_TABLE_METHOD_ID, FAN_GET_TABLE_RESPONSE_SIZE_BYTES,
    FanGetTableAbiError, FanTableQuery, LENOVO_FAN_METHOD_GUID,
    encode_confirmed_83ex_fan_get_table_request,
};
use serde::Serialize;
use std::fmt;

pub const FAN_WMI_INSTANCE: u8 = 0;

/// Static, auditable description of the only WMI method this phase models.
/// It is not a device handle and cannot perform an invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExperimentalFanWmiReadPlan {
    pub provenance: FanWmiPlanProvenance,
    pub guid: &'static str,
    pub instance: u8,
    pub method_id: u32,
    pub request: [u8; FAN_GET_TABLE_INPUT_SIZE_BYTES],
    pub expected_response_bytes: usize,
    pub status: FanWmiReadStatus,
    pub prerequisites: Vec<FanWmiKernelDependency>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FanWmiPlanProvenance {
    StaticSimulation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FanWmiReadStatus {
    Blocked,
}

/// A requirement that must be resolved before a real firmware call may even
/// be proposed. These are not switches and no API can satisfy them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FanWmiKernelDependency {
    BoundKernelWmiDriver,
    ReadOnlyUserspaceContract,
    ExplicitMethodAuthorization,
    VerifiedHostCompatibility,
}

/// The static plan for the exact Windows-observed coordinates. The query is
/// encoded through the AML-backed codec rather than duplicated here.
pub fn documented_83ex_fan_wmi_read_plan() -> ExperimentalFanWmiReadPlan {
    let query = FanTableQuery::documented_windows_observation();
    ExperimentalFanWmiReadPlan {
        provenance: FanWmiPlanProvenance::StaticSimulation,
        guid: LENOVO_FAN_METHOD_GUID,
        instance: FAN_WMI_INSTANCE,
        method_id: FAN_GET_TABLE_METHOD_ID,
        request: encode_confirmed_83ex_fan_get_table_request(query),
        expected_response_bytes: FAN_GET_TABLE_RESPONSE_SIZE_BYTES,
        status: FanWmiReadStatus::Blocked,
        prerequisites: vec![
            FanWmiKernelDependency::BoundKernelWmiDriver,
            FanWmiKernelDependency::ReadOnlyUserspaceContract,
            FanWmiKernelDependency::ExplicitMethodAuthorization,
            FanWmiKernelDependency::VerifiedHostCompatibility,
        ],
    }
}

/// Distinct future failure classes. No production transport exists yet, but a
/// future implementation must retain these boundaries rather than treating
/// malformed ACPI output as a compatibility or decoding success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxFanWmiReadError {
    Compatibility(FanWmiCompatibilityError),
    Transport(WmiTransportError),
    AcpiResponse(FanWmiAcpiResponseError),
    Decoding(FanGetTableAbiError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanWmiCompatibilityError {
    HostIdentityNotVerified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FanWmiAcpiResponseError {
    MissingObject,
    UnexpectedObjectType { observed: String },
}

impl fmt::Display for LinuxFanWmiReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compatibility(error) => write!(f, "fan WMI compatibility error: {error}"),
            Self::Transport(error) => write!(f, "fan WMI transport error: {error}"),
            Self::AcpiResponse(error) => write!(f, "fan WMI ACPI response error: {error}"),
            Self::Decoding(error) => write!(f, "fan WMI decoding error: {error}"),
        }
    }
}

impl fmt::Display for FanWmiCompatibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HostIdentityNotVerified => write!(f, "host identity is not verified"),
        }
    }
}

impl fmt::Display for FanWmiAcpiResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingObject => write!(f, "ACPI method returned no object"),
            Self::UnexpectedObjectType { observed } => {
                write!(f, "ACPI method returned {observed}, not a buffer")
            }
        }
    }
}

impl std::error::Error for LinuxFanWmiReadError {}
impl std::error::Error for FanWmiCompatibilityError {}
impl std::error::Error for FanWmiAcpiResponseError {}

/// Experimental transport shape with no production constructor.
///
/// Its private field prevents external crates from constructing it. This crate
/// exposes no non-test constructor, so normal applications, including
/// `legion-cli`, cannot select it. Its `WmiTransport` implementation always
/// fails closed before any attempted host operation.
#[derive(Debug, Clone, Copy)]
pub struct ExperimentalLinuxFanWmiTransport {
    _private: (),
}

impl ExperimentalLinuxFanWmiTransport {
    #[cfg(test)]
    const fn blocked() -> Self {
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

        Err(WmiTransportError::MethodUnavailable {
            operation: request.operation.label().to_owned(),
            reason: "blocked: no reviewed kernel WMI driver or read-only userspace contract exists for the AML-confirmed Fan_Get_Table ABI".to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_core::{WmiAccess, WmiArtifact, WmiKnowledge, WmiMethodMetadata, WmiProvenance};
    use legion_fan::{FanTableWmiAdapter, FanTableWmiError, documented_windows_table_evidence};

    #[test]
    fn plan_uses_only_the_aml_confirmed_target_and_buffers() {
        let plan = documented_83ex_fan_wmi_read_plan();
        assert_eq!(plan.provenance, FanWmiPlanProvenance::StaticSimulation);
        assert_eq!(plan.guid, "92549549-4BDE-4F06-AC04-CE8BF898DBAA");
        assert_eq!(plan.instance, 0);
        assert_eq!(plan.method_id, 5);
        assert_eq!(plan.request, [1, 4]);
        assert_eq!(plan.expected_response_bytes, 88);
        assert_eq!(plan.status, FanWmiReadStatus::Blocked);
        assert!(
            plan.prerequisites
                .contains(&FanWmiKernelDependency::BoundKernelWmiDriver)
        );
    }

    #[test]
    fn transport_fails_closed_before_any_host_operation() {
        let transport = ExperimentalLinuxFanWmiTransport::blocked();
        let request = WmiRequest {
            operation: WmiOperation::FanGetTable,
            access: WmiAccess::ReadOnly,
            arguments: Vec::new(),
            method: WmiMethodMetadata {
                observed_name: Some("Fan_Get_Table".to_owned()),
                method_identifier: Some("5".to_owned()),
                request_abi: WmiKnowledge::Observed,
                response_abi: WmiKnowledge::Observed,
            },
            provenance: WmiProvenance {
                artifact: WmiArtifact::StaticSimulation,
                observed_on: legion_core::WmiObservationPlatform::Linux,
                source: "test".to_owned(),
                bios_version: Some("NUCN15WW".to_owned()),
                knowledge: WmiKnowledge::Observed,
            },
        };
        assert!(matches!(
            transport.execute(&request),
            Err(WmiTransportError::MethodUnavailable { .. })
        ));
    }

    #[test]
    fn shared_fan_adapter_receives_a_transport_failure_not_a_reading() {
        let adapter = FanTableWmiAdapter::new(ExperimentalLinuxFanWmiTransport::blocked());
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
    fn error_classes_keep_compatibility_transport_acpi_and_decoding_distinct() {
        let compatibility =
            LinuxFanWmiReadError::Compatibility(FanWmiCompatibilityError::HostIdentityNotVerified);
        let transport = LinuxFanWmiReadError::Transport(WmiTransportError::MethodUnavailable {
            operation: "Fan_Get_Table".to_owned(),
            reason: "test".to_owned(),
        });
        let acpi = LinuxFanWmiReadError::AcpiResponse(FanWmiAcpiResponseError::MissingObject);
        let acpi_type =
            LinuxFanWmiReadError::AcpiResponse(FanWmiAcpiResponseError::UnexpectedObjectType {
                observed: "integer".to_owned(),
            });
        let decoding =
            LinuxFanWmiReadError::Decoding(FanGetTableAbiError::UnexpectedResponseLength {
                actual: 0,
            });

        assert!(matches!(
            compatibility,
            LinuxFanWmiReadError::Compatibility(_)
        ));
        assert!(matches!(transport, LinuxFanWmiReadError::Transport(_)));
        assert!(matches!(acpi, LinuxFanWmiReadError::AcpiResponse(_)));
        assert!(matches!(acpi_type, LinuxFanWmiReadError::AcpiResponse(_)));
        assert!(matches!(decoding, LinuxFanWmiReadError::Decoding(_)));
    }

    #[test]
    fn transport_rejects_every_other_operation() {
        let transport = ExperimentalLinuxFanWmiTransport::blocked();
        let request = WmiRequest {
            operation: WmiOperation::OtherReadOnly {
                label: "other".to_owned(),
            },
            access: WmiAccess::ReadOnly,
            arguments: Vec::new(),
            method: WmiMethodMetadata {
                observed_name: None,
                method_identifier: None,
                request_abi: WmiKnowledge::Unknown,
                response_abi: WmiKnowledge::Unknown,
            },
            provenance: WmiProvenance {
                artifact: WmiArtifact::StaticSimulation,
                observed_on: legion_core::WmiObservationPlatform::NotApplicable,
                source: "test".to_owned(),
                bios_version: None,
                knowledge: WmiKnowledge::Unknown,
            },
        };
        assert!(matches!(
            transport.execute(&request),
            Err(WmiTransportError::UnsupportedOperation { .. })
        ));
    }
}
