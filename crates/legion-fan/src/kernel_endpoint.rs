//! Decoder for injected bytes of the proposed binary fan kernel endpoint.
//!
//! No endpoint exists at runtime in this phase. The decoder has no filesystem
//! or device APIs and always marks its parsed input as a static simulation.

use crate::{
    FAN_GET_TABLE_RESPONSE_SIZE_BYTES, FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES,
    KernelFanReadContractError, KernelFanReadStatus, SimulatedKernelFanReadResponse,
    SimulatedKernelFanReadResult, consume_simulated_kernel_fan_read,
};
use legion_core::TelemetryArtifact;
use std::fmt;

pub const FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES: usize =
    FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES as usize + FAN_GET_TABLE_RESPONSE_SIZE_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelFanEndpointError {
    UnexpectedWireLength { actual: usize },
    UnknownStatus { actual: u32 },
    PayloadTooLarge { declared: usize },
    Contract(KernelFanReadContractError),
}

impl fmt::Display for KernelFanEndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedWireLength { actual } => write!(
                f,
                "fan kernel endpoint has {actual} bytes; expected {FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES}"
            ),
            Self::UnknownStatus { actual } => {
                write!(f, "fan kernel endpoint returned unknown status {actual}")
            }
            Self::PayloadTooLarge { declared } => write!(
                f,
                "fan kernel endpoint declares {declared} payload bytes; maximum is {FAN_GET_TABLE_RESPONSE_SIZE_BYTES}"
            ),
            Self::Contract(error) => write!(f, "fan kernel endpoint contract error: {error}"),
        }
    }
}

impl std::error::Error for KernelFanEndpointError {}

/// Decodes one fully acquired fixture of the binary endpoint contract.
/// Truncated data, unknown statuses, and contract errors never produce a
/// successful result.
pub fn consume_simulated_kernel_endpoint_bytes(
    bytes: &[u8],
) -> Result<SimulatedKernelFanReadResult, KernelFanEndpointError> {
    if bytes.len() != FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES {
        return Err(KernelFanEndpointError::UnexpectedWireLength {
            actual: bytes.len(),
        });
    }
    let version = u16::from_le_bytes([bytes[0], bytes[1]]);
    let header_size = u16::from_le_bytes([bytes[2], bytes[3]]);
    let raw_status = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let status = status_from_raw(raw_status)
        .ok_or(KernelFanEndpointError::UnknownStatus { actual: raw_status })?;
    let payload_size = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    if payload_size > FAN_GET_TABLE_RESPONSE_SIZE_BYTES {
        return Err(KernelFanEndpointError::PayloadTooLarge {
            declared: payload_size,
        });
    }

    let response = SimulatedKernelFanReadResponse {
        version,
        header_size,
        status,
        payload_size,
        payload: bytes[FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES as usize
            ..FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES as usize + payload_size]
            .to_vec(),
        provenance: TelemetryArtifact::StaticSimulation,
    };
    consume_simulated_kernel_fan_read(response).map_err(KernelFanEndpointError::Contract)
}

fn status_from_raw(raw: u32) -> Option<KernelFanReadStatus> {
    match raw {
        0 => Some(KernelFanReadStatus::Ok),
        1 => Some(KernelFanReadStatus::Unsupported),
        2 => Some(KernelFanReadStatus::TransportError),
        3 => Some(KernelFanReadStatus::AcpiResponseError),
        4 => Some(KernelFanReadStatus::AbiError),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FAN_KERNEL_READ_CONTRACT_VERSION;

    fn endpoint_fixture(version: u16, status: u32, payload_size: u32) -> Vec<u8> {
        let mut bytes = vec![0; FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES];
        bytes[0..2].copy_from_slice(&version.to_le_bytes());
        bytes[2..4].copy_from_slice(&FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES.to_le_bytes());
        bytes[4..8].copy_from_slice(&status.to_le_bytes());
        bytes[8..12].copy_from_slice(&payload_size.to_le_bytes());
        bytes[12..16].copy_from_slice(&10_u32.to_le_bytes());
        bytes[56..60].copy_from_slice(&10_u32.to_le_bytes());
        bytes
    }

    #[test]
    fn accepts_a_full_static_success_response() {
        let bytes = endpoint_fixture(FAN_KERNEL_READ_CONTRACT_VERSION, 0, 88);
        let result = consume_simulated_kernel_endpoint_bytes(&bytes).unwrap();
        assert_eq!(
            result.response.provenance,
            TelemetryArtifact::StaticSimulation
        );
        assert_eq!(result.decoded.original.len(), 88);
    }

    #[test]
    fn reports_a_kernel_status_without_decoding_it() {
        let bytes = endpoint_fixture(FAN_KERNEL_READ_CONTRACT_VERSION, 1, 0);
        assert_eq!(
            consume_simulated_kernel_endpoint_bytes(&bytes),
            Err(KernelFanEndpointError::Contract(
                KernelFanReadContractError::Status(KernelFanReadStatus::Unsupported)
            ))
        );
    }

    #[test]
    fn distinguishes_a_simulated_wmi_failure_from_an_acpi_response_failure() {
        let transport = endpoint_fixture(FAN_KERNEL_READ_CONTRACT_VERSION, 2, 0);
        assert_eq!(
            consume_simulated_kernel_endpoint_bytes(&transport),
            Err(KernelFanEndpointError::Contract(
                KernelFanReadContractError::Status(KernelFanReadStatus::TransportError)
            ))
        );

        let acpi = endpoint_fixture(FAN_KERNEL_READ_CONTRACT_VERSION, 3, 0);
        assert_eq!(
            consume_simulated_kernel_endpoint_bytes(&acpi),
            Err(KernelFanEndpointError::Contract(
                KernelFanReadContractError::Status(KernelFanReadStatus::AcpiResponseError)
            ))
        );
    }

    #[test]
    fn rejects_a_fat2_with_an_invalid_declared_table_length() {
        let mut bytes = endpoint_fixture(FAN_KERNEL_READ_CONTRACT_VERSION, 0, 88);
        bytes[56..60].copy_from_slice(&9_u32.to_le_bytes());
        assert!(matches!(
            consume_simulated_kernel_endpoint_bytes(&bytes),
            Err(KernelFanEndpointError::Contract(
                KernelFanReadContractError::Abi(_)
            ))
        ));
    }

    #[test]
    fn rejects_truncated_responses() {
        assert_eq!(
            consume_simulated_kernel_endpoint_bytes(&[0; 99]),
            Err(KernelFanEndpointError::UnexpectedWireLength { actual: 99 })
        );
    }

    #[test]
    fn rejects_an_incompatible_version() {
        let bytes = endpoint_fixture(2, 0, 88);
        assert_eq!(
            consume_simulated_kernel_endpoint_bytes(&bytes),
            Err(KernelFanEndpointError::Contract(
                KernelFanReadContractError::UnsupportedVersion { actual: 2 }
            ))
        );
    }
}
