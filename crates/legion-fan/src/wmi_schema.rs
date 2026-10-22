//! Model of the `LENOVO_FAN_METHOD` schema confirmed in Windows for the 83EX.
//!
//! This module records class metadata, not an executable Linux protocol. In
//! particular, the ACPI buffer representation, response byte layout, and a
//! safe Linux invocation remain unknown.

use legion_core::WmiKnowledge;
use serde::Serialize;

pub const LENOVO_FAN_METHOD_GUID: &str = "92549549-4BDE-4F06-AC04-CE8BF898DBAA";
pub const LENOVO_FAN_METHOD_CLASS: &str = "LENOVO_FAN_METHOD";
pub const FAN_GET_TABLE_METHOD_ID: u32 = 5;
pub const FAN_SET_TABLE_METHOD_ID: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WmiSchemaParameterDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WmiSchemaValueType {
    UInt8,
    UInt8Array,
    UInt32,
    UInt32Array,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WmiSchemaParameter {
    pub name: &'static str,
    pub direction: WmiSchemaParameterDirection,
    pub value_type: WmiSchemaValueType,
}

/// Explicitly separates Windows WMI schema metadata from the unknown ABI of
/// its possible future Linux/ACPI transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LinuxAcpiAbiBoundary {
    pub request_encoding: WmiKnowledge,
    pub response_encoding: WmiKnowledge,
    pub response_size_bytes: Option<usize>,
}

/// Schema obtained from Windows metadata for the target 83EX with BIOS
/// `NUCN15WW`. It confirms names, WMI method ID and CIM types only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ConfirmedFanGetTableSchema {
    pub class_name: &'static str,
    pub guid: &'static str,
    pub method_name: &'static str,
    pub wmi_method_id: u32,
    pub inputs: [WmiSchemaParameter; 2],
    pub outputs: [WmiSchemaParameter; 4],
    pub linux_acpi_abi: LinuxAcpiAbiBoundary,
}

pub const fn confirmed_83ex_fan_get_table_schema() -> ConfirmedFanGetTableSchema {
    ConfirmedFanGetTableSchema {
        class_name: LENOVO_FAN_METHOD_CLASS,
        guid: LENOVO_FAN_METHOD_GUID,
        method_name: "Fan_Get_Table",
        wmi_method_id: FAN_GET_TABLE_METHOD_ID,
        inputs: [
            WmiSchemaParameter {
                name: "FanID",
                direction: WmiSchemaParameterDirection::Input,
                value_type: WmiSchemaValueType::UInt8,
            },
            WmiSchemaParameter {
                name: "SensorID",
                direction: WmiSchemaParameterDirection::Input,
                value_type: WmiSchemaValueType::UInt8,
            },
        ],
        outputs: [
            WmiSchemaParameter {
                name: "FanTable",
                direction: WmiSchemaParameterDirection::Output,
                value_type: WmiSchemaValueType::UInt32Array,
            },
            WmiSchemaParameter {
                name: "FanTableSize",
                direction: WmiSchemaParameterDirection::Output,
                value_type: WmiSchemaValueType::UInt32,
            },
            WmiSchemaParameter {
                name: "SensorTable",
                direction: WmiSchemaParameterDirection::Output,
                value_type: WmiSchemaValueType::UInt32Array,
            },
            WmiSchemaParameter {
                name: "SensorTableSize",
                direction: WmiSchemaParameterDirection::Output,
                value_type: WmiSchemaValueType::UInt32,
            },
        ],
        linux_acpi_abi: LinuxAcpiAbiBoundary {
            request_encoding: WmiKnowledge::Unknown,
            response_encoding: WmiKnowledge::Unknown,
            response_size_bytes: None,
        },
    }
}

/// Metadata only: no write request, transport, or callable operation is
/// represented by this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct KnownFanSetTableMetadata {
    pub class_name: &'static str,
    pub guid: &'static str,
    pub method_name: &'static str,
    pub wmi_method_id: u32,
    pub input: WmiSchemaParameter,
    pub invocation: FanSetTableInvocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FanSetTableInvocation {
    Prohibited,
}

pub const fn known_83ex_fan_set_table_metadata() -> KnownFanSetTableMetadata {
    KnownFanSetTableMetadata {
        class_name: LENOVO_FAN_METHOD_CLASS,
        guid: LENOVO_FAN_METHOD_GUID,
        method_name: "Fan_Set_Table",
        wmi_method_id: FAN_SET_TABLE_METHOD_ID,
        input: WmiSchemaParameter {
            name: "FanTable",
            direction: WmiSchemaParameterDirection::Input,
            value_type: WmiSchemaValueType::UInt8Array,
        },
        invocation: FanSetTableInvocation::Prohibited,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_schema_confirms_cim_types_without_claiming_a_linux_abi() {
        let schema = confirmed_83ex_fan_get_table_schema();
        assert_eq!(schema.guid, LENOVO_FAN_METHOD_GUID);
        assert_eq!(schema.wmi_method_id, 5);
        assert_eq!(schema.inputs[0].value_type, WmiSchemaValueType::UInt8);
        assert_eq!(schema.inputs[1].value_type, WmiSchemaValueType::UInt8);
        assert_eq!(
            schema.outputs[0].value_type,
            WmiSchemaValueType::UInt32Array
        );
        assert_eq!(
            schema.linux_acpi_abi.request_encoding,
            WmiKnowledge::Unknown
        );
        assert_eq!(schema.linux_acpi_abi.response_size_bytes, None);
    }

    #[test]
    fn fan_set_table_is_metadata_only_and_prohibited() {
        let metadata = known_83ex_fan_set_table_metadata();
        assert_eq!(metadata.wmi_method_id, 6);
        assert_eq!(metadata.input.value_type, WmiSchemaValueType::UInt8Array);
        assert_eq!(metadata.invocation, FanSetTableInvocation::Prohibited);
    }
}
