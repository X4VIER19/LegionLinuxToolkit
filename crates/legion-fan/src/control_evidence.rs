//! Offline inspection of SFAN input evidence from 83EX/NUCN15WW.
//! No encoder, transport, authorization, or executable write operation exists.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SfanInputEvidence {
    original: Vec<u8>,
    pub selector: u8,
    pub first_metadata_byte: u8,
    pub first_declared_count: u32,
    pub configuration_indices: [u16; 10],
    pub middle_metadata_byte: u8,
    pub second_declared_count: u32,
    pub unused_words: [u16; 10],
    pub final_metadata_byte: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SfanEvidenceError {
    Size { actual: usize },
    UnknownSelector(u8),
    IndexOutsideConservativeDomain { point: usize, value: u16 },
}

impl SfanInputEvidence {
    /// Accepts the exact field span observed in AML. Firmware has no explicit
    /// exact-size check; rejecting trailing bytes is our offline policy.
    pub fn inspect(bytes: &[u8]) -> Result<Self, SfanEvidenceError> {
        if bytes.len() != 52 {
            return Err(SfanEvidenceError::Size {
                actual: bytes.len(),
            });
        }
        let words = |offset| {
            std::array::from_fn(|i| {
                u16::from_le_bytes([bytes[offset + i * 2], bytes[offset + i * 2 + 1]])
            })
        };
        Ok(Self {
            original: bytes.to_vec(),
            selector: bytes[0],
            first_metadata_byte: bytes[1],
            first_declared_count: u32::from_le_bytes(bytes[2..6].try_into().unwrap()),
            configuration_indices: words(6),
            middle_metadata_byte: bytes[26],
            second_declared_count: u32::from_le_bytes(bytes[27..31].try_into().unwrap()),
            unused_words: words(31),
            final_metadata_byte: bytes[51],
        })
    }

    pub fn original(&self) -> &[u8] {
        &self.original
    }

    /// Checks only the selector branches and conservative 1..10 index domain
    /// already modelled by the read-side catalogues. This does NOT certify
    /// thermal safety, metadata semantics, or permission to write.
    pub fn validate_index_domain(&self) -> Result<(), SfanEvidenceError> {
        if !matches!(self.selector, 1 | 2 | 3 | 0xff | 0xe0) {
            return Err(SfanEvidenceError::UnknownSelector(self.selector));
        }
        for (point, &value) in self.configuration_indices.iter().enumerate() {
            if !(crate::curve_catalog::MINIMUM_CURVE_CONFIGURATION_INDEX
                ..=crate::curve_catalog::MAXIMUM_CURVE_CONFIGURATION_INDEX)
                .contains(&u32::from(value))
            {
                return Err(SfanEvidenceError::IndexOutsideConservativeDomain { point, value });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(value: u16) -> Vec<u8> {
        let mut bytes = vec![0; 52];
        bytes[0] = 1;
        for point in 0..10 {
            bytes[6 + point * 2..8 + point * 2].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }
    #[test]
    fn preserves_unused_fields_and_both_index_boundaries() {
        for value in [1, 10] {
            let mut bytes = fixture(value);
            bytes[27..31].copy_from_slice(&u32::MAX.to_le_bytes());
            bytes[51] = 0xa5;
            let parsed = SfanInputEvidence::inspect(&bytes).unwrap();
            assert_eq!(parsed.original(), bytes);
            assert_eq!(parsed.second_declared_count, u32::MAX);
            assert_eq!(parsed.final_metadata_byte, 0xa5);
            assert!(parsed.validate_index_domain().is_ok());
        }
    }
    #[test]
    fn rejects_size_selector_and_out_of_domain_indices() {
        for length in [0, 51, 53] {
            assert!(SfanInputEvidence::inspect(&vec![0; length]).is_err());
        }
        for value in [0, 11, u16::MAX] {
            assert!(
                SfanInputEvidence::inspect(&fixture(value))
                    .unwrap()
                    .validate_index_domain()
                    .is_err()
            );
        }
        let mut bytes = fixture(1);
        bytes[0] = 4;
        assert_eq!(
            SfanInputEvidence::inspect(&bytes)
                .unwrap()
                .validate_index_domain(),
            Err(SfanEvidenceError::UnknownSelector(4))
        );
    }
}
