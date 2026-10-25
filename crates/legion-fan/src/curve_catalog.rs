//! Offline model of the AML fan-curve catalogues observed on the 83EX.
//!
//! This parser consumes saved AML *text* supplied by its caller.  It neither
//! reads ACPI tables nor evaluates firmware.  `FAT2` values are configuration
//! indices here; they are not temperatures or RPM measurements.

use crate::{DecodedFanGetTableResponse, FAN_GET_TABLE_TABLE_ENTRY_COUNT};
use serde::Serialize;
use std::fmt;

pub const FNT_CATALOGUE_COUNT: usize = 5;
pub const FNT_PROFILE_COUNT: usize = 15;
pub const FNT_PROFILE_VALUE_COUNT: usize = 35;
pub const FI_CATALOGUE_COUNT: usize = 5;
pub const FI_PROFILE_COUNT: usize = 3;
pub const FI_PROFILE_VALUE_COUNT: usize = 22;
pub const MINIMUM_CURVE_CONFIGURATION_INDEX: u32 = 1;
pub const MAXIMUM_CURVE_CONFIGURATION_INDEX: u32 = 10;
pub const AML_CURVE_VALUE_OFFSET: usize = 2;
pub const AML_CURVE_SCALE_DIVISOR: u32 = 100;

/// A literal retained exactly as it appeared in disassembled AML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AmlCatalogueValue {
    pub original: String,
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AmlFanCurveCatalogues {
    pub fnt: Vec<Vec<Vec<AmlCatalogueValue>>>,
    pub fi: Vec<Vec<Vec<AmlCatalogueValue>>>,
}

/// Selection is intentionally caller-provided: GFAN/FAT2 does not identify
/// which of the fifteen FNT packages SFAN would choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FntProfileSelection {
    pub catalogue_index: usize,
    pub profile_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndexedCurveValue {
    pub configuration_index: u32,
    pub fnt_value_offset: usize,
    pub original_catalogue_value: AmlCatalogueValue,
    pub mathematical_scale_divisor: u32,
    pub physical_interpretation: PhysicalInterpretation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhysicalInterpretation {
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InterpretedFanGetTableIndices {
    pub selection: FntProfileSelection,
    pub fan_table: [IndexedCurveValue; FAN_GET_TABLE_TABLE_ENTRY_COUNT],
    pub sensor_table: [IndexedCurveValue; FAN_GET_TABLE_TABLE_ENTRY_COUNT],
}

/// Parses exactly the saved AML `Name (FNT0..FNT4, Package(...))` and
/// `Name (FI00..FI04, Package(...))` constructs needed by this offline model.
pub fn parse_aml_fan_curve_catalogues(
    source: &str,
) -> Result<AmlFanCurveCatalogues, AmlCatalogueError> {
    let fnt = (0..FNT_CATALOGUE_COUNT)
        .map(|index| {
            parse_catalogue(
                source,
                &format!("FNT{index}"),
                FNT_PROFILE_COUNT,
                FNT_PROFILE_VALUE_COUNT,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let fi = (0..FI_CATALOGUE_COUNT)
        .map(|index| {
            parse_catalogue(
                source,
                &format!("FI0{index}"),
                FI_PROFILE_COUNT,
                FI_PROFILE_VALUE_COUNT,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AmlFanCurveCatalogues { fnt, fi })
}

pub fn interpret_fat2_curve_indices(
    response: &DecodedFanGetTableResponse,
    catalogues: &AmlFanCurveCatalogues,
    selection: FntProfileSelection,
) -> Result<InterpretedFanGetTableIndices, AmlCatalogueError> {
    let profile = catalogues
        .fnt
        .get(selection.catalogue_index)
        .and_then(|catalogue| catalogue.get(selection.profile_index))
        .ok_or(AmlCatalogueError::UnknownFntSelection { selection })?;
    let map = |index| map_configuration_index(profile, index);
    Ok(InterpretedFanGetTableIndices {
        selection,
        fan_table: response
            .fan_table
            .map(map)
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .expect("fixed FAT2 length"),
        sensor_table: response
            .sensor_table
            .map(map)
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .expect("fixed FAT2 length"),
    })
}

pub(crate) fn map_configuration_index(
    profile: &[AmlCatalogueValue],
    index: u32,
) -> Result<IndexedCurveValue, AmlCatalogueError> {
    if !(MINIMUM_CURVE_CONFIGURATION_INDEX..=MAXIMUM_CURVE_CONFIGURATION_INDEX).contains(&index) {
        return Err(AmlCatalogueError::UnsafeConfigurationIndex { index });
    }
    let fnt_value_offset = index as usize + AML_CURVE_VALUE_OFFSET;
    let original_catalogue_value = profile
        .get(fnt_value_offset)
        .cloned()
        .ok_or(AmlCatalogueError::MissingCurveOffset { fnt_value_offset })?;
    Ok(IndexedCurveValue {
        configuration_index: index,
        fnt_value_offset,
        original_catalogue_value,
        mathematical_scale_divisor: AML_CURVE_SCALE_DIVISOR,
        physical_interpretation: PhysicalInterpretation::Unknown,
    })
}

fn parse_catalogue(
    source: &str,
    name: &str,
    expected_packages: usize,
    expected_values: usize,
) -> Result<Vec<Vec<AmlCatalogueValue>>, AmlCatalogueError> {
    let marker = format!("Name ({name}, Package");
    let start = source
        .find(&marker)
        .ok_or_else(|| AmlCatalogueError::MissingCatalogue {
            name: name.to_owned(),
        })?;
    let after_marker = &source[start + marker.len()..];
    let open = after_marker
        .find('{')
        .ok_or_else(|| AmlCatalogueError::MalformedCatalogue {
            name: name.to_owned(),
        })?;
    let (body, _) =
        brace_body(&after_marker[open..]).ok_or_else(|| AmlCatalogueError::MalformedCatalogue {
            name: name.to_owned(),
        })?;
    let mut packages = Vec::new();
    let mut remainder = body;
    while let Some(package_at) = remainder.find("Package") {
        let after = &remainder[package_at + "Package".len()..];
        let Some(open_at) = after.find('{') else {
            return Err(AmlCatalogueError::MalformedCatalogue {
                name: name.to_owned(),
            });
        };
        let (package_body, consumed) =
            brace_body(&after[open_at..]).ok_or_else(|| AmlCatalogueError::MalformedCatalogue {
                name: name.to_owned(),
            })?;
        packages.push(parse_literals(package_body, name)?);
        remainder = &after[open_at + consumed..];
    }
    if packages.len() != expected_packages
        || packages
            .iter()
            .any(|values| values.len() != expected_values)
    {
        return Err(AmlCatalogueError::UnexpectedCatalogueShape {
            name: name.to_owned(),
            packages: packages.len(),
            expected_packages,
            expected_values,
        });
    }
    Ok(packages)
}

/// Returns content within the first balanced brace pair and bytes consumed.
fn brace_body(input: &str) -> Option<(&str, usize)> {
    if !input.starts_with('{') {
        return None;
    }
    let mut depth = 0usize;
    for (position, character) in input.char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&input[1..position], position + 1));
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_literals(
    body: &str,
    catalogue: &str,
) -> Result<Vec<AmlCatalogueValue>, AmlCatalogueError> {
    let without_comments = body
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<String>();
    without_comments
        .split(',')
        .map(|raw| {
            let original = raw.trim();
            if original.is_empty() {
                return Err(AmlCatalogueError::InvalidLiteral {
                    catalogue: catalogue.to_owned(),
                    literal: original.to_owned(),
                });
            }
            let value = match original {
                "Zero" => 0,
                "One" => 1,
                _ => original
                    .strip_prefix("0x")
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .or_else(|| original.parse().ok())
                    .ok_or_else(|| AmlCatalogueError::InvalidLiteral {
                        catalogue: catalogue.to_owned(),
                        literal: original.to_owned(),
                    })?,
            };
            Ok(AmlCatalogueValue {
                original: original.to_owned(),
                value,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmlCatalogueError {
    MissingCatalogue {
        name: String,
    },
    MalformedCatalogue {
        name: String,
    },
    UnexpectedCatalogueShape {
        name: String,
        packages: usize,
        expected_packages: usize,
        expected_values: usize,
    },
    InvalidLiteral {
        catalogue: String,
        literal: String,
    },
    UnknownFntSelection {
        selection: FntProfileSelection,
    },
    UnsafeConfigurationIndex {
        index: u32,
    },
    MissingCurveOffset {
        fnt_value_offset: usize,
    },
}

impl fmt::Display for AmlCatalogueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCatalogue { name } => write!(f, "missing AML catalogue {name}"),
            Self::MalformedCatalogue { name } => write!(f, "malformed AML catalogue {name}"),
            Self::UnexpectedCatalogueShape {
                name,
                packages,
                expected_packages,
                expected_values,
            } => write!(
                f,
                "{name} has {packages} packages; expected {expected_packages} packages of {expected_values} values"
            ),
            Self::InvalidLiteral { catalogue, literal } => {
                write!(f, "invalid literal {literal:?} in {catalogue}")
            }
            Self::UnknownFntSelection { selection } => write!(
                f,
                "unknown FNT selection {}:{}",
                selection.catalogue_index, selection.profile_index
            ),
            Self::UnsafeConfigurationIndex { index } => write!(
                f,
                "configuration index {index} is outside the demonstrated 1..=10 range"
            ),
            Self::MissingCurveOffset { fnt_value_offset } => {
                write!(f, "FNT profile has no value at offset {fnt_value_offset}")
            }
        }
    }
}
impl std::error::Error for AmlCatalogueError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode_confirmed_83ex_fan_get_table_response;

    fn catalogue_source() -> String {
        let catalogue = |name: String, packages: usize, values: usize| {
            let package = (0..values)
                .map(|index| {
                    if index == 12 {
                        "0x1388".to_owned()
                    } else if index == 0 {
                        "Zero".to_owned()
                    } else {
                        index.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Name ({name}, Package (0x{packages:X}) {{ {} }})",
                (0..packages)
                    .map(|_| format!("Package (0x{values:X}) {{ {package} }}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let mut source = String::new();
        for index in 0..FNT_CATALOGUE_COUNT {
            source.push_str(&catalogue(
                format!("FNT{index}"),
                FNT_PROFILE_COUNT,
                FNT_PROFILE_VALUE_COUNT,
            ));
        }
        for index in 0..FI_CATALOGUE_COUNT {
            source.push_str(&catalogue(
                format!("FI0{index}"),
                FI_PROFILE_COUNT,
                FI_PROFILE_VALUE_COUNT,
            ));
        }
        source
    }
    fn all_tens() -> DecodedFanGetTableResponse {
        let mut bytes = [0_u8; 88];
        bytes[0..4].copy_from_slice(&10_u32.to_le_bytes());
        bytes[44..48].copy_from_slice(&10_u32.to_le_bytes());
        for offset in (4..44).step_by(4).chain((48..88).step_by(4)) {
            bytes[offset..offset + 4].copy_from_slice(&10_u32.to_le_bytes());
        }
        decode_confirmed_83ex_fan_get_table_response(&bytes).unwrap()
    }
    #[test]
    fn parses_shapes_and_preserves_aml_literals() {
        let catalogues = parse_aml_fan_curve_catalogues(&catalogue_source()).unwrap();
        assert_eq!(catalogues.fnt.len(), 5);
        assert_eq!(catalogues.fnt[0].len(), 15);
        assert_eq!(catalogues.fnt[0][0][12].original, "0x1388");
        assert_eq!(catalogues.fi[4].len(), 3);
    }
    #[test]
    fn interprets_ten_as_index_and_not_a_physical_measurement() {
        let interpreted = interpret_fat2_curve_indices(
            &all_tens(),
            &parse_aml_fan_curve_catalogues(&catalogue_source()).unwrap(),
            FntProfileSelection {
                catalogue_index: 0,
                profile_index: 0,
            },
        )
        .unwrap();
        let value = &interpreted.fan_table[0];
        assert_eq!(value.configuration_index, 10);
        assert_eq!(value.fnt_value_offset, 12);
        assert_eq!(value.original_catalogue_value.original, "0x1388");
        assert_eq!(value.mathematical_scale_divisor, 100);
        assert_eq!(
            value.physical_interpretation,
            PhysicalInterpretation::Unknown
        );
    }
    #[test]
    fn rejects_indices_that_would_be_unsafe_in_aml() {
        let mut response = all_tens();
        response.fan_table[0] = 0;
        assert_eq!(
            interpret_fat2_curve_indices(
                &response,
                &parse_aml_fan_curve_catalogues(&catalogue_source()).unwrap(),
                FntProfileSelection {
                    catalogue_index: 0,
                    profile_index: 0
                }
            ),
            Err(AmlCatalogueError::UnsafeConfigurationIndex { index: 0 })
        );
    }
}
