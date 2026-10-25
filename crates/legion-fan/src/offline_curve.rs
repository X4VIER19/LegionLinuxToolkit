//! Shared offline LLT GodMode V2 curve document and FNT0 preview.
//!
//! This module contains no filesystem or firmware access. Consumers choose how
//! to persist its versioned JSON document; all curve rules and payload bytes
//! remain in `control_payload`.

use crate::{
    AmlCatalogueValue, AmlFanCurveCatalogues,
    control_payload::{CurveRules, OfflineCurveProposal},
};
use serde::{Deserialize, Serialize};

pub const CURVE_SCHEMA_VERSION: u32 = 1;
pub const CURVE_HARDWARE: &str = "83EX/NUCN15WW";
pub const CURVE_RULES: &str = "LLT-GodMode-V2";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OfflineCurveFile {
    schema_version: u32,
    hardware: String,
    rules: String,
    indices: [u16; 10],
}

impl OfflineCurveFile {
    pub fn new(indices: [u16; 10]) -> Self {
        Self {
            schema_version: CURVE_SCHEMA_VERSION,
            hardware: CURVE_HARDWARE.into(),
            rules: CURVE_RULES.into(),
            indices,
        }
    }

    pub fn indices(&self) -> &[u16; 10] {
        &self.indices
    }

    pub fn validate(&self) -> Result<OfflineCurveProposal, String> {
        if self.schema_version != CURVE_SCHEMA_VERSION
            || self.hardware != CURVE_HARDWARE
            || self.rules != CURVE_RULES
        {
            return Err("unsupported curve version, hardware or rules".into());
        }
        OfflineCurveProposal::new(self.indices, CurveRules::DeclaredLltV2ToV4)
            .map_err(|error| format!("invalid LLT V2 curve: {error:?}"))
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let file: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        file.validate()?;
        Ok(file)
    }

    pub fn to_pretty_json(&self) -> Result<Vec<u8>, String> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OfflineCurvePreviewPoint {
    pub point: usize,
    pub index: u16,
    pub nominal_rpm: u32,
    pub fnt0_sensor4: u32,
    pub fnt0_sensor5: u32,
    pub cpu_cl_ct_raw: [u32; 2],
    pub gpu_gl_gt_raw: [u32; 2],
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OfflineCurvePreview {
    pub source: String,
    pub points: Vec<OfflineCurvePreviewPoint>,
}

#[derive(Deserialize)]
struct Reference {
    source: String,
    fnt0: [Vec<u32>; 3],
    fi02: [[u32; 22]; 3],
}

pub fn preview_83ex_fnt0(proposal: &OfflineCurveProposal) -> Result<OfflineCurvePreview, String> {
    let reference: Reference = serde_json::from_str(include_str!("../data/83ex-fnt0-fi02.json"))
        .map_err(|error| error.to_string())?;
    if reference.fnt0.iter().any(|package| package.len() != 35) {
        return Err("embedded FNT0 package must retain all 35 fields".into());
    }
    // Only the preserved packages are represented: this is an offline FNT0
    // preview, not a current EC/catologue selection assertion.
    let mut fnt0 = vec![vec![]; 9];
    fnt0.extend(reference.fnt0.iter().map(|values| {
        values
            .iter()
            .map(|&value| AmlCatalogueValue {
                original: value.to_string(),
                value,
            })
            .collect()
    }));
    let catalogues = AmlFanCurveCatalogues {
        fnt: vec![fnt0],
        fi: vec![],
    };
    let nominal = proposal
        .preview_fnt0(&catalogues)
        .map_err(|error| error.to_string())?;
    let points = (0..10)
        .map(|index| OfflineCurvePreviewPoint {
            point: index + 1,
            index: proposal.indices()[index],
            nominal_rpm: nominal[index].original_catalogue_value.value,
            // Thresholds are attached to point position, never shifted by a
            // selected curve index.
            fnt0_sensor4: reference.fnt0[0][15 + index],
            fnt0_sensor5: reference.fnt0[1][15 + index],
            cpu_cl_ct_raw: [reference.fi02[0][index], reference.fi02[0][index + 10]],
            gpu_gl_gt_raw: [reference.fi02[1][index], reference.fi02[1][index + 10]],
        })
        .collect();
    Ok(OfflineCurvePreview {
        source: reference.source,
        points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_and_json_are_compatible_with_the_existing_cli_document() {
        let file = OfflineCurveFile::new([2, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let proposal = file.validate().unwrap();
        let preview = preview_83ex_fnt0(&proposal).unwrap();
        assert_eq!(preview.points[0].nominal_rpm, 1700);
        assert_eq!(preview.points[0].fnt0_sensor4, 48);
        assert_eq!(preview.points[0].cpu_cl_ct_raw, [0, 58]);
        assert_eq!(
            OfflineCurveFile::from_json(&file.to_pretty_json().unwrap()).unwrap(),
            file
        );
    }

    #[test]
    fn wrong_metadata_or_invalid_indices_are_rejected() {
        assert!(OfflineCurveFile::from_json(
            br#"{\"schema_version\":2,\"hardware\":\"83EX/NUCN15WW\",\"rules\":\"LLT-GodMode-V2\",\"indices\":[1,2,3,4,5,6,7,8,9,10]}"#
        ).is_err());
        assert!(OfflineCurveFile::new([1; 10]).validate().is_err());
    }
}
