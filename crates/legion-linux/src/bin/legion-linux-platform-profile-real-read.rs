//! Feature-gated one-shot platform-profile reader.
//!
//! This binary is not built unless the non-default
//! `approved-real-platform-profile-read` feature is selected. It deliberately
//! has no path, root, configuration, environment, retry, or monitoring mode.

use legion_linux::{
    ExperimentalRealReadError, LinuxPlatformProfileReading, approved_one_shot_platform_profile_read,
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
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

struct Invocation {
    confirmation: String,
    format: OutputFormat,
}

#[derive(Clone, Copy)]
enum OutputFormat {
    Text,
    Json,
}

fn parse_arguments(args: Vec<String>) -> Result<Invocation, String> {
    let mut confirmation = None;
    let mut format = OutputFormat::Text;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--confirm" => {
                index += 1;
                let supplied = args
                    .get(index)
                    .ok_or("--confirm requires the exact literal confirmation")?;
                if confirmation.replace(supplied.clone()).is_some() {
                    return Err("--confirm may be supplied only once".to_owned());
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
            argument => return Err(format!("unsupported one-shot argument: {argument}")),
        }
        index += 1;
    }

    let confirmation = confirmation.ok_or("--confirm is required")?;
    if confirmation != CONFIRMATION {
        return Err("the supplied confirmation does not match the required literal".to_owned());
    }
    Ok(Invocation {
        confirmation,
        format,
    })
}

fn run(invocation: Invocation) -> Result<String, String> {
    // There is intentionally one call site: no loop, retry, fallback, or
    // second query is possible from this binary.
    let reading = approved_one_shot_platform_profile_read(&invocation.confirmation)
        .map_err(render_real_read_error)?;
    match invocation.format {
        OutputFormat::Text => Ok(text(&reading)),
        OutputFormat::Json => serde_json::to_string(&RealReadResponse {
            execution: "explicit-one-shot-linux-read-only",
            reading,
        })
        .map_err(|error| format!("failed to serialize verified Linux read: {error}")),
    }
}

fn render_real_read_error(error: ExperimentalRealReadError) -> String {
    error.to_string()
}

#[derive(Serialize)]
struct RealReadResponse {
    execution: &'static str,
    reading: LinuxPlatformProfileReading,
}

fn text(reading: &LinuxPlatformProfileReading) -> String {
    format!(
        "Explicit one-shot Linux read-only result\nState: {}\nProvenance: {} ({})\nNo profile, fan, power, firmware, WMI, ACPI, or EC setting was changed.",
        reading.profile().state.as_str(),
        reading.profile().provenance.source,
        reading.profile().provenance.confidence.as_str(),
    )
}

fn help() -> &'static str {
    "Usage: legion-linux-platform-profile-real-read --confirm I-UNDERSTAND-READ-ONLY-SYSFS [--format text|json]\nThis feature-gated tool performs one explicit read-only attempt and then exits. It accepts no paths, configuration, environment, monitoring, retry, or fallback options."
}
