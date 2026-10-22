//! In-memory model of the proposed read-only kernel/userspace fan contract.
//!
//! The corresponding C header is an unconnected layout specification. This
//! Rust module consumes injected static simulations only; it neither locates a
//! kernel endpoint nor constructs a real hardware provenance.

use crate::{
    DecodedFanGetTableResponse, FAN_GET_TABLE_RESPONSE_SIZE_BYTES, FanGetTableAbiError,
    decode_confirmed_83ex_fan_get_table_response,
};
use legion_core::TelemetryArtifact;
use std::fmt;

pub const FAN_KERNEL_READ_CONTRACT_VERSION: u16 = 1;
pub const FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES: u16 = 12;

/// Classification mirrored by `legion_fan_read_contract.h`; it is not a
/// physical fan state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum KernelFanReadStatus {
    Ok = 0,
    Unsupported = 1,
    TransportError = 2,
    AcpiResponseError = 3,
    AbiError = 4,
}

/// An injected version-one response. `payload` is opaque unless status is OK
/// and the ABI decoder separately accepts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedKernelFanReadResponse {
    pub version: u16,
    pub header_size: u16,
    pub status: KernelFanReadStatus,
    pub payload_size: usize,
    pub payload: Vec<u8>,
    pub provenance: TelemetryArtifact,
}

impl SimulatedKernelFanReadResponse {
    pub fn static_fat2(payload: [u8; FAN_GET_TABLE_RESPONSE_SIZE_BYTES]) -> Self {
        Self {
            version: FAN_KERNEL_READ_CONTRACT_VERSION,
            header_size: FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES,
            status: KernelFanReadStatus::Ok,
            payload_size: payload.len(),
            payload: payload.to_vec(),
            provenance: TelemetryArtifact::StaticSimulation,
        }
    }
}

/// Successful contract consumption keeps the original response and the
/// lossless FAT2 words; their physical meanings remain unspecified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedKernelFanReadResult {
    pub response: SimulatedKernelFanReadResponse,
    pub decoded: DecodedFanGetTableResponse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelFanReadContractError {
    NonSimulatedProvenance,
    UnsupportedVersion { actual: u16 },
    UnexpectedHeaderSize { actual: u16 },
    Status(KernelFanReadStatus),
    InconsistentPayloadSize { declared: usize, actual: usize },
    Abi(FanGetTableAbiError),
}

impl fmt::Display for KernelFanReadContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonSimulatedProvenance => {
                write!(
                    f,
                    "kernel fan contract adapter accepts static simulations only"
                )
            }
            Self::UnsupportedVersion { actual } => write!(
                f,
                "fan kernel contract version {actual} is unsupported; expected {FAN_KERNEL_READ_CONTRACT_VERSION}"
            ),
            Self::UnexpectedHeaderSize { actual } => write!(
                f,
                "fan kernel contract header has {actual} bytes; expected {FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES}"
            ),
            Self::Status(status) => write!(f, "fan kernel contract returned {status:?}"),
            Self::InconsistentPayloadSize { declared, actual } => write!(
                f,
                "fan kernel contract declares {declared} payload bytes but contains {actual}"
            ),
            Self::Abi(error) => write!(f, "fan kernel contract FAT2 error: {error}"),
        }
    }
}

impl std::error::Error for KernelFanReadContractError {}

/// Consumes one already-acquired, injected response. No acquisition or retry
/// operation exists in this adapter.
pub fn consume_simulated_kernel_fan_read(
    response: SimulatedKernelFanReadResponse,
) -> Result<SimulatedKernelFanReadResult, KernelFanReadContractError> {
    if response.provenance != TelemetryArtifact::StaticSimulation {
        return Err(KernelFanReadContractError::NonSimulatedProvenance);
    }
    if response.version != FAN_KERNEL_READ_CONTRACT_VERSION {
        return Err(KernelFanReadContractError::UnsupportedVersion {
            actual: response.version,
        });
    }
    if response.header_size != FAN_KERNEL_READ_CONTRACT_HEADER_SIZE_BYTES {
        return Err(KernelFanReadContractError::UnexpectedHeaderSize {
            actual: response.header_size,
        });
    }
    if response.status != KernelFanReadStatus::Ok {
        return Err(KernelFanReadContractError::Status(response.status));
    }
    if response.payload_size != response.payload.len() {
        return Err(KernelFanReadContractError::InconsistentPayloadSize {
            declared: response.payload_size,
            actual: response.payload.len(),
        });
    }
    let decoded = decode_confirmed_83ex_fan_get_table_response(&response.payload)
        .map_err(KernelFanReadContractError::Abi)?;
    Ok(SimulatedKernelFanReadResult { response, decoded })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fat2() -> [u8; FAN_GET_TABLE_RESPONSE_SIZE_BYTES] {
        let mut bytes = [0; FAN_GET_TABLE_RESPONSE_SIZE_BYTES];
        bytes[0..4].copy_from_slice(&10_u32.to_le_bytes());
        bytes[44..48].copy_from_slice(&10_u32.to_le_bytes());
        bytes
    }

    #[test]
    fn consumes_a_valid_static_fat2_without_assigning_physical_meaning() {
        let response = SimulatedKernelFanReadResponse::static_fat2(fat2());
        let result = consume_simulated_kernel_fan_read(response.clone()).unwrap();
        assert_eq!(result.response, response);
        assert_eq!(result.decoded.original, fat2());
    }

    #[test]
    fn rejects_non_success_status_before_decoding_payload() {
        let mut response = SimulatedKernelFanReadResponse::static_fat2(fat2());
        response.status = KernelFanReadStatus::Unsupported;
        assert_eq!(
            consume_simulated_kernel_fan_read(response),
            Err(KernelFanReadContractError::Status(
                KernelFanReadStatus::Unsupported
            ))
        );
    }

    #[test]
    fn distinguishes_contract_and_payload_errors() {
        let mut version = SimulatedKernelFanReadResponse::static_fat2(fat2());
        version.version = 2;
        assert_eq!(
            consume_simulated_kernel_fan_read(version),
            Err(KernelFanReadContractError::UnsupportedVersion { actual: 2 })
        );

        let mut size = SimulatedKernelFanReadResponse::static_fat2(fat2());
        size.payload_size = 87;
        assert_eq!(
            consume_simulated_kernel_fan_read(size),
            Err(KernelFanReadContractError::InconsistentPayloadSize {
                declared: 87,
                actual: 88,
            })
        );
    }

    #[test]
    fn rejects_a_non_simulated_provenance() {
        let mut response = SimulatedKernelFanReadResponse::static_fat2(fat2());
        response.provenance = TelemetryArtifact::LinuxReadOnly;
        assert_eq!(
            consume_simulated_kernel_fan_read(response),
            Err(KernelFanReadContractError::NonSimulatedProvenance)
        );
    }
}
