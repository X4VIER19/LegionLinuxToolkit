//! Offline by default. Optional explicit capture emits the original binary,
//! including failures, for later analysis without another firmware query.
use std::{env, path::Path, process};

fn run(arguments: &[String]) -> Result<(), String> {
    match arguments {
        [action, capture, dsdt, flag, format]
            if action == "analyze" && flag == "--format" && matches!(format.as_str(), "json" | "text") =>
        {
            let result = legion_linux::fact_capture::analyze_fact_files(Path::new(capture), Path::new(dsdt))?;
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&result).map_err(|error| error.to_string())?);
            } else {
                println!("Source: {}", result.source);
                for record in &result.records {
                    println!("instance {} original: {:?}", record.instance, record.original);
                    println!("SFTW values (no new physical interpretation): {:?}", record.values);
                }
                println!("matching FNT catalogues: {:?}; Windows levels consistent: {}", result.matching_catalogues, result.historical_windows_levels_consistent);
                println!("{} No write authorization.", result.conclusion);
            }
            Ok(())
        }
        #[cfg(feature = "experimental-fact-read")]
        [action, flag, confirmation] if action == "capture" && flag == "--confirm" => {
            use std::io::Write;
            let bytes = legion_linux::fact_capture::approved_fact_capture(confirmation)?;
            std::io::stdout().lock().write_all(&bytes).map_err(|error| error.to_string())?;
            legion_fan::fact::decode_fact_capture(&bytes)?;
            eprintln!("FACT batch captured: four queries, no retries; unverified Linux observation, no write authorization");
            Ok(())
        }
        _ => Err("usage: legion-linux-fact analyze <absolute-capture> <absolute-DSDT.dsl> --format <text|json>; experimental capture requires its feature and exact confirmation".into()),
    }
}

fn main() {
    if let Err(error) = run(&env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("FACT: {error}");
        process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unconfirmed_capture_and_extra_arguments_without_acquisition() {
        assert!(run(&[]).is_err());
        assert!(run(&["capture".into()]).is_err());
        assert!(run(&["capture".into(), "--confirm".into(), "wrong".into()]).is_err());
        assert!(run(&["capture".into(), "--path".into(), "/sys".into()]).is_err());
    }
}
