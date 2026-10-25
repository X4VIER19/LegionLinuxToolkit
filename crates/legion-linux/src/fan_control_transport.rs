//! Separate experimental write boundary. There is deliberately no execution
//! method, host constructor, WmiOperation extension, device endpoint or CLI.
use legion_fan::control_payload::{LLT_PAYLOAD_BYTES, OfflineCurveProposal};
use legion_fan::{FAN_SET_TABLE_METHOD_ID, LENOVO_FAN_METHOD_GUID};

/// Immutable offline description sharing the read transport's target/instance.
/// It cannot be passed to the read-only `WmiTransport` request API.
pub struct BlockedFanControlPlan {
    payload: [u8; LLT_PAYLOAD_BYTES],
}

impl BlockedFanControlPlan {
    pub fn offline(proposal: &OfflineCurveProposal) -> Self {
        Self {
            payload: proposal.payload(),
        }
    }
    pub const fn guid(&self) -> &'static str {
        LENOVO_FAN_METHOD_GUID
    }
    pub const fn instance(&self) -> u8 {
        crate::fan_wmi_transport::FAN_WMI_INSTANCE
    }
    pub const fn method_id(&self) -> u32 {
        FAN_SET_TABLE_METHOD_ID
    }
    pub const fn payload(&self) -> &[u8; LLT_PAYLOAD_BYTES] {
        &self.payload
    }
    pub const fn execution_enabled(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_fan::control_payload::CurveRules;
    #[test]
    fn offline_plan_never_grants_execution() {
        let proposal = OfflineCurveProposal::default_curve(CurveRules::DeclaredLltV2ToV4).unwrap();
        let plan = BlockedFanControlPlan::offline(&proposal);
        assert_eq!(plan.method_id(), 6);
        assert_eq!(plan.instance(), 0);
        assert_eq!(plan.payload().len(), 64);
        assert!(!plan.execution_enabled());
    }
}
