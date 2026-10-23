//! One-shot, feature-gated Linux hwmon RPM observer.
//!
//! This binary is absent from normal builds. It accepts no path, environment,
//! configuration, polling, retry, or write option. It must be explicitly
//! compiled with `approved-real-hwmon-rpm-read` and confirmed on every run.

use legion_linux::{HwmonLinuxReading, approved_one_shot_hwmon_rpm_read};
use serde::Serialize;
use std::{env, process::ExitCode};

const CONFIRMATION: &str = "I-UNDERSTAND-READ-ONLY-HWMON-RPM";

fn main() -> ExitCode {
    match parse(env::args().skip(1).collect()).and_then(run) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("experimental hwmon RPM CLI error: {error}");
            ExitCode::from(2)
        }
    }
}

#[derive(Clone, Copy)]
enum OutputFormat {
    Text,
    Json,
}

struct Invocation {
    confirmation: String,
    format: OutputFormat,
}

fn parse(args: Vec<String>) -> Result<Invocation, String> {
    let mut confirmation = None;
    let mut format = OutputFormat::Text;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--confirm" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or("--confirm requires the exact literal")?;
                if confirmation.replace(value.clone()).is_some() {
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
        format,
    })
}

fn run(invocation: Invocation) -> Result<String, String> {
    let reading = approved_one_shot_hwmon_rpm_read(&invocation.confirmation)
        .map_err(|error| error.to_string())?;
    match invocation.format {
        OutputFormat::Text => Ok(text(&reading)),
        OutputFormat::Json => serde_json::to_string(&Response {
            execution: "explicit-one-shot-linux-read-only",
            reading: &reading,
        })
        .map_err(|error| format!("could not serialize hwmon observation: {error}")),
    }
}

fn text(reading: &HwmonLinuxReading) -> String {
    let fans = reading
        .fans
        .iter()
        .map(|fan| format!(
            "- driver={} channel=fan{}_input reported={} RPM [classification={:?}; validation={:?}; interpretation={:?}; provenance={}]",
            fan.driver_name,
            fan.channel,
            fan.rpm.value.map_or_else(|| "<none>".to_owned(), |value| value.to_string()),
            fan.source_classification,
            fan.validation,
            fan.interpretation,
            fan.rpm.provenance.source,
        ))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Experimental one-shot Linux hwmon RPM observation\nDMI provenance: {:?}; verification: observed but unverified\n{fans}\nChannels are not assigned to CPU or GPU. No fan, PWM, profile, WMI, ACPI, EC, or firmware setting was changed.",
        reading.dmi_identity.identity.origin(),
    )
}

#[derive(Serialize)]
struct Response<'a> {
    execution: &'static str,
    reading: &'a HwmonLinuxReading,
}

fn help() -> &'static str {
    "Usage: legion-cli-hwmon-rpm-real-read --confirm I-UNDERSTAND-READ-ONLY-HWMON-RPM [--format text|json]\nThis feature-gated command performs one explicit read-only observation and exits. It has fixed DMI and /sys/class/hwmon paths; it accepts no path, configuration, environment, retry, monitoring, fallback, or write options. A `yogafan` source remains unvalidated unless a separate driver-identity proof is added."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_confirmation_and_paths_before_any_provider_call() {
        assert!(parse(vec![]).is_err());
        assert!(parse(vec!["--confirm".into(), "/sys".into()]).is_err());
        assert!(parse(vec!["--root".into(), "/sys".into()]).is_err());
    }
}
