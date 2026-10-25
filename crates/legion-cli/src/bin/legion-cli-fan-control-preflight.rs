//! Preparation only: explicit simulation, closed scenarios, no host paths.
use legion_linux::fan_control_transport::{
    LinuxMetadataCapture, SimulatedControlPreconditions, recorded_control_preflight,
    simulated_control_preflight,
};
use std::{env, process::ExitCode};

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("simulation-only preflight: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> Result<String, String> {
    let mut simulate = false;
    let mut recorded = false;
    let mut scenario = "missing-evidence";
    let mut format = "text";
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--simulate" if !simulate => simulate = true,
            "--recorded-evidence" if !recorded => recorded = true,
            "--scenario" => {
                index += 1;
                scenario = args.get(index).ok_or("missing scenario")?;
            }
            "--format" => {
                index += 1;
                format = args.get(index).ok_or("missing format")?;
            }
            _ => return Err("usage: --simulate [--scenario missing-evidence|complete-fixture|extreme|full-speed|wrong-host] OR --recorded-evidence [--format text|json]; neither mode reads the host".into()),
        }
        index += 1;
    }
    if simulate == recorded {
        return Err(
            "select exactly one of --simulate or --recorded-evidence; live access is unavailable"
                .into(),
        );
    }
    if !matches!(format, "text" | "json") {
        return Err("unsupported format".into());
    }
    if recorded {
        if args.iter().any(|argument| argument == "--scenario") {
            return Err("recorded evidence cannot be overridden by a simulation scenario".into());
        }
        let identity = legion_core::HostIdentity::observed_linux_dmi_unverified(
            legion_core::HostIdentityInput {
                manufacturer: Some("Lenovo".into()),
                product_identifier: Some("83EX".into()),
                model: Some("Legion Slim 5 16ARP9".into()),
                bios_version: Some("NUCN15WW".into()),
                ..Default::default()
            },
        )
        .map_err(|error| error.to_string())?;
        let report = recorded_control_preflight(
            &legion_fixtures::legion_slim_5_16arp9_registry(),
            &identity,
            LinuxMetadataCapture {
                observed_date: "2026-10-24",
                source: "recorded-fixed-DMI/platform_profile/power_supply/hwmon-metadata; no sensor values read",
                platform_profile: "low-power",
                ac_online: true,
                rpm_sources: vec![
                    "acpi_fan/fan1_input; module=fan",
                    "yogafan/fan1_input; module=yogafan-original-incompatible",
                ],
                thermal_sources: vec![
                    "acpitz/temp1_input; component-unvalidated",
                    "k10temp/temp1_input; label=Tctl; threshold-unvalidated",
                ],
            },
        );
        return if format == "json" {
            serde_json::to_string_pretty(&report).map_err(|error| error.to_string())
        } else {
            Ok(report.text())
        };
    }
    if !matches!(
        scenario,
        "missing-evidence" | "complete-fixture" | "extreme" | "full-speed" | "wrong-host"
    ) {
        return Err("unknown embedded scenario; paths are not accepted".into());
    }
    let identity = legion_fixtures::simulated_host_identity(if scenario == "wrong-host" {
        legion_fixtures::HostIdentityScenario::BiosMismatch
    } else {
        legion_fixtures::HostIdentityScenario::Exact
    })
    .map_err(|error| error.to_string())?;
    let complete = scenario != "missing-evidence";
    let evidence = SimulatedControlPreconditions {
        identity: &identity,
        platform_profile: complete.then_some(if scenario == "extreme" {
            "max-power"
        } else {
            "custom"
        }),
        gz44: complete.then_some(if scenario == "extreme" { 7 } else { 3 }),
        fan_full_speed: complete.then_some(scenario == "full-speed"),
        llt_controller: complete.then_some("V2"),
        validated_rpm_channels: complete.then_some(2),
        validated_thermal_sources: complete.then_some(true),
        catalogue_selection_known: complete.then_some(true),
        ac_connected: complete.then_some(true),
        windows_recovery_rehearsed: complete.then_some(true),
    };
    let report =
        simulated_control_preflight(&legion_fixtures::legion_slim_5_16arp9_registry(), evidence);
    if format == "json" {
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())
    } else {
        Ok(report.text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn json(scenario: &str) -> serde_json::Value {
        serde_json::from_str(
            &run(vec![
                "--simulate".into(),
                "--scenario".into(),
                scenario.into(),
                "--format".into(),
                "json".into(),
            ])
            .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn complete_simulation_never_enables_execution_and_preserves_payload() {
        let report = json("complete-fixture");
        assert_eq!(report["source"], "static-simulation");
        assert_eq!(report["execution_enabled"], false);
        assert_eq!(report["method_id"], 6);
        let payload = report["payload"].as_array().unwrap();
        assert_eq!(payload.len(), 64);
        assert_eq!(payload[0], 1);
        for point in 0..10 {
            assert_eq!(payload[6 + point * 2], point + 1);
        }
        assert!(payload[26..].iter().all(|byte| byte == 0));
        assert_eq!(
            report["remaining_execution_blocks"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
    #[test]
    fn unknown_and_conflicting_preconditions_fail_closed() {
        for (scenario, status) in [
            ("missing-evidence", "missing-evidence"),
            ("extreme", "incompatible"),
            ("full-speed", "incompatible"),
            ("wrong-host", "incompatible"),
        ] {
            let report = json(scenario);
            assert_eq!(report["execution_enabled"], false);
            assert!(
                report["checks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|check| check["status"] == status)
            );
        }
        assert!(run(vec![]).is_err());
        assert!(run(vec!["--real".into()]).is_err());
        assert!(
            run(vec![
                "--simulate".into(),
                "--scenario".into(),
                "/sys".into()
            ])
            .is_err()
        );
    }

    #[test]
    fn historical_windows_state_never_satisfies_linux_full_speed_or_sensors() {
        let output = run(vec![
            "--recorded-evidence".into(),
            "--format".into(),
            "json".into(),
        ])
        .unwrap();
        let report: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(report["execution_enabled"], false);
        assert_eq!(report["windows_reference"]["smart_fan_version"], 7);
        assert_eq!(report["windows_reference"]["legion_zone_version"], 4);
        assert_eq!(
            report["windows_reference"]["supported_power_modes"],
            0x0001001f
        );
        assert_eq!(
            report["windows_reference"]["fan_full_speed_at_capture"],
            false
        );
        assert_eq!(report["windows_reference"]["rollback_validated"], false);
        assert_eq!(
            report["linux_metadata_capture"]["platform_profile"],
            "low-power"
        );
        assert_eq!(report["checks"][2]["status"], "missing-evidence");
        assert_eq!(report["checks"][4]["status"], "missing-evidence");
        assert_eq!(report["checks"][5]["status"], "missing-evidence");
        assert!(run(vec!["--recorded-evidence".into(), "--simulate".into()]).is_err());
    }
}
