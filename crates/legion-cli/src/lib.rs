//! Shared, host-independent gates for experimental CLI entry points.
//!
//! This crate never discovers the current computer. The compatibility gate
//! below evaluates only a closed, embedded fixture record so a future caller
//! cannot turn missing host identity into a successful match.

use legion_core::{CompatibilityStatus, MatchResult};
use serde::Serialize;

/// Exact name for the only static compatibility declaration accepted by the
/// experimental real-read entry point. It is not a claim that the current host
/// was detected or verified.
pub const STATIC_83EX_NUCN15WW_COMPATIBILITY: &str = "83ex-nucn15ww-static";

/// Evidence passed to the feature-gated executable before it can attempt an
/// actual read. It describes embedded data only and deliberately records that
/// no DMI or host inspection occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StaticCompatibilityGate {
    profile_id: String,
    capability_status: CompatibilityStatus,
    source: &'static str,
    host_identity_verified: bool,
}

impl StaticCompatibilityGate {
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub fn capability_status(&self) -> CompatibilityStatus {
        self.capability_status
    }

    pub fn host_identity_verified(&self) -> bool {
        self.host_identity_verified
    }
}

/// Evaluates one closed fixture selector. Missing, unknown, ambiguous, or
/// incomplete data fail closed. This function cannot access DMI, sysfs,
/// environment variables, files, or firmware.
pub fn require_static_platform_profile_compatibility(
    selector: &str,
) -> Result<StaticCompatibilityGate, String> {
    if selector != STATIC_83EX_NUCN15WW_COMPATIBILITY {
        return Err("unknown static compatibility declaration".to_owned());
    }

    let query = legion_fixtures::compatibility_query("known")
        .ok_or("required static compatibility query is unavailable")?;
    let registry = legion_fixtures::legion_slim_5_16arp9_registry();
    let MatchResult::Exact { profile } = registry.match_query(&query) else {
        return Err("static compatibility declaration did not match exactly".to_owned());
    };
    if profile.id != "lenovo-legion-slim-5-16arp9-83ex-nucn15ww" {
        return Err("static compatibility declaration has an unexpected profile".to_owned());
    }
    let capability = profile
        .capabilities
        .iter()
        .find(|capability| capability.key == "platform-profile.read")
        .ok_or("static compatibility declaration lacks platform-profile.read")?;
    if !matches!(
        capability.status,
        CompatibilityStatus::Observed | CompatibilityStatus::Validated
    ) {
        return Err("static compatibility declaration does not permit observation".to_owned());
    }

    Ok(StaticCompatibilityGate {
        profile_id: profile.id,
        capability_status: capability.status,
        source: "embedded-static-compatibility",
        host_identity_verified: false,
    })
}

/// Blocks an experimental host read until a future, separately reviewed
/// verifier can establish the current machine identity. Static fixture
/// compatibility always fails this gate. There is no public constructor or
/// command-line override that can produce a verified gate.
pub fn require_verified_host_identity(gate: &StaticCompatibilityGate) -> Result<(), String> {
    if gate.host_identity_verified {
        Ok(())
    } else {
        Err("host identity is not verified; refusing experimental sysfs read".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_exact_embedded_compatibility_declaration() {
        let gate =
            require_static_platform_profile_compatibility(STATIC_83EX_NUCN15WW_COMPATIBILITY)
                .unwrap();
        assert_eq!(
            gate.profile_id(),
            "lenovo-legion-slim-5-16arp9-83ex-nucn15ww"
        );
        assert_eq!(gate.capability_status(), CompatibilityStatus::Observed);
        assert!(!gate.host_identity_verified());
    }

    #[test]
    fn refuses_missing_unknown_or_path_like_compatibility_declarations() {
        for selector in ["", "unknown", "/sys/firmware/acpi", "83EX"] {
            assert!(require_static_platform_profile_compatibility(selector).is_err());
        }
    }

    #[test]
    fn static_compatibility_cannot_authorize_a_host_read() {
        let gate =
            require_static_platform_profile_compatibility(STATIC_83EX_NUCN15WW_COMPATIBILITY)
                .unwrap();
        assert!(require_verified_host_identity(&gate).is_err());
    }
}
