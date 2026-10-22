//! Planning-only entry point for the experimental fan WMI read.
//!
//! It never opens a host interface, instantiates the transport, or invokes
//! firmware. The feature gate exists solely to make the future activation
//! surface explicit and auditable.

use legion_linux::fan_wmi_transport::documented_83ex_fan_wmi_read_plan;

const CONFIRMATION: &str = "I-UNDERSTAND-FAN-WMI-IS-BLOCKED";

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.as_slice() == ["--confirm", CONFIRMATION, "--format", "json"] {
        let plan = documented_83ex_fan_wmi_read_plan();
        println!(
            "{}",
            serde_json::to_string_pretty(&plan).expect("static plan serializes")
        );
        return;
    }

    eprintln!(
        "This planning-only binary cannot access WMI, ACPI, EC, sysfs, or firmware. \
         It accepts only: --confirm {CONFIRMATION} --format json"
    );
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_is_a_fixed_literal_not_a_host_access_switch() {
        assert_eq!(CONFIRMATION, "I-UNDERSTAND-FAN-WMI-IS-BLOCKED");
    }

    #[test]
    fn plan_output_is_explicitly_static_and_blocked() {
        let json = serde_json::to_string(&documented_83ex_fan_wmi_read_plan()).unwrap();
        assert!(json.contains("static-simulation"));
        assert!(json.contains("blocked"));
        assert!(!json.contains("linux-read-only"));
    }
}
