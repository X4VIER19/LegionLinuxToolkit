use legion_core::{
    MatchResult, PlatformProfileReading, Snapshot, TelemetrySnapshot, TelemetryValue, WmiResponse,
    WmiTransportError, parse_snapshot,
};
use legion_linux::{
    HwmonFanQuery, LinuxHwmonFanReader, LinuxPlatformProfileReader, PlatformProfileEntry,
    PlatformProfileQuery, SimulatedEntry, SimulatedHwmonDevice, SimulatedHwmonEntry,
    SimulatedHwmonTree, SimulatedPlatformProfileTree,
};
use serde::Serialize;
use std::{env, process::ExitCode};
mod curves;
mod status;

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("CLI error: {error}");
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
    if args.first().is_some_and(|arg| arg == "status") {
        return status::run(&args[1..]);
    }
    if args.first().is_some_and(|arg| arg == "curve") {
        return curves::run(&args[1..]);
    }
    let mut view = "summary";
    let mut format = "text";
    let mut fixture = "valid";
    let mut device = "known";
    let mut scenario = "valid";
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "summary" | "profiles" | "attributes" | "compatibility" | "telemetry" | "wmi"
            | "platform-profile" | "fan-rpm" => view = &args[index],
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
                return Ok("Usage: legion-cli [summary|profiles|attributes] [--format text|json] [--fixture valid|missing|malformed]\n       legion-cli compatibility [--device known|unknown|incomplete] [--format text|json]\n       legion-cli telemetry [--scenario valid|absent|unvalidated|inconsistent|read-error] [--format text|json]\n       legion-cli platform-profile [--scenario valid|active-only|missing-active|empty-active|empty-choices|unknown|duplicate|inconsistent|malformed] [--format text|json]\n       legion-cli fan-rpm [--scenario quirk-two-channel|one-channel|zero-generic|read-error|duplicate-source|device-disappeared|missing-device|invalid-rpm] [--format text|json]\n       legion-cli wmi [--scenario valid|unavailable|inconsistent-buffer|unsupported] [--format text|json]\nCurve editor: legion-cli curve --help\nStatus: legion-cli status [--format text|json] (one real read-only snapshot)\nOther diagnostics are embedded/simulated; curve documents use explicit local regular files. No hardware controls.".into())
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
    if view == "telemetry" {
        return telemetry(scenario, format);
    }
    if view == "platform-profile" {
        return platform_profile(scenario, format);
    }
    if view == "fan-rpm" {
        return fan_rpm(scenario, format);
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

/// `fan-rpm` intentionally creates only an in-memory hwmon-shaped tree. The
/// closed scenario set gives this CLI no path, environment, or fallback route
/// to `/sys/class/hwmon`.
fn fan_rpm(scenario: &str, format: &str) -> Result<String, String> {
    let tree = simulated_hwmon_scenario(scenario)
        .ok_or_else(|| format!("unknown simulated fan-rpm scenario: {scenario}"))?;
    let readings = LinuxHwmonFanReader::new(tree)
        .read_once()
        .map_err(|error| format!("simulated hwmon fan query failed: {error}"))?;
    match format {
        "json" => json(&readings),
        "text" => Ok(text_fan_rpm(&readings)),
        _ => unreachable!(),
    }
}

fn simulated_hwmon_scenario(scenario: &str) -> Option<SimulatedHwmonTree> {
    let valid_quirk = || {
        SimulatedHwmonTree::new("legion-cli-embedded-hwmon").with_device(
            "simulated-yogafan-83ex",
            SimulatedHwmonDevice::validated_83ex_quirk(b"yogafan\n".to_vec())
                .with_entry(
                    "fan1_input",
                    SimulatedHwmonEntry::Regular(b"1900\n".to_vec()),
                )
                .with_entry(
                    "fan2_input",
                    SimulatedHwmonEntry::Regular(b"1900\n".to_vec()),
                ),
        )
    };
    match scenario {
        "quirk-two-channel" => Some(valid_quirk()),
        "one-channel" => Some(
            SimulatedHwmonTree::new("legion-cli-one-channel").with_device(
                "simulated-acpi-fan",
                SimulatedHwmonDevice::generic(b"acpi_fan\n".to_vec()).with_entry(
                    "fan1_input",
                    SimulatedHwmonEntry::Regular(b"1900\n".to_vec()),
                ),
            ),
        ),
        "zero-generic" => Some(
            SimulatedHwmonTree::new("legion-cli-zero").with_device(
                "simulated-original-yogafan",
                SimulatedHwmonDevice::generic(b"yogafan\n".to_vec())
                    .with_entry("fan1_input", SimulatedHwmonEntry::Regular(b"0\n".to_vec())),
            ),
        ),
        "read-error" => Some(
            SimulatedHwmonTree::new("legion-cli-error").with_device(
                "simulated-acpi-fan",
                SimulatedHwmonDevice::generic(b"acpi_fan\n".to_vec())
                    .with_entry("fan1_input", SimulatedHwmonEntry::PermissionDenied),
            ),
        ),
        "duplicate-source" => Some(
            SimulatedHwmonTree::new("legion-cli-duplicate")
                .with_device(
                    "simulated-a",
                    SimulatedHwmonDevice::generic(b"acpi_fan\n".to_vec()).with_entry(
                        "fan1_input",
                        SimulatedHwmonEntry::Regular(b"1900\n".to_vec()),
                    ),
                )
                .with_device(
                    "simulated-b",
                    SimulatedHwmonDevice::generic(b"acpi_fan\n".to_vec()).with_entry(
                        "fan1_input",
                        SimulatedHwmonEntry::Regular(b"1900\n".to_vec()),
                    ),
                ),
        ),
        "device-disappeared" => Some(
            SimulatedHwmonTree::new("legion-cli-disappeared").with_device(
                "simulated-acpi-fan",
                SimulatedHwmonDevice::generic(b"acpi_fan\n".to_vec())
                    .with_entry("fan1_input", SimulatedHwmonEntry::DeviceDisappeared),
            ),
        ),
        "missing-device" => Some(SimulatedHwmonTree::new("legion-cli-empty")),
        "invalid-rpm" => Some(SimulatedHwmonTree::new("legion-cli-invalid").with_device(
            "simulated-acpi-fan",
            SimulatedHwmonDevice::generic(b"acpi_fan\n".to_vec()).with_entry(
                "fan1_input",
                SimulatedHwmonEntry::Regular(b"not-a-number\n".to_vec()),
            ),
        )),
        _ => None,
    }
}

fn telemetry(scenario: &str, format: &str) -> Result<String, String> {
    let scenario = legion_fixtures::telemetry_scenario(scenario)
        .ok_or_else(|| format!("unknown simulated telemetry scenario: {scenario}"))?;
    let snapshot =
        legion_fixtures::simulated_telemetry(scenario).map_err(|error| error.to_string())?;
    match format {
        "json" => json(&snapshot),
        "text" => Ok(text_telemetry(&snapshot)),
        _ => unreachable!(),
    }
}

fn platform_profile(scenario: &str, format: &str) -> Result<String, String> {
    let scenario = legion_fixtures::platform_profile_scenario(scenario)
        .ok_or_else(|| format!("unknown simulated platform-profile scenario: {scenario}"))?;
    let raw = legion_fixtures::simulated_platform_profile_raw(scenario);
    let reader = LinuxPlatformProfileReader::new(
        SimulatedPlatformProfileTree::new("legion-cli-embedded-scenario")
            .with_entry(
                PlatformProfileEntry::Active,
                simulated_platform_profile_entry(raw.active),
            )
            .with_entry(
                PlatformProfileEntry::Choices,
                simulated_platform_profile_entry(raw.choices),
            ),
        4096,
    );
    let reading = PlatformProfileQuery::read_once(&reader)
        .map_err(|error| format!("simulated platform-profile query failed: {error}"))?;
    match format {
        "json" => json(reading.profile()),
        "text" => Ok(text_platform_profile(reading.profile())),
        _ => unreachable!(),
    }
}

/// The only CLI platform-profile input is a fixture value already held in
/// memory. An absent fixture value stays absent; it is never replaced with an
/// empty string or a host read.
fn simulated_platform_profile_entry(value: Option<String>) -> SimulatedEntry {
    match value {
        Some(value) => SimulatedEntry::Regular(value.into_bytes()),
        None => SimulatedEntry::Absent,
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

fn text_telemetry(snapshot: &TelemetrySnapshot) -> String {
    let active_profile = snapshot
        .profiles
        .active()
        .map(|profile| profile.name.as_str())
        .unwrap_or("unavailable");
    let temperatures = snapshot
        .temperatures
        .iter()
        .map(|reading| format!("- {}: {}", reading.sensor, text_value(&reading.reading)))
        .collect::<Vec<_>>()
        .join("\n");
    let frequencies = snapshot
        .frequencies
        .iter()
        .map(|reading| format!("- {}: {}", reading.component, text_value(&reading.reading)))
        .collect::<Vec<_>>()
        .join("\n");
    let power = snapshot
        .power
        .iter()
        .map(|reading| format!("- {}: {}", reading.component, text_value(&reading.reading)))
        .collect::<Vec<_>>()
        .join("\n");
    let fans = snapshot
        .fans
        .iter()
        .map(|fan| {
            let zero_note = (fan.reported_speed.value == Some(0)
                && fan.interpretation == legion_core::FanInterpretation::NonConclusive)
                .then_some("; zero RPM does not establish a stopped fan")
                .unwrap_or("");
            format!(
                "- {}: {} [validation={}; interpretation={:?}{}]",
                fan.fan,
                text_value(&fan.reported_speed),
                match fan.validation {
                    legion_core::FanValidation::Validated => "validated",
                    legion_core::FanValidation::Unvalidated => "unvalidated",
                },
                fan.interpretation,
                zero_note
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "LegionLinuxToolkit — simulated telemetry only\nModel: {} ({})\nActive profile: {}\nTemperatures:\n{}\nFrequencies:\n{}\nPower:\n{}\nFans:\n{}\nNo host sensors, firmware, or profiles were accessed or changed.",
        snapshot.identity.model,
        snapshot.identity.machine_type,
        active_profile,
        temperatures,
        frequencies,
        power,
        fans,
    )
}

fn text_platform_profile(reading: &PlatformProfileReading) -> String {
    let raw_active = reading.raw.active.as_deref().unwrap_or("<absent>");
    let raw_choices = reading.raw.choices.as_deref().unwrap_or("<absent>");
    let active = reading
        .normalized
        .active
        .as_ref()
        .map(|profile| format!("{} ({:?})", profile.name, profile.vocabulary))
        .unwrap_or_else(|| "<absent>".to_owned());
    let choices = reading
        .normalized
        .available
        .as_ref()
        .map(|profiles| {
            profiles
                .iter()
                .map(|profile| format!("{} ({:?})", profile.name, profile.vocabulary))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "<absent>".to_owned());
    format!(
        "Platform-profile simulation only\nRaw active: {raw_active:?}\nRaw choices: {raw_choices:?}\nNormalized active: {active}\nNormalized choices: {choices}\nState: {}\nProvenance: {} ({})\nNo host path, firmware, or energy profile was accessed or changed.",
        reading.state.as_str(),
        reading.provenance.source,
        reading.provenance.confidence.as_str(),
    )
}

fn text_fan_rpm(readings: &[legion_linux::HwmonFanReading]) -> String {
    let rendered = readings
        .iter()
        .map(|reading| {
            let zero_note = (reading.rpm.value == Some(0)
                && reading.interpretation == legion_core::FanInterpretation::NonConclusive)
                .then_some("; zero does not establish a stopped fan")
                .unwrap_or("");
            format!(
                "- driver={} channel=fan{}_input value={} RPM [source={:?}; module={:?}; validation={:?}; interpretation={:?}; provenance={} ({}){}]",
                reading.driver_name,
                reading.channel,
                reading.rpm.value.map_or_else(|| "<none>".to_owned(), |value| value.to_string()),
                reading.source_classification,
                reading.module_provenance,
                reading.validation,
                reading.interpretation,
                reading.rpm.provenance.source,
                reading.rpm.provenance.confidence.as_str(),
                zero_note,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Fan RPM diagnostic — simulated hwmon only\n{rendered}\nChannels are not assigned to CPU or GPU. No host hwmon path, firmware, or fan control was accessed."
    )
}

fn text_value<T: std::fmt::Display>(reading: &TelemetryValue<T>) -> String {
    let value = reading
        .value
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "no numeric value".to_owned());
    format!(
        "{} {} [{}; {}; confidence={}; static simulation]",
        value,
        reading.unit.as_str(),
        reading.classification.as_str(),
        reading.state.as_str(),
        reading.provenance.confidence.as_str(),
    )
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

    #[test]
    fn renders_simulated_telemetry_with_explicit_units_and_provenance() {
        let output = run(vec!["telemetry".into(), "--format".into(), "json".into()]).unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(decoded["source"], "simulated");
        assert_eq!(decoded["data"]["artifact"], "static-simulation");
        assert_eq!(decoded["data"]["temperatures"][0]["reading"]["unit"], "°C");
        assert_eq!(
            decoded["data"]["temperatures"][0]["reading"]["provenance"]["artifact"],
            "static-simulation"
        );
        assert_eq!(
            decoded["data"]["temperatures"][0]["reading"]["provenance"]["source"],
            "legion-fixtures::base_telemetry"
        );
        assert_eq!(
            decoded["data"]["temperatures"][0]["reading"]["provenance"]["confidence"],
            "low"
        );
        assert_eq!(
            decoded["data"]["power"][0]["reading"]["classification"],
            "configured-limit"
        );
    }

    #[test]
    fn displays_a_zero_rpm_as_non_conclusive() {
        let output = run(vec!["telemetry".into()]).unwrap();
        assert!(output.contains("0 RPM"));
        assert!(output.contains("NonConclusive"));
        assert!(output.contains("does not establish a stopped fan"));
    }

    #[test]
    fn reports_telemetry_inconsistencies_and_rejects_host_path_names() {
        let error = run(vec![
            "telemetry".into(),
            "--scenario".into(),
            "inconsistent".into(),
        ])
        .unwrap_err();
        assert!(error.contains("inconsistent simulated telemetry"));

        let path = run(vec!["telemetry".into(), "--scenario".into(), "/sys".into()]).unwrap_err();
        assert!(path.contains("unknown simulated telemetry scenario"));
    }

    #[test]
    fn reports_only_simulated_hwmon_quirk_data() {
        let output = run(vec![
            "fan-rpm".into(),
            "--scenario".into(),
            "quirk-two-channel".into(),
            "--format".into(),
            "json".into(),
        ])
        .unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(decoded["source"], "simulated");
        assert_eq!(decoded["data"].as_array().unwrap().len(), 2);
        assert_eq!(decoded["data"][0]["rpm"]["value"], 1900);
        assert_eq!(
            decoded["data"][0]["rpm"]["provenance"]["artifact"],
            "static-simulation"
        );
        assert_eq!(
            decoded["data"][0]["source_classification"],
            "validated-83ex-quirk-simulation"
        );
    }

    #[test]
    fn does_not_promote_generic_yogafan_zero_or_paths() {
        let output = run(vec![
            "fan-rpm".into(),
            "--scenario".into(),
            "zero-generic".into(),
        ])
        .unwrap();
        assert!(output.contains("zero does not establish a stopped fan"));
        assert!(output.contains("GenericUnvalidated"));

        let path = run(vec![
            "fan-rpm".into(),
            "--scenario".into(),
            "/sys/class/hwmon".into(),
        ])
        .unwrap_err();
        assert!(path.contains("unknown simulated fan-rpm scenario"));
    }

    #[test]
    fn renders_raw_and_normalized_simulated_platform_profile_as_json() {
        let output = run(vec![
            "platform-profile".into(),
            "--format".into(),
            "json".into(),
        ])
        .unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(decoded["source"], "simulated");
        assert_eq!(decoded["data"]["raw"]["active"], "balanced\n");
        assert_eq!(decoded["data"]["normalized"]["active"]["name"], "balanced");
        assert_eq!(
            decoded["data"]["provenance"]["artifact"],
            "static-simulation"
        );
        assert_eq!(
            decoded["data"]["provenance"]["source"],
            "legion-linux::simulated-root:legion-cli-embedded-scenario"
        );
    }

    #[test]
    fn platform_profile_scenarios_use_the_shared_simulated_query() {
        for scenario in ["valid", "active-only", "missing-active", "unknown"] {
            let output = run(vec![
                "platform-profile".into(),
                "--scenario".into(),
                scenario.into(),
                "--format".into(),
                "json".into(),
            ])
            .unwrap();
            let decoded: serde_json::Value = serde_json::from_str(&output).unwrap();
            assert_eq!(decoded["source"], "simulated");
            assert_eq!(
                decoded["data"]["provenance"]["artifact"],
                "static-simulation"
            );
        }

        for scenario in [
            "empty-active",
            "empty-choices",
            "duplicate",
            "inconsistent",
            "malformed",
        ] {
            let error = run(vec![
                "platform-profile".into(),
                "--scenario".into(),
                scenario.into(),
            ])
            .unwrap_err();
            assert!(error.starts_with("simulated platform-profile query failed:"));
        }
    }

    #[test]
    fn renders_absent_and_rejects_invalid_platform_profile_scenarios() {
        let absent = run(vec![
            "platform-profile".into(),
            "--scenario".into(),
            "missing-active".into(),
        ])
        .unwrap();
        assert!(absent.contains("State: unavailable"));

        let invalid = run(vec![
            "platform-profile".into(),
            "--scenario".into(),
            "duplicate".into(),
        ])
        .unwrap_err();
        assert!(invalid.contains("duplicate simulated platform-profile choice"));

        let path = run(vec![
            "platform-profile".into(),
            "--scenario".into(),
            "/sys".into(),
        ])
        .unwrap_err();
        assert!(path.contains("unknown simulated platform-profile scenario"));
    }
}
