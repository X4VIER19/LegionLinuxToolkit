//! Independent, bounded NO-WRITE supervision rehearsal, simulation by default.
use std::{env, process::ExitCode};

fn run(args: &[String]) -> Result<String, String> {
    let registry = legion_fixtures::legion_slim_5_16arp9_registry();
    let result=match args {
        [mode] if mode=="--simulate"=>legion_linux::trial_monitor::run_simulated_rehearsal(&registry),
        #[cfg(feature="experimental-fan-trial-monitor")]
        [mode,flag,literal,idle] if mode=="--live-no-write-rehearsal" && flag=="--confirm" && idle=="--attest-idle" => {
            legion_linux::trial_monitor::run_live_rehearsal(&registry,literal,true)
        }
        #[cfg(feature="experimental-fan-trial-monitor")]
        [mode,flag,literal,idle] if mode=="--staged-default-trial" && flag=="--confirm" && idle=="--attest-idle" => {
            legion_linux::trial_monitor::run_staged_default_trial(&registry,literal,true)
        }
        _=>return Err("usage: --simulate OR (explicit experimental feature) --live-no-write-rehearsal --confirm I-AUTHORIZE-READ-ONLY-180S-83EX-SUPERVISION --attest-idle; no paths, modules, writes or indefinite monitoring".into()),
    }?;
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}
fn main() -> ExitCode {
    match run(&env::args().skip(1).collect::<Vec<_>>()) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Supervision stopped: {error}");
            ExitCode::from(2)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_arguments_cannot_start_live_acquisition() {
        for args in [
            vec![],
            vec!["--real".into()],
            vec!["--live-no-write-rehearsal".into()],
            vec!["--root".into(), "/sys".into()],
        ] {
            assert!(run(&args).is_err());
        }
    }
}
