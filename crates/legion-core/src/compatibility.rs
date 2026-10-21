use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceDescriptor {
    pub manufacturer: String,
    pub model: String,
    pub machine_type: String,
    pub bios_version: String,
    /// Optional because the reference DMI value has not been collected yet.
    pub dmi_identifier: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DeviceQuery {
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub machine_type: Option<String>,
    pub bios_version: Option<String>,
    pub dmi_identifier: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompatibilityStatus {
    Observed,
    Validated,
    Unvalidated,
    Unknown,
    NotCompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObservationPlatform {
    Linux,
    Windows,
    Documentation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKind {
    Documentation,
    Observation,
    Validation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub platform: ObservationPlatform,
    pub kind: EvidenceKind,
    pub source: String,
    pub summary: String,
    pub status: CompatibilityStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityRecord {
    pub key: String,
    pub status: CompatibilityStatus,
    pub evidence: Vec<Evidence>,
    /// This registry is descriptive only and never enables a hardware control.
    controls_enabled: bool,
}

impl CapabilityRecord {
    pub fn descriptive(
        key: impl Into<String>,
        status: CompatibilityStatus,
        evidence: Evidence,
    ) -> Self {
        Self {
            key: key.into(),
            status,
            evidence: vec![evidence],
            controls_enabled: false,
        }
    }

    pub fn controls_enabled(&self) -> bool {
        self.controls_enabled
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompatibilityProfile {
    pub id: String,
    pub identity: DeviceDescriptor,
    pub capabilities: Vec<CapabilityRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompatibilityRegistry {
    pub profiles: Vec<CompatibilityProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "result", rename_all = "kebab-case")]
pub enum MatchResult {
    Exact { profile: CompatibilityProfile },
    Unknown { reason: String },
    Ambiguous { candidate_ids: Vec<String> },
    Incomplete { missing_fields: Vec<String> },
}

impl CompatibilityRegistry {
    /// Performs only strict matching against caller-provided static data.
    /// It does not inspect the current computer or enable controls.
    pub fn match_query(&self, query: &DeviceQuery) -> MatchResult {
        let missing_fields = required_missing(query);
        if !missing_fields.is_empty() {
            return MatchResult::Incomplete { missing_fields };
        }

        let candidates = self
            .profiles
            .iter()
            .filter(|profile| profile_matches(profile, query))
            .cloned()
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [] => MatchResult::Unknown {
                reason: "no exact static compatibility profile".to_owned(),
            },
            [profile] => MatchResult::Exact {
                profile: profile.clone(),
            },
            _ => MatchResult::Ambiguous {
                candidate_ids: candidates.into_iter().map(|profile| profile.id).collect(),
            },
        }
    }
}

fn required_missing(query: &DeviceQuery) -> Vec<String> {
    [
        ("manufacturer", missing_or_blank(&query.manufacturer)),
        ("model", missing_or_blank(&query.model)),
        ("machine_type", missing_or_blank(&query.machine_type)),
        ("bios_version", missing_or_blank(&query.bios_version)),
    ]
    .into_iter()
    .filter_map(|(field, missing)| missing.then_some(field.to_owned()))
    .collect()
}

fn missing_or_blank(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(|value| value.trim().is_empty())
}

fn profile_matches(profile: &CompatibilityProfile, query: &DeviceQuery) -> bool {
    let identity = &profile.identity;
    query.manufacturer.as_deref() == Some(identity.manufacturer.as_str())
        && query.model.as_deref() == Some(identity.model.as_str())
        && query.machine_type.as_deref() == Some(identity.machine_type.as_str())
        && query.bios_version.as_deref() == Some(identity.bios_version.as_str())
        && identity.dmi_identifier.as_deref() == query.dmi_identifier.as_deref()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: &str, bios: &str) -> CompatibilityProfile {
        CompatibilityProfile {
            id: id.to_owned(),
            identity: DeviceDescriptor {
                manufacturer: "Lenovo".to_owned(),
                model: "Legion Slim 5 16ARP9".to_owned(),
                machine_type: "83EX".to_owned(),
                bios_version: bios.to_owned(),
                dmi_identifier: None,
            },
            capabilities: vec![],
        }
    }

    fn query() -> DeviceQuery {
        DeviceQuery {
            manufacturer: Some("Lenovo".to_owned()),
            model: Some("Legion Slim 5 16ARP9".to_owned()),
            machine_type: Some("83EX".to_owned()),
            bios_version: Some("NUCN15WW".to_owned()),
            dmi_identifier: None,
        }
    }

    #[test]
    fn matches_only_a_complete_exact_profile() {
        let registry = CompatibilityRegistry {
            profiles: vec![profile("83ex-nucn15ww", "NUCN15WW")],
        };
        assert!(matches!(
            registry.match_query(&query()),
            MatchResult::Exact { .. }
        ));
    }

    #[test]
    fn rejects_unknown_or_incomplete_queries() {
        let registry = CompatibilityRegistry {
            profiles: vec![profile("83ex-nucn15ww", "NUCN15WW")],
        };
        let mut unknown = query();
        unknown.bios_version = Some("OTHER".to_owned());
        assert!(matches!(
            registry.match_query(&unknown),
            MatchResult::Unknown { .. }
        ));
        let incomplete = DeviceQuery::default();
        assert!(matches!(
            registry.match_query(&incomplete),
            MatchResult::Incomplete { .. }
        ));
        let mut blank = query();
        blank.model = Some("  ".to_owned());
        assert!(matches!(
            registry.match_query(&blank),
            MatchResult::Incomplete { .. }
        ));
        let mut undocumented_dmi = query();
        undocumented_dmi.dmi_identifier = Some("unverified-dmi".to_owned());
        assert!(matches!(
            registry.match_query(&undocumented_dmi),
            MatchResult::Unknown { .. }
        ));
    }

    #[test]
    fn refuses_ambiguous_profiles() {
        let registry = CompatibilityRegistry {
            profiles: vec![profile("first", "NUCN15WW"), profile("second", "NUCN15WW")],
        };
        assert!(matches!(
            registry.match_query(&query()),
            MatchResult::Ambiguous { .. }
        ));
    }
}
