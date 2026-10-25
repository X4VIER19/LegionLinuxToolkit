//! Periodic read-only presentation of the existing status reader.
//! Uses normal terminal SIGINT: no signal handlers/unsafe, raw mode, daemon,
//! terminal rewriting or persistent resources requiring cleanup.
use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

#[derive(Debug, PartialEq)]
struct Options {
    interval: Duration,
    samples: Option<u64>,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut interval = None;
    let mut samples = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--interval" if interval.is_none() => {
                let seconds = args.next().ok_or("--interval requires seconds")?.parse::<f64>().map_err(|_| "invalid interval")?;
                if !seconds.is_finite() || !(1.0..=3600.0).contains(&seconds) {
                    return Err("interval must be between 1 and 3600 seconds".into());
                }
                interval = Some(Duration::from_secs_f64(seconds));
            }
            "--samples" if samples.is_none() => {
                let count = args.next().ok_or("--samples requires a positive integer")?.parse::<u64>().map_err(|_| "invalid sample count")?;
                if count == 0 { return Err("sample count must be positive".into()); }
                samples = Some(count);
            }
            _ => return Err("Usage: legion-cli monitor [--interval SECONDS] [--samples N]; Ctrl+C stops; no paths or control options".into()),
        }
    }
    Ok(Options {
        interval: interval.unwrap_or(Duration::from_secs(2)),
        samples,
    })
}

fn sample_loop(
    options: &Options,
    mut read: impl FnMut() -> Result<String, String>,
    mut emit: impl FnMut(&str) -> Result<(), String>,
    mut wait: impl FnMut(Duration),
) -> Result<u64, String> {
    let mut count = 0;
    loop {
        let begin = Instant::now();
        count += 1;
        let reading = match read() {
            Ok(text) => text,
            Err(error) => format!("ERROR adquisición: {error}; sin reutilizar valores anteriores"),
        };
        emit(&format!("[{count}] {reading}"))?;
        if options.samples.is_some_and(|limit| count >= limit) {
            return Ok(count);
        }
        // No catch-up bursts and no retry of a failed sample. Slow acquisitions
        // delay the cadence; the following acquisition is the next sample.
        wait(options.interval.saturating_sub(begin.elapsed()));
    }
}

pub(super) fn run(args: &[String]) -> Result<String, String> {
    let options = options(args)?; // Reject arguments before ALL host access.
    let reader = legion_linux::status::StatusReader::open()?;
    let mut out = io::stdout().lock();
    writeln!(out,"MONITOR solo lectura — intervalo {} s — Ctrl+C termina (SIGINT)\nRPM reportadas, sin asignación CPU/GPU. yogafan original/0 no es fiable; acpi_fan no acredita ambos ventiladores.",options.interval.as_secs_f64()).map_err(|e|e.to_string())?;
    let count = sample_loop(
        &options,
        || reader.read_once().map(|status| status.to_compact_text()),
        |line| {
            writeln!(out, "{line}")
                .and_then(|_| out.flush())
                .map_err(|e| e.to_string())
        },
        std::thread::sleep,
    )?;
    Ok(format!(
        "Monitor terminado: {count} muestras, sin escrituras."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arguments_have_safe_defaults_and_reject_invalid_or_control_options() {
        let parse =
            |args: &[&str]| options(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(
            parse(&[]).unwrap(),
            Options {
                interval: Duration::from_secs(2),
                samples: None
            }
        );
        assert_eq!(
            parse(&["--samples", "3", "--interval", "1.5"]).unwrap(),
            Options {
                interval: Duration::from_millis(1500),
                samples: Some(3)
            }
        );
        for args in [
            vec!["--interval", "0"],
            vec!["--interval", "NaN"],
            vec!["--interval", "-1"],
            vec!["--samples", "0"],
            vec!["--samples", "x"],
            vec!["--interval"],
            vec!["--samples"],
            vec!["--path", "/sys"],
            vec!["--real"],
            vec!["--interval", "2", "--interval", "3"],
        ] {
            assert!(parse(&args).is_err());
        }
    }
    #[test]
    fn finite_samples_use_injected_reads_and_expose_failures_without_retry() {
        let options = Options {
            interval: Duration::from_secs(2),
            samples: Some(3),
        };
        let mut calls = 0;
        let mut lines = Vec::new();
        let mut waits = Vec::new();
        assert_eq!(sample_loop(&options,|| {calls+=1; if calls==2 {Err("fixture sensor missing".into())} else {Ok("CPU fixture; acpi_fan/fan1=1500 RPM no validado; yogafan/fan1=0 no concluyente".into())}}, |line| {lines.push(line.to_string());Ok(())}, |delay|waits.push(delay)).unwrap(),3);
        assert_eq!(calls, 3);
        assert_eq!(waits.len(), 2);
        assert!(waits.iter().all(|delay| *delay <= Duration::from_secs(2)));
        assert!(lines[1].contains("ERROR adquisición: fixture sensor missing"));
        assert!(!lines[1].contains("1500"));
        assert!(lines[2].starts_with("[3]"));
    }
}
