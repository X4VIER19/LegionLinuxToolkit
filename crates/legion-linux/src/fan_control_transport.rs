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
    windows_reference: WindowsV2Reference,
    windows_reference_consistent: bool,
    linux_metadata_capture: Option<LinuxMetadataCapture>,
    full_speed_observation: Option<crate::fan_full_speed::FullSpeedObservation>,
    thermal_trial_protocol: legion_fan::control_trial::ThermalTrialProtocol,
    fact_capture_reference: FactCaptureReference,
    execution_enabled: bool,
    remaining_execution_blocks: [&'static str; 3],
}

/// Owner-reported completed Linux capture, not a fresh EC or selector read.
#[derive(Debug, Serialize)]
struct FactCaptureReference {
    source: &'static str,
    capture_bytes: usize,
    instances: [u16; 4],
    matching_catalogues: [u8; 1],
    uniquely_matches_fnt0: bool,
    historical_windows_levels_consistent: bool,
    proves_current_ec_configuration: bool,
    guarantees_future_sfan_selection: bool,
}

/// Historical observations supplied by the owner, never current Linux state.
#[derive(Debug, Serialize)]
pub struct WindowsV2Reference {
    source: &'static str,
    smart_fan_version: u8,
    legion_zone_version: u8,
    controller: &'static str,
    supported_power_modes: u32,
    god_mode_fn_q_switchable: bool,
    fan_full_speed_at_capture: bool,
    catalogue_mode: u16,
    catalogue_coordinates: [(u8, u8); 3],
    nominal_rpm: [u16; 10],
    software_minima: [u16; 10],
    default_indices: [u16; 10],
    manual_default_applications: u8,
    observed_rpm_range: [u16; 2],
    rollback_validated: bool,
}

fn windows_v2_reference() -> WindowsV2Reference {
    WindowsV2Reference {
        source: "owner-reported-windows-observation-LLT-2.34.0; historical-only",
        smart_fan_version: 7,
        legion_zone_version: 4,
        controller: "GodMode V2",
        supported_power_modes: 0x0001001f,
        god_mode_fn_q_switchable: true,
        fan_full_speed_at_capture: false,
        catalogue_mode: 255,
        catalogue_coordinates: [(1, 4), (2, 5), (1, 1)],
        nominal_rpm: [0, 1700, 1900, 2100, 2400, 2900, 3200, 3500, 3800, 5000],
        software_minima: legion_fan::control_payload::LLT_V2_V4_MINIMA,
        default_indices: legion_fan::control_payload::DEFAULT_INDICES,
        manual_default_applications: 2,
        observed_rpm_range: [1700, 2400],
        rollback_validated: false,
    }
}

impl WindowsV2Reference {
    /// Internal consistency of this recorded LLT V2 observation only. This
    /// does not check live capabilities or establish physical RPM semantics.
    fn validate(&self) -> bool {
        self.smart_fan_version == 7
            && self.legion_zone_version == 4
            && self.controller == "GodMode V2"
            && self.catalogue_mode == 255
            && self.catalogue_coordinates == [(1, 4), (2, 5), (1, 1)]
            && self.supported_power_modes == 0x0001001f
            && self.god_mode_fn_q_switchable
            && self.nominal_rpm.windows(2).all(|pair| pair[0] <= pair[1])
            && self.software_minima == legion_fan::control_payload::LLT_V2_V4_MINIMA
            && self.default_indices == legion_fan::control_payload::DEFAULT_INDICES
            && OfflineCurveProposal::new(
                self.default_indices,
                legion_fan::control_payload::CurveRules::DeclaredLltV2ToV4,
            )
            .is_ok()
    }
}

/// Recorded passive metadata, not a live collector or a real-read capability.
/// Public input remains untrusted and cannot enable execution. No RPM or
/// temperature measurements are inferred from readable source paths.
#[derive(Debug, Serialize)]
pub struct LinuxMetadataCapture {
    pub observed_date: &'static str,
    pub source: &'static str,
    pub platform_profile: &'static str,
    pub ac_online: bool,
    pub rpm_sources: Vec<&'static str>,
    pub thermal_sources: Vec<&'static str>,
}

impl SimulatedControlPreflight {
    /// Adds one independently acquired observation. It does not upgrade other
    /// preconditions, historical evidence, identity, or execution authority.
    pub fn with_full_speed_observation(
        mut self,
        observation: crate::fan_full_speed::FullSpeedObservation,
    ) -> Self {
        use legion_fan::fan_full_speed::FanFullSpeedState;
        if observation.source().starts_with("linux-wmi-one-shot") {
            self.source = "partial-live-preflight; only-FanFullSpeed-collected; other-checks-retain-their-source";
        }
        self.checks[2].status = match (observation.source(), observation.result().state) {
            ("static-simulation", FanFullSpeedState::Disabled) => "disabled-in-simulation-only",
            (_, FanFullSpeedState::Disabled) => "disabled-at-single-linux-WMI-observation",
            (_, FanFullSpeedState::Enabled) => "blocked-enabled",
            (_, FanFullSpeedState::Unknown) => "blocked-unknown",
            (_, FanFullSpeedState::Error) => "blocked-query-error",
        };
        self.full_speed_observation = Some(observation);
        self
    }
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
            "Source: {}\nWindows reference (historical): SmartFan={} LegionZone={} {}; FanFullSpeed=false at Windows capture only\nLinux metadata capture: {:?}\nFanFullSpeed observation: {:?}\nFACT capture reference (not current EC): {:?}\nProposed thermal trial (not authorization): {:?}\nGUID={} instance={} MethodId={}\nPayload ({} bytes): {hex}\n{checks}\nExecution enabled: false\nBlocks: {}",
            self.source,
            self.windows_reference.smart_fan_version,
            self.windows_reference.legion_zone_version,
            self.windows_reference.controller,
            self.linux_metadata_capture,
            self.full_speed_observation,
            self.fact_capture_reference,
            self.thermal_trial_protocol,
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
        match registry.match_query(&query) {
            MatchResult::Exact { .. } => Some(true),
            MatchResult::Incomplete { .. } => None,
            _ => Some(false),
        },
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
        windows_reference: windows_v2_reference(),
        windows_reference_consistent: windows_v2_reference().validate(),
        linux_metadata_capture: None,
        full_speed_observation: None,
        thermal_trial_protocol: legion_fan::control_trial::proposed_83ex_thermal_protocol(),
        fact_capture_reference: FactCaptureReference {
            source: "owner-reported-successful-Linux-FACT-capture; historical-not-live; module-unloaded",
            capture_bytes: 316,
            instances: [6, 9, 10, 11],
            matching_catalogues: [0],
            uniquely_matches_fnt0: true,
            historical_windows_levels_consistent: true,
            proves_current_ec_configuration: false,
            guarantees_future_sfan_selection: false,
        },
        execution_enabled: plan.execution_enabled(),
        remaining_execution_blocks: [
            "kernel constant false and no write endpoint",
            "fresh independent CPU/GPU and two reliable RPM channels plus other preconditions required",
            "separate authorization and approved partial-write recovery required",
        ],
    }
}

/// Reuses the same preflight with a recorded Linux capture. Historical Windows
/// evidence satisfies only reference requirements, never current Linux checks.
pub fn recorded_control_preflight(
    registry: &CompatibilityRegistry,
    identity: &HostIdentity,
    capture: LinuxMetadataCapture,
) -> SimulatedControlPreflight {
    let mut report = simulated_control_preflight(
        registry,
        SimulatedControlPreconditions {
            identity,
            platform_profile: None,
            gz44: None,
            fan_full_speed: None,
            llt_controller: None,
            validated_rpm_channels: None,
            validated_thermal_sources: None,
            catalogue_selection_known: None,
            ac_connected: None,
            windows_recovery_rehearsed: None,
        },
    );
    report.source = "recorded-evidence; not-live; no-execution-authority";
    // Identity was supplied from the recorded DMI capture; this remains a
    // record of that observation, not verification of the current host.
    if report.checks[0].status == "satisfied-in-simulation-only" {
        report.checks[0].status = "matched-at-recorded-linux-capture";
    }
    report.checks[1].status = if capture.platform_profile == "custom" {
        "custom-at-capture; GZ44-not-observed"
    } else {
        "blocked-profile-not-custom-at-capture"
    };
    report.checks[3].status = if report.windows_reference_consistent {
        "V2-confirmed-in-historical-windows-reference"
    } else {
        "blocked-inconsistent-Windows-reference"
    };
    report.checks[7].status = if capture.ac_online {
        "AC-online-at-recorded-capture"
    } else {
        "blocked-AC-offline-at-capture"
    };
    report.checks[8].status = "Windows-default-reapplied; partial-write-rollback-unproven";
    report.linux_metadata_capture = Some(capture);
    report
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

    #[test]
    fn validates_v2_reference_without_inventing_rollback_or_live_state() {
        let mut reference = windows_v2_reference();
        assert!(reference.validate());
        assert!(!reference.rollback_validated);
        assert_eq!(reference.manual_default_applications, 2);
        reference.smart_fan_version = 8;
        assert!(!reference.validate());
        reference.smart_fan_version = 7;
        reference.nominal_rpm[3] = 1;
        assert!(!reference.validate());
    }

    #[test]
    fn full_speed_states_never_enable_write_and_keep_simulation_provenance() {
        let identity =
            legion_fixtures::simulated_host_identity(legion_fixtures::HostIdentityScenario::Exact)
                .unwrap();
        for (integer, status, expected) in [
            (0u64, 0u8, "disabled-in-simulation-only"),
            (1, 0, "blocked-enabled"),
            (9, 0, "blocked-unknown"),
            (0, 2, "blocked-query-error"),
        ] {
            let mut bytes = [0; 20];
            bytes[0] = 1;
            bytes[2] = 12;
            bytes[4] = status;
            bytes[8] = 8;
            bytes[12..].copy_from_slice(&integer.to_le_bytes());
            let report = recorded_control_preflight(
                &legion_fixtures::legion_slim_5_16arp9_registry(),
                &identity,
                LinuxMetadataCapture {
                    observed_date: "fixture",
                    source: "static-simulation",
                    platform_profile: "custom",
                    ac_online: true,
                    rpm_sources: vec![],
                    thermal_sources: vec![],
                },
            )
            .with_full_speed_observation(
                crate::fan_full_speed::FullSpeedObservation::simulation(&bytes),
            );
            assert_eq!(report.checks[2].status, expected);
            assert!(!report.execution_enabled);
            assert_eq!(
                report.full_speed_observation.unwrap().source(),
                "static-simulation"
            );
        }
    }
}
