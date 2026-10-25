//! Offline FACT layout from 83EX/NUCN15WW WQA7 -> SFTW. No firmware API.
use crate::AmlFanCurveCatalogues;
use serde::Serialize;

pub const FACT_CAPTURE_BYTES: usize = 316;
pub const FACT_INSTANCES: [u16; 4] = [6, 9, 10, 11];
// Widths of all 35 values, in SFTW assignment order, including opaque fields.
const WIDTHS: [usize; 35] = [
    2, 2, 4, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 4, 4, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 2, 1, 1, 2, 2,
    2, 2, 2,
];

#[derive(Debug, Clone, Serialize)]
pub struct FactRecord {
    pub instance: u16,
    pub original: Vec<u8>,
    /// SFTW values in original package order; opaque auxiliary fields retained.
    pub values: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct FactComparison {
    pub source: &'static str,
    pub original_capture: Vec<u8>,
    pub records: Vec<FactRecord>,
    pub matching_catalogues: Vec<usize>,
    pub uniquely_matches_fnt0: bool,
    pub historical_windows_levels_consistent: bool,
    pub conclusion: &'static str,
    pub authorizes_write: bool,
}

pub fn decode_fact_capture(bytes: &[u8]) -> Result<Vec<FactRecord>, String> {
    if bytes.len() != FACT_CAPTURE_BYTES {
        return Err("FACT capture must contain exactly 316 bytes".into());
    }
    if bytes[0..4] != [1, 0, 12, 0] {
        return Err("FACT contract version/header mismatch".into());
    }
    let status = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let size = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    if status != 0 || size != 304 {
        return Err(format!(
            "FACT acquisition failed/incomplete: status={status}, completed_payload_bytes={size}; retain original capture; no valid catalogue"
        ));
    }
    FACT_INSTANCES
        .iter()
        .enumerate()
        .map(|(index, &instance)| {
            let start = 12 + index * 76;
            if bytes[start..start + 2] != instance.to_le_bytes()
                || bytes[start + 2..start + 4] != [0, 0]
            {
                return Err("unexpected FACT instance/order/reserved field".into());
            }
            let original = bytes[start + 4..start + 76].to_vec();
            let mut offset = 0;
            let values: Vec<_> = WIDTHS
                .iter()
                .map(|&width| {
                    let mut value = [0; 4];
                    value[..width].copy_from_slice(&original[offset..offset + width]);
                    offset += width;
                    u32::from_le_bytes(value)
                })
                .collect();
            if values[2] != 10 || values[14] != 10 {
                return Err("FACT fan/sensor counts must both be ten".into());
            }
            Ok(FactRecord {
                instance,
                original,
                values,
            })
        })
        .collect()
}

pub fn compare_fact_capture(
    bytes: &[u8],
    catalogues: &AmlFanCurveCatalogues,
) -> Result<FactComparison, String> {
    let records = decode_fact_capture(bytes)?;
    let mut matching_catalogues = Vec::new();
    if catalogues.fnt.len() != 5 {
        return Err("expected the five offline DSDT catalogues".into());
    }
    for (index, catalogue) in catalogues.fnt.iter().enumerate() {
        let mut matches = true;
        for record in &records {
            let profile = catalogue
                .get(usize::from(record.instance))
                .ok_or("missing offline FNT package")?;
            if profile.len() != WIDTHS.len() {
                return Err("malformed offline FNT package".into());
            }
            let mut encoded = Vec::new();
            for (value, width) in profile.iter().zip(WIDTHS) {
                let raw = value.value.to_le_bytes();
                if raw[width..].iter().any(|&byte| byte != 0) {
                    return Err("offline FNT value exceeds its FACT field width".into());
                }
                encoded.extend_from_slice(&raw[..width]);
            }
            matches &= encoded == record.original;
        }
        if matches {
            matching_catalogues.push(index);
        }
    }
    let windows_levels = [0, 1700, 1900, 2100, 2400, 2900, 3200, 3500, 3800, 5000];
    let windows_consistent =
        records[1..]
            .iter()
            .zip([(1, 4), (2, 5), (1, 1)])
            .all(|(record, (fan, sensor))| {
                record.values[0] == 255
                    && record.values[1] == fan
                    && record.values[13] == sensor
                    && record.values[3..13] == windows_levels
            });
    Ok(FactComparison {
        source: "offline-capture-unverified; DSDT-reference; historical-Windows-comparison",
        original_capture: bytes.to_vec(),
        records,
        uniquely_matches_fnt0: matching_catalogues == [0],
        matching_catalogues,
        historical_windows_levels_consistent: windows_consistent,
        conclusion: "Matching FACT identifies published SFTW output during four non-atomic queries, not direct PMID/PGID/DCID values, current EC configuration, or a future SFAN selection.",
        authorizes_write: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AmlCatalogueValue;

    fn fixture() -> (Vec<u8>, AmlFanCurveCatalogues) {
        let mut bytes = vec![0; FACT_CAPTURE_BYTES];
        bytes[..4].copy_from_slice(&[1, 0, 12, 0]);
        bytes[8..12].copy_from_slice(&304_u32.to_le_bytes());
        let mut fnt = vec![
            vec![
                vec![
                    AmlCatalogueValue {
                        original: "0".into(),
                        value: 0
                    };
                    35
                ];
                15
            ];
            5
        ];
        for (index, instance) in FACT_INSTANCES.into_iter().enumerate() {
            let profile = &mut fnt[0][usize::from(instance)];
            let coordinates = [(1, 1, 4), (255, 1, 4), (255, 2, 5), (255, 1, 1)][index];
            for (position, value) in [
                (0, coordinates.0),
                (1, coordinates.1),
                (2, 10),
                (13, coordinates.2),
                (14, 10),
            ] {
                profile[position].value = value;
            }
            for (position, value) in [0, 1700, 1900, 2100, 2400, 2900, 3200, 3500, 3800, 5000]
                .into_iter()
                .enumerate()
            {
                profile[position + 3].value = value;
            }
            let start = 12 + index * 76;
            bytes[start..start + 2].copy_from_slice(&instance.to_le_bytes());
            let mut offset = start + 4;
            for (value, width) in profile.iter().zip(WIDTHS) {
                bytes[offset..offset + width].copy_from_slice(&value.value.to_le_bytes()[..width]);
                offset += width;
            }
        }
        (bytes, AmlFanCurveCatalogues { fnt, fi: vec![] })
    }

    #[test]
    fn compares_all_bytes_and_windows_without_authorizing_a_write() {
        let (bytes, catalogues) = fixture();
        let result = compare_fact_capture(&bytes, &catalogues).unwrap();
        assert!(result.uniquely_matches_fnt0);
        assert!(result.historical_windows_levels_consistent);
        assert_eq!(result.original_capture, bytes);
        assert!(!result.authorizes_write);
        let mut different = bytes.clone();
        different[315] = 1; // Opaque auxiliary byte must also participate.
        assert!(
            compare_fact_capture(&different, &catalogues)
                .unwrap()
                .matching_catalogues
                .is_empty()
        );
    }

    #[test]
    fn rejects_partial_errors_layout_lengths_and_counts() {
        let (bytes, _) = fixture();
        for (offset, value) in [
            (0, 2),
            (2, 0),
            (4, 2),
            (8, 0),
            (12, 7),
            (14, 1),
            (20, 9),
            (48, 9),
        ] {
            let mut invalid = bytes.clone();
            invalid[offset] = value;
            assert!(decode_fact_capture(&invalid).is_err(), "offset {offset}");
        }
        assert!(decode_fact_capture(&bytes[..315]).is_err());
        assert!(decode_fact_capture(&[bytes.as_slice(), &[0]].concat()).is_err());
    }

    #[test]
    fn ambiguous_catalogues_and_windows_mismatch_remain_explicit() {
        let (mut bytes, mut catalogues) = fixture();
        catalogues.fnt[1] = catalogues.fnt[0].clone();
        let result = compare_fact_capture(&bytes, &catalogues).unwrap();
        assert_eq!(result.matching_catalogues, [0, 1]);
        assert!(!result.uniquely_matches_fnt0);
        bytes[12 + 76 + 4] = 1; // Not historical Custom mode.
        assert!(
            !compare_fact_capture(&bytes, &catalogues)
                .unwrap()
                .historical_windows_levels_consistent
        );
    }
}
