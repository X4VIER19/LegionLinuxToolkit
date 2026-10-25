//! Independent implementation of the 64-byte LLT contract reported for
//! commit 6f19ef48095a32afe439474a65e5b95cf8fa1b24. Pure byte generation:
//! no firmware invocation, transport handle, or authorization is represented.

use crate::control_evidence::{SfanEvidenceError, SfanInputEvidence};

pub const LLT_PAYLOAD_BYTES: usize = 64;
pub const DEFAULT_INDICES: [u16; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
pub const LLT_V2_V4_MINIMA: [u16; 10] = [1, 1, 1, 1, 1, 1, 1, 1, 3, 5];

/// Explicit choice of software validation rules; it does not identify the
/// variant on the current host or assert a firmware thermal constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveRules {
    Unidentified,
    DeclaredLltV2ToV4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlPayloadError {
    VariantUnidentified,
    Sfan(SfanEvidenceError),
    BelowSoftwareMinimum {
        point: usize,
        value: u16,
        minimum: u16,
    },
    Decreasing {
        point: usize,
    },
}

/// Validated software proposal, not permission to apply a hardware change.
/// No Deserialize or public fields can bypass its constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineCurveProposal {
    indices: [u16; 10],
}

impl OfflineCurveProposal {
    pub fn new(indices: [u16; 10], rules: CurveRules) -> Result<Self, ControlPayloadError> {
        if rules != CurveRules::DeclaredLltV2ToV4 {
            return Err(ControlPayloadError::VariantUnidentified);
        }
        // Reuse the AML span parser and index validator. All unconsumed fields
        // are zero in this LLT software contract; the full buffer is 64 bytes.
        let payload = serialize_indices(indices);
        SfanInputEvidence::inspect(&payload[..52])
            .map_err(ControlPayloadError::Sfan)?
            .validate_index_domain()
            .map_err(ControlPayloadError::Sfan)?;
        for (point, &value) in indices.iter().enumerate() {
            let minimum = LLT_V2_V4_MINIMA[point];
            if value < minimum {
                return Err(ControlPayloadError::BelowSoftwareMinimum {
                    point,
                    value,
                    minimum,
                });
            }
            if point > 0 && value < indices[point - 1] {
                return Err(ControlPayloadError::Decreasing { point });
            }
        }
        Ok(Self { indices })
    }

    pub fn default_curve(rules: CurveRules) -> Result<Self, ControlPayloadError> {
        Self::new(DEFAULT_INDICES, rules)
    }

    pub fn indices(&self) -> &[u16; 10] {
        &self.indices
    }

    pub fn payload(&self) -> [u8; LLT_PAYLOAD_BYTES] {
        serialize_indices(self.indices)
    }
}

fn serialize_indices(indices: [u16; 10]) -> [u8; LLT_PAYLOAD_BYTES] {
    let mut bytes = [0; LLT_PAYLOAD_BYTES];
    bytes[0] = 1;
    for (point, value) in indices.iter().enumerate() {
        bytes[6 + point * 2..8 + point * 2].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_matches_documented_windows_contract_byte_for_byte() {
        let expected = [
            1, 0, 0, 0, 0, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0, 9, 0, 10, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0,
        ];
        let proposal = OfflineCurveProposal::default_curve(CurveRules::DeclaredLltV2ToV4).unwrap();
        assert_eq!(proposal.payload(), expected);
        let sfan = SfanInputEvidence::inspect(&expected[..52]).unwrap();
        assert_eq!(sfan.configuration_indices, DEFAULT_INDICES);
        assert!(expected[26..].iter().all(|byte| *byte == 0));
    }
    #[test]
    fn requires_explicit_rules_and_checks_software_boundaries() {
        assert_eq!(
            OfflineCurveProposal::default_curve(CurveRules::Unidentified),
            Err(ControlPayloadError::VariantUnidentified)
        );
        for indices in [LLT_V2_V4_MINIMA, [10; 10]] {
            assert!(OfflineCurveProposal::new(indices, CurveRules::DeclaredLltV2ToV4).is_ok());
        }
        for indices in [[0; 10], [11; 10], [1; 10], [2, 1, 3, 4, 5, 6, 7, 8, 9, 10]] {
            assert!(OfflineCurveProposal::new(indices, CurveRules::DeclaredLltV2ToV4).is_err());
        }
    }
}
