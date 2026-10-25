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

    /// Offline nominal-level preview from FNT0 package 9 (FanID=1,
    /// SensorID=4). The caller supplies saved catalogue evidence, never a
    /// hardware reader. Retains raw literals and the AML mathematical scale;
    /// does not convert them to measured RPM or assert current EC selection.
    pub fn preview_fnt0(
        &self,
        catalogues: &crate::AmlFanCurveCatalogues,
    ) -> Result<[crate::IndexedCurveValue; 10], crate::AmlCatalogueError> {
        let selection = crate::FntProfileSelection {
            catalogue_index: 0,
            profile_index: 9,
        };
        let profile = catalogues
            .fnt
            .first()
            .and_then(|c| c.get(9))
            .ok_or(crate::AmlCatalogueError::UnknownFntSelection { selection })?;
        self.indices
            .map(|index| crate::curve_catalog::map_configuration_index(profile, u32::from(index)))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| crate::AmlCatalogueError::UnknownFntSelection { selection })
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
    fn custom_preview_preserves_fnt0_literals_and_existing_serialization() {
        use crate::{AmlCatalogueValue, AmlFanCurveCatalogues, PhysicalInterpretation};
        let levels = [0, 1700, 1900, 2100, 2400, 2900, 3200, 3500, 3800, 5000];
        let mut profile = vec![
            AmlCatalogueValue {
                original: "Zero".into(),
                value: 0
            };
            35
        ];
        for (position, value) in levels.into_iter().enumerate() {
            profile[position + 3] = AmlCatalogueValue {
                original: format!("0x{value:X}"),
                value,
            };
        }
        let catalogues = AmlFanCurveCatalogues {
            fnt: vec![vec![profile; 15]; 5],
            fi: vec![],
        };
        let indices = [2, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let proposal = OfflineCurveProposal::new(indices, CurveRules::DeclaredLltV2ToV4).unwrap();
        let preview = proposal.preview_fnt0(&catalogues).unwrap();
        assert_eq!(
            preview.each_ref().map(|p| p.original_catalogue_value.value),
            [1700, 1700, 1900, 2100, 2400, 2900, 3200, 3500, 3800, 5000]
        );
        assert_eq!(preview[0].original_catalogue_value.original, "0x6A4");
        assert_eq!(preview[0].fnt_value_offset, 4);
        assert_eq!(preview[0].mathematical_scale_divisor, 100);
        assert_eq!(
            preview[0].physical_interpretation,
            PhysicalInterpretation::Unknown
        );
        let payload = proposal.payload();
        assert_eq!(payload.len(), 64);
        assert_eq!(&payload[..6], &[1, 0, 0, 0, 0, 0]);
        for (point, index) in indices.into_iter().enumerate() {
            assert_eq!(&payload[6 + point * 2..8 + point * 2], &index.to_le_bytes());
        }
        assert!(payload[26..].iter().all(|b| *b == 0));
        assert!(
            proposal
                .preview_fnt0(&AmlFanCurveCatalogues {
                    fnt: vec![],
                    fi: vec![]
                })
                .is_err()
        );
    }
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
