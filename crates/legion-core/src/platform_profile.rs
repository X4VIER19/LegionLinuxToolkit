//! Platform-profile parsing over already-provided text.
//!
//! This module is deliberately independent of sysfs and the host filesystem.
//! A future Linux-specific layer may supply text to this adapter only after a
//! separate review; this module never selects or opens a path.

use crate::{TelemetryProvenance, TelemetryState};
use serde::Serialize;
use std::{collections::BTreeSet, fmt};

/// Exact text supplied by a caller. `None` is distinct from an empty string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlatformProfileRawInput {
    pub active: Option<String>,
    pub choices: Option<String>,
}

/// A normalised profile name while retaining whether its vocabulary is known
/// to this static parser. Unknown, well-formed names remain data rather than
/// being silently discarded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NormalizedPlatformProfile {
    pub name: String,
    pub vocabulary: PlatformProfileVocabulary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlatformProfileVocabulary {
    Known,
    Unknown,
}

/// Parsed representation. `available` stays absent when no choices text was
/// supplied, so callers cannot mistake an omitted list for an empty list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NormalizedPlatformProfiles {
    pub active: Option<NormalizedPlatformProfile>,
    pub available: Option<Vec<NormalizedPlatformProfile>>,
}

/// Input given to the adapter. It intentionally contains text already held in
/// memory; it has no path or reader field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformProfileInput {
    pub raw: PlatformProfileRawInput,
    pub provenance: TelemetryProvenance,
}

/// A read-only, host-independent result. Its provenance must identify the
/// origin of the supplied text; phase 5B.2 fixtures use `static-simulation`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlatformProfileReading {
    pub raw: PlatformProfileRawInput,
    pub normalized: NormalizedPlatformProfiles,
    pub state: TelemetryState,
    pub provenance: TelemetryProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformProfileError {
    Empty { field: &'static str },
    Malformed { field: &'static str, value: String },
    Duplicate { profile: String },
    ActiveNotAvailable { active: String },
}

impl fmt::Display for PlatformProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { field } => write!(f, "empty simulated platform-profile {field}"),
            Self::Malformed { field, value } => {
                write!(f, "malformed simulated platform-profile {field}: {value:?}")
            }
            Self::Duplicate { profile } => {
                write!(f, "duplicate simulated platform-profile choice: {profile}")
            }
            Self::ActiveNotAvailable { active } => write!(
                f,
                "inconsistent simulated platform-profile: active profile {active:?} is absent from choices"
            ),
        }
    }
}

impl std::error::Error for PlatformProfileError {}

/// Stateless adapter for a documented text format. It parses only text passed
/// in by its caller and therefore cannot reach real hardware as a fallback.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformProfileAdapter;

impl PlatformProfileAdapter {
    pub fn parse(
        &self,
        input: PlatformProfileInput,
    ) -> Result<PlatformProfileReading, PlatformProfileError> {
        let active = match &input.raw.active {
            Some(value) => Some(parse_one("active", value)?),
            None => None,
        };
        let available = match &input.raw.choices {
            Some(value) => Some(parse_choices(value)?),
            None => None,
        };

        if let (Some(active), Some(available)) = (&active, &available)
            && !available.iter().any(|profile| profile.name == active.name)
        {
            return Err(PlatformProfileError::ActiveNotAvailable {
                active: active.name.clone(),
            });
        }

        let has_unknown_vocabulary = active
            .iter()
            .chain(available.iter().flatten())
            .any(|profile| profile.vocabulary == PlatformProfileVocabulary::Unknown);
        let state = match active {
            None => TelemetryState::Unavailable,
            Some(_) if has_unknown_vocabulary => TelemetryState::Unvalidated,
            Some(_) => TelemetryState::Available,
        };

        Ok(PlatformProfileReading {
            raw: input.raw,
            normalized: NormalizedPlatformProfiles { active, available },
            state,
            provenance: input.provenance,
        })
    }
}

fn parse_one(
    field: &'static str,
    raw: &str,
) -> Result<NormalizedPlatformProfile, PlatformProfileError> {
    reject_unexpected_controls(field, raw)?;
    let normalized = raw.trim_matches(is_allowed_separator);
    if normalized.is_empty() {
        return Err(PlatformProfileError::Empty { field });
    }
    if normalized.chars().any(is_allowed_separator) || !is_well_formed_name(normalized) {
        return Err(PlatformProfileError::Malformed {
            field,
            value: raw.to_owned(),
        });
    }
    Ok(profile(normalized))
}

fn parse_choices(raw: &str) -> Result<Vec<NormalizedPlatformProfile>, PlatformProfileError> {
    reject_unexpected_controls("choices", raw)?;
    if raw.trim_matches(is_allowed_separator).is_empty() {
        return Err(PlatformProfileError::Empty { field: "choices" });
    }
    let mut names = BTreeSet::new();
    let mut profiles = Vec::new();
    for token in raw
        .split(is_allowed_separator)
        .filter(|token| !token.is_empty())
    {
        if !is_well_formed_name(token) {
            return Err(PlatformProfileError::Malformed {
                field: "choices",
                value: token.to_owned(),
            });
        }
        if !names.insert(token) {
            return Err(PlatformProfileError::Duplicate {
                profile: token.to_owned(),
            });
        }
        profiles.push(profile(token));
    }
    Ok(profiles)
}

fn reject_unexpected_controls(field: &'static str, raw: &str) -> Result<(), PlatformProfileError> {
    if raw
        .chars()
        .any(|character| character.is_control() && !is_allowed_separator(character))
    {
        return Err(PlatformProfileError::Malformed {
            field,
            value: raw.to_owned(),
        });
    }
    Ok(())
}

fn is_allowed_separator(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\r')
}

fn is_well_formed_name(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
}

fn profile(name: &str) -> NormalizedPlatformProfile {
    let vocabulary = match name {
        // Standard vocabulary plus `max-power`, documented as an observation
        // for the initial target. This list is informational, not a capability
        // allowlist and does not authorise control operations.
        "low-power"
        | "balanced"
        | "balanced-performance"
        | "performance"
        | "cool"
        | "quiet"
        | "custom"
        | "max-power" => PlatformProfileVocabulary::Known,
        _ => PlatformProfileVocabulary::Unknown,
    };
    NormalizedPlatformProfile {
        name: name.to_owned(),
        vocabulary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConfidenceLevel, TelemetryArtifact};

    fn input(active: Option<&str>, choices: Option<&str>) -> PlatformProfileInput {
        PlatformProfileInput {
            raw: PlatformProfileRawInput {
                active: active.map(str::to_owned),
                choices: choices.map(str::to_owned),
            },
            provenance: TelemetryProvenance {
                artifact: TelemetryArtifact::StaticSimulation,
                source: "inline-test".to_owned(),
                confidence: ConfidenceLevel::Low,
            },
        }
    }

    #[test]
    fn preserves_raw_text_and_normalizes_valid_profiles() {
        let reading = PlatformProfileAdapter
            .parse(input(
                Some("balanced\n"),
                Some("low-power balanced performance custom\n"),
            ))
            .unwrap();
        assert_eq!(reading.raw.active.as_deref(), Some("balanced\n"));
        assert_eq!(reading.normalized.active.unwrap().name, "balanced");
        assert_eq!(reading.normalized.available.unwrap().len(), 4);
        assert_eq!(reading.state, TelemetryState::Available);
    }

    #[test]
    fn keeps_absence_distinct_from_empty_and_unknown() {
        let absent = PlatformProfileAdapter
            .parse(input(None, Some("balanced performance")))
            .unwrap();
        assert_eq!(absent.state, TelemetryState::Unavailable);
        assert_eq!(absent.normalized.active, None);
        assert!(matches!(
            PlatformProfileAdapter.parse(input(Some(""), None)),
            Err(PlatformProfileError::Empty { field: "active" })
        ));

        let unknown = PlatformProfileAdapter
            .parse(input(Some("vendor-turbo"), Some("balanced vendor-turbo")))
            .unwrap();
        assert_eq!(unknown.state, TelemetryState::Unvalidated);
        assert_eq!(
            unknown.normalized.active.unwrap().vocabulary,
            PlatformProfileVocabulary::Unknown
        );
    }

    #[test]
    fn rejects_duplicates_inconsistent_and_malformed_text() {
        assert!(matches!(
            PlatformProfileAdapter.parse(input(Some("balanced"), Some("balanced balanced"))),
            Err(PlatformProfileError::Duplicate { .. })
        ));
        assert!(matches!(
            PlatformProfileAdapter.parse(input(Some("balanced"), Some("performance"))),
            Err(PlatformProfileError::ActiveNotAvailable { .. })
        ));
        assert!(matches!(
            PlatformProfileAdapter.parse(input(Some("balanced profile"), None)),
            Err(PlatformProfileError::Malformed {
                field: "active",
                ..
            })
        ));
    }

    #[test]
    fn rejects_unexpected_control_characters_and_empty_choices() {
        assert!(matches!(
            PlatformProfileAdapter.parse(input(Some("balanced\u{001b}"), None)),
            Err(PlatformProfileError::Malformed {
                field: "active",
                ..
            })
        ));
        assert!(matches!(
            PlatformProfileAdapter
                .parse(input(Some("balanced"), Some("balanced\u{000b}performance"),)),
            Err(PlatformProfileError::Malformed {
                field: "choices",
                ..
            })
        ));
        assert!(matches!(
            PlatformProfileAdapter.parse(input(Some("balanced"), Some(" \t\n\r"))),
            Err(PlatformProfileError::Empty { field: "choices" })
        ));
    }
}
