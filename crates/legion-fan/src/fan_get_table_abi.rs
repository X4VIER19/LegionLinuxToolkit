//! Static AML-backed codec for the 83EX `Fan_Get_Table` WMI method.
//!
//! The DSDT associated with the `GMZN` device defines `WMAB(Arg0, Arg1,
//! Arg2)`. For method ID 5 it reads byte 0 and byte 1 of `Arg2` and returns
//! `GFAN`. `GFAN` returns `FAT2`, an 88-byte buffer containing two little-
//! endian `u32` counts and two ten-element little-endian `u32` arrays. This
//! module models only that proven binary representation in memory. It does not
//! open a WMI device, evaluate AML, or assign physical meaning to either array.

use crate::{FanTableQuery, WINDOWS_TABLE_POINT_COUNT};
use std::fmt;

pub const FAN_GET_TABLE_INPUT_SIZE_BYTES: usize = 2;
pub const FAN_GET_TABLE_TABLE_ENTRY_COUNT: usize = WINDOWS_TABLE_POINT_COUNT as usize;
pub const FAN_GET_TABLE_RESPONSE_SIZE_BYTES: usize =
    (2 + (2 * FAN_GET_TABLE_TABLE_ENTRY_COUNT)) * std::mem::size_of::<u32>();

/// Complete, lossless interpretation of the fixed `FAT2` response buffer.
/// Array element meaning remains intentionally unspecified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFanGetTableResponse {
    pub original: [u8; FAN_GET_TABLE_RESPONSE_SIZE_BYTES],
    pub fan_table_size: u32,
    pub fan_table: [u32; FAN_GET_TABLE_TABLE_ENTRY_COUNT],
    pub sensor_table_size: u32,
    pub sensor_table: [u32; FAN_GET_TABLE_TABLE_ENTRY_COUNT],
}

/// Encodes exactly the two bytes that `WMAB` reads for method ID 5.
pub const fn encode_confirmed_83ex_fan_get_table_request(query: FanTableQuery) -> [u8; 2] {
    [query.fan_id, query.sensor_id]
}

/// Decodes the exact `FAT2` shape returned by the target's static AML.
pub fn decode_confirmed_83ex_fan_get_table_response(
    bytes: &[u8],
) -> Result<DecodedFanGetTableResponse, FanGetTableAbiError> {
    let original = bytes
        .try_into()
        .map_err(|_| FanGetTableAbiError::UnexpectedResponseLength {
            actual: bytes.len(),
        })?;
    let fan_table_size = read_u32(&original, 0);
    let fan_table = read_table(&original, 4);
    let sensor_table_size = read_u32(&original, 44);
    let sensor_table = read_table(&original, 48);

    let expected = WINDOWS_TABLE_POINT_COUNT as u32;
    if fan_table_size != expected {
        return Err(FanGetTableAbiError::UnexpectedDeclaredSize {
            field: "FanTableSize",
            actual: fan_table_size,
            expected,
        });
    }
    if sensor_table_size != expected {
        return Err(FanGetTableAbiError::UnexpectedDeclaredSize {
            field: "SensorTableSize",
            actual: sensor_table_size,
            expected,
        });
    }

    Ok(DecodedFanGetTableResponse {
        original,
        fan_table_size,
        fan_table,
        sensor_table_size,
        sensor_table,
    })
}

fn read_u32(bytes: &[u8; FAN_GET_TABLE_RESPONSE_SIZE_BYTES], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed u32 slice"),
    )
}

fn read_table(
    bytes: &[u8; FAN_GET_TABLE_RESPONSE_SIZE_BYTES],
    offset: usize,
) -> [u32; FAN_GET_TABLE_TABLE_ENTRY_COUNT] {
    std::array::from_fn(|index| read_u32(bytes, offset + (index * 4)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FanGetTableAbiError {
    UnexpectedResponseLength {
        actual: usize,
    },
    UnexpectedDeclaredSize {
        field: &'static str,
        actual: u32,
        expected: u32,
    },
}

impl fmt::Display for FanGetTableAbiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedResponseLength { actual } => write!(
                f,
                "Fan_Get_Table response has {actual} bytes; expected {FAN_GET_TABLE_RESPONSE_SIZE_BYTES}"
            ),
            Self::UnexpectedDeclaredSize {
                field,
                actual,
                expected,
            } => write!(f, "{field} is {actual}; AML fixes it at {expected}"),
        }
    }
}

impl std::error::Error for FanGetTableAbiError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn response() -> [u8; FAN_GET_TABLE_RESPONSE_SIZE_BYTES] {
        let mut bytes = [0; FAN_GET_TABLE_RESPONSE_SIZE_BYTES];
        bytes[0..4].copy_from_slice(&10_u32.to_le_bytes());
        for index in 0..FAN_GET_TABLE_TABLE_ENTRY_COUNT {
            let start = 4 + (index * 4);
            bytes[start..start + 4].copy_from_slice(&((index + 1) as u32).to_le_bytes());
        }
        bytes[44..48].copy_from_slice(&10_u32.to_le_bytes());
        for index in 0..FAN_GET_TABLE_TABLE_ENTRY_COUNT {
            let start = 48 + (index * 4);
            bytes[start..start + 4].copy_from_slice(&((index + 11) as u32).to_le_bytes());
        }
        bytes
    }

    #[test]
    fn encodes_the_two_bytes_read_by_wmab() {
        assert_eq!(
            encode_confirmed_83ex_fan_get_table_request(
                FanTableQuery::documented_windows_observation()
            ),
            [1, 4]
        );
    }

    #[test]
    fn decodes_the_fixed_fat2_buffer_without_assigning_semantics() {
        let bytes = response();
        let decoded = decode_confirmed_83ex_fan_get_table_response(&bytes).unwrap();
        assert_eq!(decoded.original, bytes);
        assert_eq!(decoded.fan_table_size, 10);
        assert_eq!(decoded.fan_table, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(decoded.sensor_table_size, 10);
        assert_eq!(
            decoded.sensor_table,
            [11, 12, 13, 14, 15, 16, 17, 18, 19, 20]
        );
    }

    #[test]
    fn rejects_truncated_or_extended_responses() {
        assert_eq!(
            decode_confirmed_83ex_fan_get_table_response(&response()[..87]),
            Err(FanGetTableAbiError::UnexpectedResponseLength { actual: 87 })
        );
        let extended = [0; FAN_GET_TABLE_RESPONSE_SIZE_BYTES + 1];
        assert_eq!(
            decode_confirmed_83ex_fan_get_table_response(&extended),
            Err(FanGetTableAbiError::UnexpectedResponseLength { actual: 89 })
        );
    }

    #[test]
    fn rejects_sizes_inconsistent_with_the_fixed_aml_shape() {
        let mut bytes = response();
        bytes[44..48].copy_from_slice(&9_u32.to_le_bytes());
        assert_eq!(
            decode_confirmed_83ex_fan_get_table_response(&bytes),
            Err(FanGetTableAbiError::UnexpectedDeclaredSize {
                field: "SensorTableSize",
                actual: 9,
                expected: 10,
            })
        );
    }
}
