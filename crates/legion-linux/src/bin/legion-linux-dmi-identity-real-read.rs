//! Explicit future DMI read entry point. It is feature-gated and intentionally
//! has no path, environment, configuration, retry, or monitoring option.

use legion_linux::{DmiIdentityReading, approved_one_shot_dmi_identity_read};
use serde::Serialize;
use std::{env, process::ExitCode};

const CONFIRMATION: &str = "I-UNDERSTAND-READ-ONLY-DMI";

fn main() -> ExitCode {
    match parse_arguments(env::args().skip(1).collect()).and_then(run) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("DMI identity CLI error: {error}");
            ExitCode::from(2)
        }
    }
}

#[derive(Clone, Copy)]
enum OutputFormat {
    Text,
    Json,
}

fn parse_arguments(args: Vec<String>) -> Result<OutputFormat, String> {
    match args.as_slice() {
        [confirm, literal] if confirm == "--confirm" && literal == CONFIRMATION => {
            Ok(OutputFormat::Text)
        }
        [confirm, literal, format, value]
            if confirm == "--confirm"
                && literal == CONFIRMATION
                && format == "--format"
                && value == "json" =>
        {
            Ok(OutputFormat::Json)
        }
        [confirm, literal, format, value]
            if confirm == "--confirm"
                && literal == CONFIRMATION
                && format == "--format"
                && value == "text" =>
        {
            Ok(OutputFormat::Text)
        }
        [confirm, _] if confirm == "--confirm" => {
            Err("exact confirmation literal is required".to_owned())
        }
        _ => Err("usage: --confirm I-UNDERSTAND-READ-ONLY-DMI [--format text|json]".to_owned()),
    }
}

fn run(format: OutputFormat) -> Result<String, String> {
    let reading =
        approved_one_shot_dmi_identity_read(CONFIRMATION).map_err(|error| error.to_string())?;
    match format {
        OutputFormat::Text => Ok(format_text(&reading)),
        OutputFormat::Json => serde_json::to_string(&DmiResponse {
            execution: "explicit-one-shot-linux-dmi-read-only",
            reading: &reading,
        })
        .map_err(|error| error.to_string()),
    }
}

fn format_text(reading: &DmiIdentityReading) -> String {
    format!(
        "DMI identity observed, not verified\nOrigin: {:?}\nmanufacturer raw: {:?}\nmanufacturer normalized: {}\nproduct raw: {:?}\nproduct normalized: {}\nmodel raw: {:?}\nmodel normalized: {}\nBIOS raw: {:?}\nBIOS normalized: {}",
        reading.identity.origin(),
        reading.evidence.manufacturer.original,
        reading.evidence.manufacturer.normalized,
        reading.evidence.product_identifier.original,
        reading.evidence.product_identifier.normalized,
        reading.evidence.model.original,
        reading.evidence.model.normalized,
        reading.evidence.bios_version.original,
        reading.evidence.bios_version.normalized
    )
}

#[derive(Serialize)]
struct DmiResponse<'a> {
    execution: &'static str,
    reading: &'a DmiIdentityReading,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_is_required_before_any_future_read() {
        assert!(parse_arguments(vec![]).is_err());
        assert!(parse_arguments(vec!["--confirm".into(), "no".into()]).is_err());
        assert!(parse_arguments(vec!["--confirm".into(), CONFIRMATION.into()]).is_ok());
    }
}
