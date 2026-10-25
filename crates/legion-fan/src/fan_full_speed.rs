//! Pure codec for the specific NUCN15WW WMAE GetFeatureValue(0x04020000)
//! observation. This module performs no acquisition or firmware invocation.
use serde::Serialize;

pub const OTHER_METHOD_GUID: &str = "DC2A8805-3A8C-41BA-A6F7-092E0089CD3B";
pub const GET_FEATURE_METHOD: u32 = 17;
pub const FAN_FULL_SPEED_ID: u32 = 0x04020000;
pub const FULL_SPEED_RESPONSE_BYTES: usize = 20;
pub const fn full_speed_input() -> [u8; 4] {
    FAN_FULL_SPEED_ID.to_le_bytes()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FanFullSpeedState {
    Enabled,
    Disabled,
    Unknown,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FanFullSpeedResult {
    pub state: FanFullSpeedState,
    pub original_response: Vec<u8>,
    pub reported_integer: Option<u64>,
    pub error: Option<String>,
}

/// Same version-one 12-byte envelope as the fan read bridge; this separate
/// endpoint carries a little-endian u64 ACPI integer instead of FAT2.
pub fn decode_full_speed_response(bytes: &[u8]) -> FanFullSpeedResult {
    let failure = |message: String| FanFullSpeedResult {
        state: FanFullSpeedState::Error,
        original_response: bytes.to_vec(),
        reported_integer: None,
        error: Some(message),
    };
    if bytes.len() != FULL_SPEED_RESPONSE_BYTES {
        return failure("expected exactly 20 bytes".into());
    }
    let version = u16::from_le_bytes(bytes[0..2].try_into().unwrap());
    let header = u16::from_le_bytes(bytes[2..4].try_into().unwrap());
    let status = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let size = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    if version != 1 || header != 12 {
        return failure("unsupported envelope version/header".into());
    }
    if status != 0 {
        return failure(format!("kernel query status={status}; payload ignored"));
    }
    if size != 8 {
        return failure("success requires an 8-byte ACPI integer".into());
    }
    let value = u64::from_le_bytes(bytes[12..20].try_into().unwrap());
    FanFullSpeedResult {
        state: match value {
            0 => FanFullSpeedState::Disabled,
            1 => FanFullSpeedState::Enabled,
            _ => FanFullSpeedState::Unknown,
        },
        original_response: bytes.to_vec(),
        reported_integer: Some(value),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_input_and_explicit_states() {
        assert_eq!(full_speed_input(), [0, 0, 2, 4]);
        for (value, state) in [
            (0, FanFullSpeedState::Disabled),
            (1, FanFullSpeedState::Enabled),
            (2, FanFullSpeedState::Unknown),
            (u64::MAX, FanFullSpeedState::Unknown),
        ] {
            let mut bytes = [0; 20];
            bytes[0] = 1;
            bytes[2] = 12;
            bytes[8] = 8;
            bytes[12..].copy_from_slice(&value.to_le_bytes());
            let result = decode_full_speed_response(&bytes);
            assert_eq!(result.state, state);
            assert_eq!(result.original_response, bytes);
            bytes[4] = 2;
            let failed = decode_full_speed_response(&bytes);
            assert_eq!(failed.state, FanFullSpeedState::Error);
            assert_eq!(failed.reported_integer, None);
        }
    }
    #[test]
    fn rejects_incomplete_and_malformed_envelopes() {
        for size in [0, 12, 19, 21] {
            assert_eq!(
                decode_full_speed_response(&vec![0; size]).state,
                FanFullSpeedState::Error
            );
        }
        let mut bytes = [0; 20];
        bytes[0] = 1;
        bytes[2] = 12;
        bytes[8] = 8;
        for offset in [0, 2, 8] {
            let mut malformed = bytes;
            malformed[offset] = 99;
            assert_eq!(
                decode_full_speed_response(&malformed).state,
                FanFullSpeedState::Error
            );
        }
        bytes[4] = 3;
        assert_eq!(
            decode_full_speed_response(&bytes).state,
            FanFullSpeedState::Error
        );
    }
}
