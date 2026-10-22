//! Explicit feature-gated CLI entry point for one experimental real read.
//!
//! It accepts no path, environment, configuration, fallback, retry, watch, or
//! write option. It first evaluates an embedded static compatibility record;
//! that record is not host detection and cannot verify DMI in this phase.

#[cfg(test)]
use legion_cli::STATIC_83EX_NUCN15WW_COMPATIBILITY;
use legion_cli::{
    StaticCompatibilityGate, require_static_platform_profile_compatibility,
    require_verified_host_identity,
};
use legion_linux::{
    ExperimentalRealReadError, LinuxPlatformProfileOrigin, LinuxPlatformProfileReading,
    approved_one_shot_platform_profile_read,
};
use serde::Serialize;
use std::{env, process::ExitCode};

const CONFIRMATION: &str = "I-UNDERSTAND-READ-ONLY-SYSFS";

fn main() -> ExitCode {
    match parse_arguments(env::args().skip(1).collect()).and_then(run) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("experimental platform-profile CLI error: {error}");
            ExitCode::from(2)
        }
    }
}

struct Invocation {
    confirmation: String,
    compatibility: String,
    format: OutputFormat,
}

#[derive(Clone, Copy)]
enum OutputFormat {
    Text,
    Json,
}

fn parse_arguments(args: Vec<String>) -> Result<Invocation, String> {
    let mut confirmation = None;
    let mut compatibility = None;
    let mut format = OutputFormat::Text;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--confirm" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or("--confirm requires the exact literal confirmation")?;
                if confirmation.replace(value.clone()).is_some() {
                    return Err("--confirm may be supplied only once".to_owned());
                }
            }
            "--compatibility" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or("--compatibility requires a closed static declaration")?;
                if compatibility.replace(value.clone()).is_some() {
                    return Err("--compatibility may be supplied only once".to_owned());
                }
            }
            "--format" => {
                index += 1;
                format = match args
                    .get(index)
                    .ok_or("--format requires text or json")?
                    .as_str()
                {
                    "text" => OutputFormat::Text,
                    "json" => OutputFormat::Json,
                    value => return Err(format!("unsupported format: {value}")),
                };
            }
            "--help" | "-h" => return Err(help().to_owned()),
            value => return Err(format!("unsupported experimental argument: {value}")),
        }
        index += 1;
    }

    let confirmation = confirmation.ok_or("--confirm is required")?;
    if confirmation != CONFIRMATION {
        return Err("the supplied confirmation does not match the required literal".to_owned());
    }
    Ok(Invocation {
        confirmation,
        compatibility: compatibility.ok_or("--compatibility is required")?,
        format,
    })
}

fn run(invocation: Invocation) -> Result<String, String> {
    let compatibility = require_static_platform_profile_compatibility(&invocation.compatibility)?;
    let reading = execute_after_identity_verification(&compatibility, || {
        // This is the only access call in this binary. The shared Linux facade
        // enforces its own one-shot guard and fixed sysfs paths.
        approved_one_shot_platform_profile_read(&invocation.confirmation)
            .map_err(render_real_read_error)
    })?;
    render_verified_reading(&reading, compatibility, invocation.format)
}

/// Keeps the identity barrier structurally before the sole provider call. A
/// test passes a simulated closure and proves an unverified declaration cannot
/// reach it.
fn execute_after_identity_verification<T>(
    compatibility: &StaticCompatibilityGate,
    attempt: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    require_verified_host_identity(compatibility)?;
    attempt()
}

fn render_verified_reading(
    reading: &LinuxPlatformProfileReading,
    compatibility: StaticCompatibilityGate,
    format: OutputFormat,
) -> Result<String, String> {
    if !matches!(
        reading.origin(),
        LinuxPlatformProfileOrigin::LinuxReadOnly { .. }
    ) {
        return Err("refusing to label a non-Linux result as a real read".to_owned());
    }
    match format {
        OutputFormat::Text => Ok(format!(
            "Experimental one-shot Linux platform-profile read\nStatic compatibility: {} ({:?}; host identity verified: no)\nRaw active: {:?}\nRaw choices: {:?}\nState: {}\nProvenance: {} ({})\nNo profile, fan, power, firmware, WMI, ACPI, or EC setting was changed.",
            compatibility.profile_id(),
            compatibility.capability_status(),
            reading.profile().raw.active,
            reading.profile().raw.choices,
            reading.profile().state.as_str(),
            reading.profile().provenance.source,
            reading.profile().provenance.confidence.as_str(),
        )),
        OutputFormat::Json => serde_json::to_string(&RealReadResponse {
            execution: "explicit-one-shot-linux-read-only",
            compatibility,
            reading,
        })
        .map_err(|error| format!("failed to serialize verified Linux read: {error}")),
    }
}

fn render_real_read_error(error: ExperimentalRealReadError) -> String {
    error.to_string()
}

#[derive(Serialize)]
struct RealReadResponse<'a> {
    execution: &'static str,
    compatibility: StaticCompatibilityGate,
    reading: &'a LinuxPlatformProfileReading,
}

fn help() -> &'static str {
    "Usage: legion-cli-platform-profile-real-read --confirm I-UNDERSTAND-READ-ONLY-SYSFS --compatibility 83ex-nucn15ww-static [--format text|json]\nThis feature-gated tool is blocked until a future reviewed host-identity verifier exists. The compatibility declaration is embedded static data, not host detection. It accepts no paths, environment, configuration, retry, monitor, fallback, or write options."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_absent_or_invalid_confirmation_before_any_read_call() {
        assert!(
            parse_arguments(vec![
                "--compatibility".into(),
                STATIC_83EX_NUCN15WW_COMPATIBILITY.into(),
            ])
            .is_err()
        );
        assert!(
            parse_arguments(vec![
                "--confirm".into(),
                "no".into(),
                "--compatibility".into(),
                STATIC_83EX_NUCN15WW_COMPATIBILITY.into(),
            ])
            .is_err()
        );
    }

    #[test]
    fn rejects_paths_and_unknown_static_compatibility_before_any_read_call() {
        let parsed = parse_arguments(vec![
            "--confirm".into(),
            CONFIRMATION.into(),
            "--compatibility".into(),
            "/sys".into(),
        ])
        .unwrap();
        assert!(require_static_platform_profile_compatibility(&parsed.compatibility).is_err());
        assert!(parse_arguments(vec!["--root".into(), "/sys".into()]).is_err());
    }

    #[test]
    fn refuses_to_render_a_simulated_reading_as_linux_read_only() {
        let reading = legion_linux::LinuxPlatformProfileReader::new(
            legion_linux::SimulatedPlatformProfileTree::new("test")
                .with_entry(
                    legion_linux::PlatformProfileEntry::Active,
                    legion_linux::SimulatedEntry::Regular(b"balanced\n".to_vec()),
                )
                .with_entry(
                    legion_linux::PlatformProfileEntry::Choices,
                    legion_linux::SimulatedEntry::Regular(b"balanced\n".to_vec()),
                ),
            4096,
        )
        .read_once()
        .unwrap();
        let gate =
            require_static_platform_profile_compatibility(STATIC_83EX_NUCN15WW_COMPATIBILITY)
                .unwrap();
        assert!(render_verified_reading(&reading, gate, OutputFormat::Json).is_err());
    }

    #[test]
    fn unverified_static_identity_never_reaches_the_provider_closure() {
        let gate =
            require_static_platform_profile_compatibility(STATIC_83EX_NUCN15WW_COMPATIBILITY)
                .unwrap();
        let mut attempted = false;
        let result = execute_after_identity_verification(&gate, || {
            attempted = true;
            Ok::<_, String>(())
        });
        assert!(result.is_err());
        assert!(!attempted);
    }
}
