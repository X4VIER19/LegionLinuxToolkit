//! Strictly offline decoder for a previously captured fan endpoint response.
//!
//! This binary never opens the experimental kernel device. Its only input is
//! one explicitly supplied regular file outside `/dev`, `/proc`, and `/sys`.

use legion_linux::analyze_offline_fan_response_file;
use std::{env, path::PathBuf, process};

fn main() {
    match parse_arguments(env::args().skip(1).collect()).and_then(run) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("offline fan response analysis failed: {error}");
            process::exit(2);
        }
    }
}

fn parse_arguments(arguments: Vec<String>) -> Result<(PathBuf, OutputFormat), String> {
    match arguments.as_slice() {
        [path, format_flag, format] if format_flag == "--format" => {
            let format = match format.as_str() {
                "text" => OutputFormat::Text,
                "json" => OutputFormat::Json,
                _ => return Err("--format must be text or json".to_owned()),
            };
            Ok((PathBuf::from(path), format))
        }
        _ => Err(
            "usage: legion-linux-fan-response-analyzer <absolute-capture-file> --format <text|json>"
                .to_owned(),
        ),
    }
}

fn run((path, format): (PathBuf, OutputFormat)) -> Result<(), String> {
    let analysis = analyze_offline_fan_response_file(&path).map_err(|error| error.to_string())?;
    match format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&analysis).map_err(|error| error.to_string())?
        ),
        OutputFormat::Text => {
            println!("provenance: offline-captured-unverified");
            println!("identity verified: false");
            println!(
                "contract: version {}, header {}, status {}, payload {} bytes",
                analysis.contract_version,
                analysis.contract_header_size,
                analysis.contract_status,
                analysis.payload_size
            );
            println!("original response bytes: {:?}", analysis.original_response);
            println!("original FAT2 bytes: {:?}", analysis.original_fat2);
            println!(
                "FanTable ({} opaque values): {:?}",
                analysis.fan_table_size, analysis.fan_table
            );
            println!(
                "SensorTable ({} opaque values): {:?}",
                analysis.sensor_table_size, analysis.sensor_table
            );
            println!("No physical units or sensor meanings are assigned.");
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Text,
    Json,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_closed_argument_shape_and_output_format() {
        assert!(
            parse_arguments(vec![
                "capture.bin".to_owned(),
                "--format".to_owned(),
                "json".to_owned()
            ])
            .is_ok()
        );
        assert!(parse_arguments(vec!["capture.bin".to_owned()]).is_err());
        assert!(
            parse_arguments(vec![
                "capture.bin".to_owned(),
                "--format".to_owned(),
                "yaml".to_owned()
            ])
            .is_err()
        );
    }
}
