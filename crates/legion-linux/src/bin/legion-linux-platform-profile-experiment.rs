//! Isolated platform-profile experiment runner.
//!
//! This binary accepts only a closed list of in-memory scenarios. It has no
//! root-path argument, environment configuration, `--real` switch, or access
//! to the private sysfs provider.

use legion_linux::{
    LinuxPlatformProfileReading, isolated_platform_profile_scenario,
    run_isolated_platform_profile_experiment,
};
use serde::Serialize;
use std::{env, process::ExitCode};

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

#[derive(Serialize)]
struct ExperimentResponse {
    execution: &'static str,
    provenance: &'static str,
    scenario: &'static str,
    outcome: ExperimentOutcome,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case", tag = "status")]
enum ExperimentOutcome {
    Reading {
        reading: LinuxPlatformProfileReading,
    },
    Error {
        message: String,
    },
}

fn run(args: Vec<String>) -> Result<String, String> {
    let mut scenario_name = "valid";
    let mut format = "text";
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--scenario" => {
                index += 1;
                scenario_name = args
                    .get(index)
                    .ok_or("--scenario requires a simulated scenario name")?;
            }
            "--format" => {
                index += 1;
                format = args.get(index).ok_or("--format requires text or json")?;
            }
            "--help" | "-h" => return Ok(help().to_owned()),
            other => return Err(format!("unsupported isolated-simulation argument: {other}")),
        }
        index += 1;
    }
    if !matches!(format, "text" | "json") {
        return Err(format!("unsupported format: {format}"));
    }
    let scenario = isolated_platform_profile_scenario(scenario_name)
        .ok_or_else(|| format!("unknown isolated simulated scenario: {scenario_name}"))?;
    let response = ExperimentResponse {
        execution: "isolated-static-simulation",
        provenance: "static-simulation",
        scenario: scenario.name(),
        outcome: match run_isolated_platform_profile_experiment(scenario) {
            Ok(reading) => ExperimentOutcome::Reading { reading },
            Err(error) => ExperimentOutcome::Error {
                message: error.to_string(),
            },
        },
    };
    match format {
        "json" => serde_json::to_string(&response)
            .map_err(|error| format!("failed to serialize isolated simulation: {error}")),
        "text" => Ok(text(&response)),
        _ => unreachable!(),
    }
}

fn help() -> &'static str {
    "Usage: legion-linux-platform-profile-experiment [--scenario valid|missing-active|symlink|invalid-utf8|too-large|partial|permission-denied|concurrent-change] [--format text|json]\nThis tool runs embedded isolated simulations only. It cannot read host paths, sysfs, firmware, or power profiles."
}

fn text(response: &ExperimentResponse) -> String {
    match &response.outcome {
        ExperimentOutcome::Reading { reading } => format!(
            "Platform-profile isolated simulation only\nScenario: {}\nState: {}\nProvenance: {} ({})\nNo host path, sysfs, firmware, or energy profile was accessed or changed.",
            response.scenario,
            reading.profile.state.as_str(),
            reading.profile.provenance.source,
            reading.profile.provenance.confidence.as_str(),
        ),
        ExperimentOutcome::Error { message } => format!(
            "Platform-profile isolated simulation only\nScenario: {}\nProvenance: {}\nSimulated result: error: {message}\nNo host path, sysfs, firmware, or energy profile was accessed or changed.",
            response.scenario, response.provenance,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_json_is_explicitly_static_simulation() {
        let output = run(vec!["--format".into(), "json".into()]).unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(decoded["execution"], "isolated-static-simulation");
        assert_eq!(decoded["provenance"], "static-simulation");
        assert_eq!(decoded["scenario"], "valid");
        assert_eq!(
            decoded["outcome"]["reading"]["origin"]["StaticSimulation"]["root_label"],
            "isolated-experiment"
        );
        assert_eq!(
            decoded["outcome"]["reading"]["profile"]["provenance"]["artifact"],
            "static-simulation"
        );
    }

    #[test]
    fn reports_simulated_failures_without_a_valid_reading() {
        for scenario in [
            "missing-active",
            "symlink",
            "invalid-utf8",
            "too-large",
            "partial",
            "permission-denied",
            "concurrent-change",
        ] {
            let output = run(vec![
                "--scenario".into(),
                scenario.into(),
                "--format".into(),
                "json".into(),
            ])
            .unwrap();
            let decoded: serde_json::Value = serde_json::from_str(&output).unwrap();
            assert_eq!(decoded["execution"], "isolated-static-simulation");
            assert_eq!(decoded["provenance"], "static-simulation");
            assert_eq!(decoded["outcome"]["status"], "error");
            assert!(decoded["outcome"].get("reading").is_none());
        }
    }

    #[test]
    fn rejects_paths_and_every_real_access_switch() {
        for args in [
            vec!["--scenario", "/sys"],
            vec!["--scenario", "/proc"],
            vec!["--scenario", "/dev"],
            vec!["--real"],
            vec!["--sysfs"],
            vec!["--root", "/sys"],
            vec!["--path", "/sys/firmware/acpi"],
        ] {
            let error = run(args.into_iter().map(str::to_owned).collect()).unwrap_err();
            assert!(
                error.contains("unsupported")
                    || error.contains("unknown isolated simulated scenario")
            );
        }
    }

    #[test]
    fn help_describes_only_embedded_simulation() {
        let output = run(vec!["--help".into()]).unwrap();
        assert!(output.contains("embedded isolated simulations only"));
        assert!(output.contains("cannot read host paths"));
    }
}
