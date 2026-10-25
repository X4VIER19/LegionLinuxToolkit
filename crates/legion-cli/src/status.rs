//! Explicit status invocation; argument validation precedes all host access.
fn format(args: &[String]) -> Result<&str, String> {
    match args {
        [] => Ok("text"),
        [flag, format] if flag == "--format" && matches!(format.as_str(), "text" | "json") => Ok(format),
        _ => Err("Usage: legion-cli status [--format text|json]; one read-only snapshot, no paths or monitoring".into()),
    }
}

pub(super) fn run(args: &[String]) -> Result<String, String> {
    let format = format(args)?;
    let status = legion_linux::status::read_once()?;
    match format {
        "json" => serde_json::to_string_pretty(&status).map_err(|e| e.to_string()),
        _ => Ok(status.to_text()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_one_status_format_is_accepted_without_triggering_acquisition() {
        assert_eq!(super::format(&[]).unwrap(), "text");
        assert_eq!(
            super::format(&["--format".into(), "json".into()]).unwrap(),
            "json"
        );
        for args in [
            vec!["--watch"],
            vec!["--path", "/sys"],
            vec!["--format", "xml"],
            vec!["--interval", "1"],
        ] {
            assert!(
                super::format(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err()
            );
        }
    }
}
