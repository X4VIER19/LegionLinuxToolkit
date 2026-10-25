//! Separate experimental write boundary. There is deliberately no execution
//! method, host constructor, WmiOperation extension, device endpoint or CLI.
use legion_core::{CompatibilityRegistry, DeviceQuery, HostIdentity, MatchResult};
use legion_fan::control_payload::{LLT_PAYLOAD_BYTES, OfflineCurveProposal};
use legion_fan::{FAN_SET_TABLE_METHOD_ID, LENOVO_FAN_METHOD_GUID};
use serde::Serialize;

/// Injected observations only. Supplying these values never authorizes a
/// host operation; the report's acquisition class is always simulation.
pub struct SimulatedControlPreconditions<'a> {
    pub identity: &'a HostIdentity,
    pub platform_profile: Option<&'a str>,
    pub gz44: Option<u8>,
    pub fan_full_speed: Option<bool>,
    pub llt_controller: Option<&'a str>,
    pub validated_rpm_channels: Option<u8>,
    pub validated_thermal_sources: Option<bool>,
    pub catalogue_selection_known: Option<bool>,
    pub ac_connected: Option<bool>,
    pub windows_recovery_rehearsed: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ControlPreflightCheck {
    pub requirement: &'static str,
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct SimulatedControlPreflight {
    source: &'static str,
    guid: &'static str,
    instance: u8,
    method_id: u32,
    payload: Vec<u8>,
    checks: Vec<ControlPreflightCheck>,
    execution_enabled: bool,
    remaining_execution_blocks: [&'static str; 3],
}

impl SimulatedControlPreflight {
    pub fn text(&self) -> String {
        let checks = self
            .checks
            .iter()
            .map(|check| format!("{}: {}", check.requirement, check.status))
            .collect::<Vec<_>>()
            .join("\n");
        let hex = self
            .payload
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "Static simulation; no host observations\nGUID={} instance={} MethodId={}\nPayload ({} bytes): {hex}\n{checks}\nExecution enabled: false\nBlocks: {}",
            self.guid,
            self.instance,
            self.method_id,
            self.payload.len(),
            self.remaining_execution_blocks.join("; ")
        )
    }
}

/// One offline evaluation; no acquisition, retry or execution callback.
pub fn simulated_control_preflight(
    registry: &CompatibilityRegistry,
    evidence: SimulatedControlPreconditions<'_>,
) -> SimulatedControlPreflight {
    let query = DeviceQuery {
        manufacturer: evidence.identity.manufacturer().map(str::to_owned),
        model: evidence.identity.model().map(str::to_owned),
        machine_type: evidence.identity.product_identifier().map(str::to_owned),
        bios_version: evidence.identity.bios_version().map(str::to_owned),
        dmi_identifier: None,
    };
    let mut checks = Vec::new();
    let mut add = |requirement, state: Option<bool>| {
        checks.push(ControlPreflightCheck {
            requirement,
            status: match state {
                Some(true) => "satisfied-in-simulation-only",
                Some(false) => "incompatible",
                None => "missing-evidence",
            },
        })
    };
    add(
        "DMI/profile exact match in existing registry",
        Some(matches!(
            registry.match_query(&query),
            MatchResult::Exact { .. }
        )),
    );
    add(
        "Custom platform profile and GZ44=3 agree",
        match (evidence.platform_profile, evidence.gz44) {
            (Some(profile), Some(mode)) => Some(profile == "custom" && mode == 3),
            _ => None,
        },
    );
    add(
        "FanFullSpeed disabled",
        evidence.fan_full_speed.map(|active| !active),
    );
    add(
        "Effective LLT controller supports declared V2-V4 rules",
        evidence
            .llt_controller
            .map(|controller| matches!(controller, "V2" | "V3" | "V4")),
    );
    add(
        "Two validated RPM channels; mapping unknown",
        evidence.validated_rpm_channels.map(|count| count == 2),
    );
    add(
        "Validated current temperatures and thresholds",
        evidence.validated_thermal_sources,
    );
    add(
        "PMID/PGID/DCID and FNT/FI selection established",
        evidence.catalogue_selection_known,
    );
    add("AC connected", evidence.ac_connected);
    add(
        "Windows LLT recovery rehearsed independently",
        evidence.windows_recovery_rehearsed,
    );
    // Rules are an explicit simulation declaration, never detected variant.
    let proposal = OfflineCurveProposal::default_curve(
        legion_fan::control_payload::CurveRules::DeclaredLltV2ToV4,
    )
    .expect("fixed default satisfies the declared software rules");
    let plan = BlockedFanControlPlan::offline(&proposal);
    SimulatedControlPreflight {
        source: "static-simulation",
        guid: plan.guid(),
        instance: plan.instance(),
        method_id: plan.method_id(),
        payload: plan.payload().to_vec(),
        checks,
        execution_enabled: plan.execution_enabled(),
        remaining_execution_blocks: [
            "kernel constant false and no write endpoint",
            "real acquisition and freshness of preconditions not implemented",
            "separate authorization and approved partial-write recovery required",
        ],
    }
}

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
