//! Platform-neutral WMI request and response contracts.
//!
//! These types describe data in memory only. They do not locate a WMI device,
//! open a host path, invoke firmware, or define a binary WMI ABI.

use serde::Serialize;
use std::fmt;

/// How much is known about a field or an interface detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WmiKnowledge {
    Observed,
    Hypothesis,
    Unknown,
}

/// Platform where an observation was made. It is not a compatibility claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WmiObservationPlatform {
    Windows,
    Linux,
    NotApplicable,
}

/// Describes where a simulated artifact originated and what it does not prove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WmiArtifact {
    /// The sole artifact class available during Phase 3.
    StaticSimulation,
}

/// Describes where a simulated artifact originated and what it does not prove.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WmiProvenance {
    /// This phase cannot label an artifact as a response from the host.
    pub artifact: WmiArtifact,
    pub observed_on: WmiObservationPlatform,
    pub source: String,
    pub bios_version: Option<String>,
    pub knowledge: WmiKnowledge,
}

/// A human-readable operation label, not a numeric or binary method identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "operation")]
pub enum WmiOperation {
    /// The name was observed in Windows. Its Linux method identifier and ABI
    /// remain unknown.
    FanGetTable,
    /// A read-only label used to prove that unsupported operations are refused.
    OtherReadOnly { label: String },
}

impl WmiOperation {
    pub fn label(&self) -> &str {
        match self {
            Self::FanGetTable => "Fan_Get_Table",
            Self::OtherReadOnly { label } => label,
        }
    }
}

/// This phase exposes no writable operation variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WmiAccess {
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WmiArgument {
    pub name: String,
    pub value: u8,
    pub knowledge: WmiKnowledge,
}

/// Metadata explicitly preserving unverified ABI details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WmiMethodMetadata {
    pub observed_name: Option<String>,
    /// Opaque because the identifier's representation has not been verified.
    pub method_identifier: Option<String>,
    pub request_abi: WmiKnowledge,
    pub response_abi: WmiKnowledge,
}

/// An in-memory request. Constructing one cannot transmit it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WmiRequest {
    pub operation: WmiOperation,
    pub access: WmiAccess,
    pub arguments: Vec<WmiArgument>,
    pub method: WmiMethodMetadata,
    pub provenance: WmiProvenance,
}

/// Opaque raw bytes whose layout has deliberately not been interpreted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WmiResponse {
    pub reported_buffer_size: usize,
    pub bytes: Vec<u8>,
    pub provenance: WmiProvenance,
}

impl WmiResponse {
    /// Validates only the generic byte count, not any undocumented ABI layout.
    pub fn validate_buffer_size(&self) -> Result<(), WmiTransportError> {
        let actual_buffer_size = self.bytes.len();
        if self.reported_buffer_size == actual_buffer_size {
            Ok(())
        } else {
            Err(WmiTransportError::InconsistentBufferSize {
                reported_buffer_size: self.reported_buffer_size,
                actual_buffer_size,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "error")]
pub enum WmiTransportError {
    UnsupportedOperation {
        operation: String,
    },
    MethodUnavailable {
        operation: String,
        reason: String,
    },
    InconsistentBufferSize {
        reported_buffer_size: usize,
        actual_buffer_size: usize,
    },
}

impl fmt::Display for WmiTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedOperation { operation } => {
                write!(f, "unsupported simulated WMI operation: {operation}")
            }
            Self::MethodUnavailable { operation, reason } => {
                write!(f, "simulated WMI method unavailable: {operation}: {reason}")
            }
            Self::InconsistentBufferSize {
                reported_buffer_size,
                actual_buffer_size,
            } => write!(
                f,
                "inconsistent simulated WMI buffer size: reported {reported_buffer_size}, actual {actual_buffer_size}"
            ),
        }
    }
}

impl std::error::Error for WmiTransportError {}

/// A transport boundary for future adapters. `legion-core` supplies no real
/// implementation, and the only implementation in this phase lives in fixtures.
pub trait WmiTransport {
    fn execute(&self, request: &WmiRequest) -> Result<WmiResponse, WmiTransportError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance() -> WmiProvenance {
        WmiProvenance {
            artifact: WmiArtifact::StaticSimulation,
            observed_on: WmiObservationPlatform::Windows,
            source: "memory".to_owned(),
            bios_version: Some("NUCN15WW".to_owned()),
            knowledge: WmiKnowledge::Observed,
        }
    }

    #[test]
    fn accepts_a_consistent_opaque_buffer_without_decoding_it() {
        let response = WmiResponse {
            reported_buffer_size: 2,
            bytes: vec![0x12, 0x34],
            provenance: provenance(),
        };
        assert_eq!(response.validate_buffer_size(), Ok(()));
    }

    #[test]
    fn rejects_an_inconsistent_opaque_buffer() {
        let response = WmiResponse {
            reported_buffer_size: 3,
            bytes: vec![0x12, 0x34],
            provenance: provenance(),
        };
        assert!(matches!(
            response.validate_buffer_size(),
            Err(WmiTransportError::InconsistentBufferSize {
                reported_buffer_size: 3,
                actual_buffer_size: 2,
            })
        ));
    }

    #[test]
    fn write_operations_are_not_representable_in_this_phase() {
        assert_eq!(WmiAccess::ReadOnly, WmiAccess::ReadOnly);
    }
}
