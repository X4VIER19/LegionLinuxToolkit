//! Explicit one-shot query. This executable never opens fan curve endpoints.
use legion_linux::{
    fan_control_transport::{SimulatedControlPreconditions, simulated_control_preflight},
    fan_full_speed::{FullSpeedObservation, approved_one_shot_full_speed_read},
};
use std::{env, process::ExitCode};
const CONFIRM: &str = "I-UNDERSTAND-ONE-SHOT-FAN-FULL-SPEED-READ";
fn parse(args: &[String]) -> Result<bool, String> {
    if args.len() != 4
        || args[0] != "--confirm"
        || args[1] != CONFIRM
        || args[2] != "--format"
        || !matches!(args[3].as_str(), "text" | "json")
    {
        return Err(
            "usage: --confirm I-UNDERSTAND-ONE-SHOT-FAN-FULL-SPEED-READ --format text|json".into(),
        );
    }
    Ok(args[3] == "json")
}
fn main() -> ExitCode {
    let result = parse(&env::args().skip(1).collect::<Vec<_>>()).and_then(|json| {
        let observation = approved_one_shot_full_speed_read(CONFIRM)
            .unwrap_or_else(FullSpeedObservation::unavailable);
        render(observation, json)
    });
    match result {
        Ok((text, success)) => {
            println!("{text}");
            if success {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
        Err(error) => {
            eprintln!("FanFullSpeed state=error: {error}; no retry; control blocked");
            ExitCode::from(2)
        }
    }
}
fn render(observation: FullSpeedObservation, json: bool) -> Result<(String, bool), String> {
    let state = observation.result().state;
    let identity = legion_core::HostIdentity::declared_simulation(
        Default::default(),
        "not-collected-by-this-query",
    )
    .map_err(|error| error.to_string())?;
    let report = simulated_control_preflight(
        &legion_fixtures::legion_slim_5_16arp9_registry(),
        SimulatedControlPreconditions {
            identity: &identity,
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
    )
    .with_full_speed_observation(observation);
    let text = if json {
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
    } else {
        report.text()
    };
    Ok((
        text,
        matches!(
            state,
            legion_linux::fan_full_speed::FanFullSpeedState::Disabled
                | legion_linux::fan_full_speed::FanFullSpeedState::Enabled
        ),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmation_is_required_before_provider_and_unknown_never_unlocks() {
        assert!(parse(&[]).is_err());
        assert!(parse(&["--real".into()]).is_err());
        let (text, success) = render(FullSpeedObservation::simulation(&[]), true).unwrap();
        assert!(!success);
        let report: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(report["execution_enabled"], false);
        assert_eq!(report["full_speed_observation"]["result"]["state"], "error");
    }
}
