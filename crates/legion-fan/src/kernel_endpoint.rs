//! Decoder for already-acquired bytes of the proposed binary fan kernel endpoint.
//!
//! The decoder has no filesystem or device APIs. Acquisition provenance is
//! deliberately kept outside this module: callers must not turn parsed bytes
//! into a claim about the source that produced them.

use crate::{
    FAN_GET_TABLE_RESPONSE_SIZE_BYTES, FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES,
    KernelFanReadContractError, KernelFanReadStatus, SimulatedKernelFanReadResponse,
    SimulatedKernelFanReadResult, consume_simulated_kernel_fan_read,
};
use legion_core::TelemetryArtifact;
use std::fmt;

pub const FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES: usize =
    FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES as usize + FAN_GET_TABLE_RESPONSE_SIZE_BYTES;

/// A validated, lossless version-one endpoint response. This describes bytes
/// already obtained by a caller; it does not describe their provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedKernelFanEndpointResponse {
    pub original: [u8; FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES],
    pub version: u16,
    pub header_size: u16,
    pub status: KernelFanReadStatus,
    pub payload_size: usize,
    pub fat2: crate::DecodedFanGetTableResponse,
}

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

/// Decodes one fully acquired binary endpoint response. Truncated data,
/// unknown statuses, and contract or FAT2 errors never produce a successful
/// result. This function performs no acquisition and assigns no provenance.
pub fn decode_kernel_endpoint_bytes(
    bytes: &[u8],
) -> Result<DecodedKernelFanEndpointResponse, KernelFanEndpointError> {
    if bytes.len() != FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES {
        return Err(KernelFanEndpointError::UnexpectedWireLength {
            actual: bytes.len(),
        });
    }
    let original: [u8; FAN_KERNEL_ENDPOINT_RESPONSE_SIZE_BYTES] = bytes
        .try_into()
        .expect("wire length was checked against the fixed contract size");
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

    if version != crate::FAN_KERNEL_READ_CONTRACT_VERSION {
        return Err(KernelFanEndpointError::Contract(
            KernelFanReadContractError::UnsupportedVersion { actual: version },
        ));
    }
    if header_size != FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES {
        return Err(KernelFanEndpointError::Contract(
            KernelFanReadContractError::UnexpectedHeaderSize {
                actual: header_size,
            },
        ));
    }
    if status != KernelFanReadStatus::Ok {
        return Err(KernelFanEndpointError::Contract(
            KernelFanReadContractError::Status(status),
        ));
    }
    let payload = &original[FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES as usize
        ..FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES as usize + payload_size];
    let fat2 = crate::decode_confirmed_83ex_fan_get_table_response(payload)
        .map_err(KernelFanReadContractError::Abi)
        .map_err(KernelFanEndpointError::Contract)?;
    Ok(DecodedKernelFanEndpointResponse {
        original,
        version,
        header_size,
        status,
        payload_size,
        fat2,
    })
}

/// Decodes one fully acquired fixture and attaches its fixed simulation
/// provenance. Real or offline acquisition belongs in a different adapter.
pub fn consume_simulated_kernel_endpoint_bytes(
    bytes: &[u8],
) -> Result<SimulatedKernelFanReadResult, KernelFanEndpointError> {
    let decoded = decode_kernel_endpoint_bytes(bytes)?;
    let response = SimulatedKernelFanReadResponse {
        version: decoded.version,
        header_size: decoded.header_size,
        status: decoded.status,
        payload_size: decoded.payload_size,
        payload: decoded.fat2.original.to_vec(),
        provenance: TelemetryArtifact::StaticSimulation,
    };
    // Keep the existing simulation-only contract boundary as a regression
    // check. It should be equivalent to the provenance-neutral decoder above.
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
    fn neutral_decoder_keeps_the_complete_original_wire_response() {
        let bytes = endpoint_fixture(FAN_KERNEL_READ_CONTRACT_VERSION, 0, 88);
        let decoded = decode_kernel_endpoint_bytes(&bytes).unwrap();
        assert_eq!(decoded.original.as_slice(), bytes);
        assert_eq!(decoded.fat2.fan_table_size, 10);
        assert_eq!(decoded.fat2.sensor_table_size, 10);
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
