use legion_core::{MatchResult, Snapshot, WmiResponse, WmiTransportError, parse_snapshot};
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
struct SimulatedResponse<T: Serialize> {
    source: &'static str,
    data: T,
}

#[derive(Serialize)]
struct ProfilesView<'a> {
    active: &'a str,
    available: &'a [String],
}

#[derive(Serialize)]
struct SimulatedWmiView {
    transport: &'static str,
    request: legion_core::WmiRequest,
    outcome: SimulatedWmiOutcome,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case", tag = "status")]
enum SimulatedWmiOutcome {
    Response { response: WmiResponse },
    Error { error: WmiTransportError },
}

fn run(args: Vec<String>) -> Result<String, String> {
    let mut view = "summary";
    let mut format = "text";
    let mut fixture = "valid";
    let mut device = "known";
    let mut scenario = "valid";
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "summary" | "profiles" | "attributes" | "compatibility" | "wmi" => {
                view = &args[index]
            }
            "--format" => {
                index += 1;
                format = args.get(index).ok_or("--format requires text or json")?;
            }
            "--fixture" => {
                index += 1;
                fixture = args.get(index).ok_or("--fixture requires a fixture name")?;
            }
            "--device" => {
                index += 1;
                device = args.get(index).ok_or("--device requires a static device name")?;
            }
            "--scenario" => {
                index += 1;
                scenario = args.get(index).ok_or("--scenario requires a fixture name")?;
            }
            "--help" | "-h" => {
                return Ok("Usage: legion-cli [summary|profiles|attributes] [--format text|json] [--fixture valid|missing|malformed]\n       legion-cli compatibility [--device known|unknown|incomplete] [--format text|json]\n       legion-cli wmi [--scenario valid|unavailable|inconsistent-buffer|unsupported] [--format text|json]\nAll data is embedded and simulated.".into())
            }
            other => return Err(format!("unsupported argument: {other}")),
        }
        index += 1;
    }
    if !matches!(format, "text" | "json") {
        return Err(format!("unsupported format: {format}"));
    }
    if view == "compatibility" {
        return compatibility(device, format);
    }
    if view == "wmi" {
        return wmi(scenario, format);
    }
    let input = legion_fixtures::named(fixture)
        .ok_or_else(|| format!("unknown simulated fixture: {fixture}"))?;
    let snapshot = parse_snapshot(input).map_err(|error| error.to_string())?;
    match (view, format) {
        ("summary", "text") => Ok(text_summary(&snapshot)),
        ("profiles", "text") => Ok(text_profiles(&snapshot)),
        ("attributes", "text") => Ok(text_attributes(&snapshot)),
        ("summary", "json") => json(&snapshot),
        ("profiles", "json") => json(&ProfilesView {
            active: &snapshot.active_profile,
            available: &snapshot.profiles,
        }),
        ("attributes", "json") => json(&snapshot.attributes),
        _ => unreachable!(),
    }
}

fn wmi(scenario: &str, format: &str) -> Result<String, String> {
    use legion_core::WmiTransport;

    let scenario = legion_fixtures::wmi_scenario(scenario)
        .ok_or_else(|| format!("unknown simulated WMI scenario: {scenario}"))?;
    let request = legion_fixtures::documented_fan_get_table_request();
    let outcome = match legion_fixtures::SimulatedWmiTransport::new(scenario).execute(&request) {
        Ok(response) => SimulatedWmiOutcome::Response { response },
        Err(error) => SimulatedWmiOutcome::Error { error },
    };
    let view = SimulatedWmiView {
        transport: "simulated-wmi",
        request,
        outcome,
    };
    match format {
        "json" => json(&view),
        "text" => Ok(text_wmi(&view)),
        _ => unreachable!(),
    }
}

fn compatibility(device: &str, format: &str) -> Result<String, String> {
    let query = legion_fixtures::compatibility_query(device)
        .ok_or_else(|| format!("unknown static device query: {device}"))?;
    let registry = legion_fixtures::legion_slim_5_16arp9_registry();
    let result = registry.match_query(&query);
    match format {
        "json" => json(&result),
        "text" => Ok(text_compatibility(&result)),
        _ => unreachable!(),
    }
}

fn json<T: Serialize>(data: &T) -> Result<String, String> {
    serde_json::to_string(&SimulatedResponse {
        source: "simulated",
        data,
    })
    .map_err(|error| format!("failed to serialize simulated data: {error}"))
}

fn text_summary(snapshot: &Snapshot) -> String {
    format!(
        "LegionLinuxToolkit — simulated data\nModel: {} ({})\nBIOS: {}\nActive profile: {}\nAttributes: {}\nFan sources: {}",
        snapshot.identity.model,
        snapshot.identity.machine_type,
        snapshot.identity.bios_version,
        snapshot.active_profile,
        snapshot.attributes.len(),
        snapshot.fans.len()
    )
}

fn text_profiles(snapshot: &Snapshot) -> String {
    format!(
        "Active: {}\nAvailable: {}",
        snapshot.active_profile,
        snapshot.profiles.join(", ")
    )
}

fn text_attributes(snapshot: &Snapshot) -> String {
    snapshot
        .attributes
        .iter()
        .map(|attribute| {
            format!(
                "{}: {} {} ({})",
                attribute.name,
                attribute.current_value,
                attribute.unit,
                attribute.kind.as_str()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn text_compatibility(result: &MatchResult) -> String {
    match result {
        MatchResult::Exact { profile } => {
            let capabilities = profile
                .capabilities
                .iter()
                .map(|capability| format!("- {}: {:?}", capability.key, capability.status))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "Static compatibility profile: {}\nControls enabled: no\nCapabilities:\n{capabilities}",
                profile.id
            )
        }
        MatchResult::Unknown { reason } => {
            format!("Static compatibility: unknown\nReason: {reason}")
        }
        MatchResult::Ambiguous { candidate_ids } => format!(
            "Static compatibility: ambiguous\nCandidates: {}",
            candidate_ids.join(", ")
        ),
        MatchResult::Incomplete { missing_fields } => format!(
            "Static compatibility: incomplete\nMissing: {}",
            missing_fields.join(", ")
        ),
    }
}

fn text_wmi(view: &SimulatedWmiView) -> String {
    match &view.outcome {
        SimulatedWmiOutcome::Response { response } => format!(
            "WMI simulation only\nOperation: {}\nResult: synthetic opaque buffer ({} bytes)\nABI: request={}, response={}\nNo firmware method was invoked.",
            view.request.operation.label(),
            response.bytes.len(),
            knowledge(&view.request.method.request_abi),
            knowledge(&view.request.method.response_abi),
        ),
        SimulatedWmiOutcome::Error { error } => format!(
            "WMI simulation only\nOperation: {}\nResult: simulated error: {error}\nNo firmware method was invoked.",
            view.request.operation.label(),
        ),
    }
}

fn knowledge(value: &legion_core::WmiKnowledge) -> &'static str {
    match value {
        legion_core::WmiKnowledge::Observed => "observed",
        legion_core::WmiKnowledge::Hypothesis => "hypothesis",
        legion_core::WmiKnowledge::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_special_and_control_characters() {
        let data = "quote: \" slash: \\ newline:\n";
        let output = json(&data).unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(decoded["source"], "simulated");
        assert_eq!(decoded["data"], data);
    }

    #[test]
    fn serializes_empty_strings() {
        let output = json(&"").unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(decoded["data"], "");
    }

    #[test]
    fn reports_missing_fixture_data() {
        let error = run(vec!["--fixture".into(), "missing".into()]).unwrap_err();
        assert!(error.contains("identity.bios_version"));
    }

    #[test]
    fn rejects_host_paths() {
        let error = run(vec!["--fixture".into(), "/sys".into()]).unwrap_err();
        assert!(error.contains("unknown simulated fixture"));
    }

    #[test]
    fn renders_static_compatibility_json() {
        let output = run(vec![
            "compatibility".into(),
            "--format".into(),
            "json".into(),
        ])
        .unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(decoded["source"], "simulated");
        assert_eq!(decoded["data"]["result"], "exact");
    }

    #[test]
    fn reports_unknown_and_incomplete_static_queries() {
        assert!(
            run(vec![
                "compatibility".into(),
                "--device".into(),
                "unknown".into()
            ])
            .unwrap()
            .contains("unknown")
        );
        assert!(
            run(vec![
                "compatibility".into(),
                "--device".into(),
                "incomplete".into()
            ])
            .unwrap()
            .contains("incomplete")
        );
    }

    #[test]
    fn renders_a_clearly_simulated_wmi_response_as_json() {
        let output = run(vec!["wmi".into(), "--format".into(), "json".into()]).unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(decoded["source"], "simulated");
        assert_eq!(decoded["data"]["transport"], "simulated-wmi");
        assert_eq!(
            decoded["data"]["outcome"]["response"]["provenance"]["artifact"],
            "static-simulation"
        );
        assert_eq!(
            decoded["data"]["request"]["method"]["method_identifier"],
            serde_json::Value::Null
        );
        assert_eq!(decoded["data"]["outcome"]["status"], "response");
    }

    #[test]
    fn renders_simulated_wmi_errors_without_executing_them() {
        let output = run(vec![
            "wmi".into(),
            "--scenario".into(),
            "inconsistent-buffer".into(),
        ])
        .unwrap();
        assert!(output.contains("WMI simulation only"));
        assert!(output.contains("inconsistent simulated WMI buffer size"));
    }

    #[test]
    fn rejects_paths_as_wmi_scenario_names() {
        let error = run(vec!["wmi".into(), "--scenario".into(), "/sys".into()]).unwrap_err();
        assert!(error.contains("unknown simulated WMI scenario"));
    }
}
