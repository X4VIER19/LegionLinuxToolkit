//! Linux-facing platform-profile architecture with no host filesystem access.
//!
//! This crate deliberately accepts bytes from an injected source. Phase 5B.3
//! includes only the virtual, in-memory source below: it has no filesystem
//! reader, no configurable path, no fallback to `/sys`, and no write API.

use legion_core::{
    ConfidenceLevel, PlatformProfileAdapter, PlatformProfileError, PlatformProfileInput,
    PlatformProfileRawInput, PlatformProfileReading, TelemetryArtifact, TelemetryProvenance,
};
use std::{collections::BTreeMap, fmt};

/// The only two entries this adapter is allowed to request. Their relative
/// names are fixed in code, so callers cannot supply a path or traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlatformProfileEntry {
    Active,
    Choices,
}

impl PlatformProfileEntry {
    pub const fn relative_name(self) -> &'static str {
        match self {
            Self::Active => "platform_profile",
            Self::Choices => "platform_profile_choices",
        }
    }
}

/// Acquisition class kept outside the parsed domain object. No provider in
/// this phase can construct `LinuxReadOnlyReserved`; it is a reserved future
/// label, represented now so it cannot be confused with a fixture later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxPlatformProfileOrigin {
    StaticSimulation { root_label: String },
    LinuxReadOnlyReserved { interface: &'static str },
}

/// Parsed data plus its acquisition class. The embedded core provenance is
/// checked against this origin before the result is published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxPlatformProfileReading {
    pub profile: PlatformProfileReading,
    pub origin: LinuxPlatformProfileOrigin,
}

/// Explicit source errors. A later filesystem provider must convert its I/O
/// failures into these states; this phase has no such provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformProfileSourceError {
    Missing {
        entry: PlatformProfileEntry,
    },
    PermissionDenied {
        entry: PlatformProfileEntry,
    },
    InvalidUtf8 {
        entry: PlatformProfileEntry,
    },
    PartialRead {
        entry: PlatformProfileEntry,
        bytes_read: usize,
    },
    TooLarge {
        entry: PlatformProfileEntry,
        limit: usize,
    },
    UnsafeEntry {
        entry: PlatformProfileEntry,
        reason: String,
    },
    ReadFailed {
        entry: PlatformProfileEntry,
        reason: String,
    },
}

impl fmt::Display for PlatformProfileSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { entry } => {
                write!(f, "missing simulated entry {}", entry.relative_name())
            }
            Self::PermissionDenied { entry } => {
                write!(
                    f,
                    "permission denied for simulated entry {}",
                    entry.relative_name()
                )
            }
            Self::InvalidUtf8 { entry } => {
                write!(
                    f,
                    "invalid UTF-8 in simulated entry {}",
                    entry.relative_name()
                )
            }
            Self::PartialRead { entry, bytes_read } => write!(
                f,
                "partial simulated read of {} ({bytes_read} bytes)",
                entry.relative_name()
            ),
            Self::TooLarge { entry, limit } => write!(
                f,
                "simulated entry {} exceeds maximum size of {limit} bytes",
                entry.relative_name()
            ),
            Self::UnsafeEntry { entry, reason } => write!(
                f,
                "unsafe simulated entry {}: {reason}",
                entry.relative_name()
            ),
            Self::ReadFailed { entry, reason } => {
                write!(
                    f,
                    "failed to read simulated entry {}: {reason}",
                    entry.relative_name()
                )
            }
        }
    }
}

impl std::error::Error for PlatformProfileSourceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxPlatformProfileError {
    Source(PlatformProfileSourceError),
    Parse(PlatformProfileError),
    ProvenanceMismatch,
}

impl fmt::Display for LinuxPlatformProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(f),
            Self::Parse(error) => error.fmt(f),
            Self::ProvenanceMismatch => {
                write!(f, "platform-profile origin and provenance do not agree")
            }
        }
    }
}

impl std::error::Error for LinuxPlatformProfileError {}

impl From<PlatformProfileSourceError> for LinuxPlatformProfileError {
    fn from(error: PlatformProfileSourceError) -> Self {
        Self::Source(error)
    }
}

impl From<PlatformProfileError> for LinuxPlatformProfileError {
    fn from(error: PlatformProfileError) -> Self {
        Self::Parse(error)
    }
}

/// A provider must declare whether it completed the requested read. A partial
/// result is never parsed, even if its bytes happen to resemble a valid value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformProfileSourceRead {
    Complete(Vec<u8>),
    Partial { bytes_read: usize },
}

/// Supplies bytes for fixed entries. This trait deliberately has no path
/// argument. Implementations must enforce their own read-size cap before they
/// allocate or return bytes. It is sealed so no external crate can introduce a
/// host-reading provider during this simulated-only phase.
pub trait PlatformProfileByteSource: private::Sealed {
    fn read(
        &self,
        entry: PlatformProfileEntry,
        maximum_size: usize,
    ) -> Result<PlatformProfileSourceRead, PlatformProfileSourceError>;

    /// Identifies a virtual test root; this is deliberately not a filesystem
    /// path and the trait cannot declare a real-host origin in this phase.
    fn simulated_root_label(&self) -> &str;
}

mod private {
    pub trait Sealed {}
}

/// Read-only orchestration over an injected source. It always routes decoded
/// text through `PlatformProfileAdapter`; no result bypasses its validation.
#[derive(Debug, Clone)]
pub struct LinuxPlatformProfileReader<S> {
    source: S,
    maximum_size: usize,
}

impl<S> LinuxPlatformProfileReader<S>
where
    S: PlatformProfileByteSource,
{
    pub fn new(source: S, maximum_size: usize) -> Self {
        Self {
            source,
            maximum_size,
        }
    }

    pub fn read(&self) -> Result<LinuxPlatformProfileReading, LinuxPlatformProfileError> {
        let active = self.read_text(PlatformProfileEntry::Active)?;
        let choices = self.read_text(PlatformProfileEntry::Choices)?;
        let origin = LinuxPlatformProfileOrigin::StaticSimulation {
            root_label: self.source.simulated_root_label().to_owned(),
        };
        let provenance = provenance_for(&origin);
        let profile = PlatformProfileAdapter.parse(PlatformProfileInput {
            raw: PlatformProfileRawInput {
                active: Some(active),
                choices: Some(choices),
            },
            provenance,
        })?;
        if !provenance_matches(&origin, profile.provenance.artifact) {
            return Err(LinuxPlatformProfileError::ProvenanceMismatch);
        }
        Ok(LinuxPlatformProfileReading { profile, origin })
    }

    fn read_text(&self, entry: PlatformProfileEntry) -> Result<String, LinuxPlatformProfileError> {
        let bytes = match self.source.read(entry, self.maximum_size)? {
            PlatformProfileSourceRead::Complete(bytes) => bytes,
            PlatformProfileSourceRead::Partial { bytes_read } => {
                return Err(PlatformProfileSourceError::PartialRead { entry, bytes_read }.into());
            }
        };
        if bytes.len() > self.maximum_size {
            return Err(PlatformProfileSourceError::TooLarge {
                entry,
                limit: self.maximum_size,
            }
            .into());
        }
        String::from_utf8(bytes)
            .map_err(|_| PlatformProfileSourceError::InvalidUtf8 { entry }.into())
    }
}

fn provenance_for(origin: &LinuxPlatformProfileOrigin) -> TelemetryProvenance {
    match origin {
        LinuxPlatformProfileOrigin::StaticSimulation { root_label } => TelemetryProvenance {
            artifact: TelemetryArtifact::StaticSimulation,
            source: format!("legion-linux::simulated-root:{root_label}"),
            confidence: ConfidenceLevel::Low,
        },
        LinuxPlatformProfileOrigin::LinuxReadOnlyReserved { interface } => TelemetryProvenance {
            artifact: TelemetryArtifact::LinuxReadOnly,
            source: format!("legion-linux::reserved:{interface}"),
            confidence: ConfidenceLevel::Low,
        },
    }
}

fn provenance_matches(origin: &LinuxPlatformProfileOrigin, artifact: TelemetryArtifact) -> bool {
    matches!(
        (origin, artifact),
        (
            LinuxPlatformProfileOrigin::StaticSimulation { .. },
            TelemetryArtifact::StaticSimulation
        ) | (
            LinuxPlatformProfileOrigin::LinuxReadOnlyReserved { .. },
            TelemetryArtifact::LinuxReadOnly
        )
    )
}

/// A fully virtual fixture tree. Its label is diagnostic data, never a host
/// path. It models unsafe objects explicitly so tests can prove that symlinks
/// and path escapes are rejected without following them.
#[derive(Debug, Clone)]
pub struct SimulatedPlatformProfileTree {
    root_label: String,
    entries: BTreeMap<PlatformProfileEntry, SimulatedEntry>,
}

#[derive(Debug, Clone)]
pub enum SimulatedEntry {
    Regular(Vec<u8>),
    Partial { bytes_read: usize },
    Missing,
    PermissionDenied,
    Symlink { target: String },
    ReadFailure { reason: String },
}

impl SimulatedPlatformProfileTree {
    pub fn new(root_label: impl Into<String>) -> Self {
        Self {
            root_label: root_label.into(),
            entries: BTreeMap::new(),
        }
    }

    pub fn with_entry(mut self, entry: PlatformProfileEntry, value: SimulatedEntry) -> Self {
        self.entries.insert(entry, value);
        self
    }
}

impl PlatformProfileByteSource for SimulatedPlatformProfileTree {
    fn read(
        &self,
        entry: PlatformProfileEntry,
        maximum_size: usize,
    ) -> Result<PlatformProfileSourceRead, PlatformProfileSourceError> {
        match self.entries.get(&entry) {
            None | Some(SimulatedEntry::Missing) => {
                Err(PlatformProfileSourceError::Missing { entry })
            }
            Some(SimulatedEntry::PermissionDenied) => {
                Err(PlatformProfileSourceError::PermissionDenied { entry })
            }
            Some(SimulatedEntry::Symlink { target }) => {
                Err(PlatformProfileSourceError::UnsafeEntry {
                    entry,
                    reason: format!("symbolic links are refused (target {target:?})"),
                })
            }
            Some(SimulatedEntry::ReadFailure { reason }) => {
                Err(PlatformProfileSourceError::ReadFailed {
                    entry,
                    reason: reason.clone(),
                })
            }
            Some(SimulatedEntry::Partial { bytes_read }) => {
                Ok(PlatformProfileSourceRead::Partial {
                    bytes_read: *bytes_read,
                })
            }
            Some(SimulatedEntry::Regular(bytes)) if bytes.len() > maximum_size => {
                Err(PlatformProfileSourceError::TooLarge {
                    entry,
                    limit: maximum_size,
                })
            }
            Some(SimulatedEntry::Regular(bytes)) => {
                Ok(PlatformProfileSourceRead::Complete(bytes.clone()))
            }
        }
    }

    fn simulated_root_label(&self) -> &str {
        &self.root_label
    }
}

impl private::Sealed for SimulatedPlatformProfileTree {}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_core::{PlatformProfileVocabulary, TelemetryState};

    const MAXIMUM_SIZE: usize = 128;

    fn tree(active: SimulatedEntry, choices: SimulatedEntry) -> SimulatedPlatformProfileTree {
        SimulatedPlatformProfileTree::new("temporary-test-tree")
            .with_entry(PlatformProfileEntry::Active, active)
            .with_entry(PlatformProfileEntry::Choices, choices)
    }

    fn regular(value: &str) -> SimulatedEntry {
        SimulatedEntry::Regular(value.as_bytes().to_vec())
    }

    #[test]
    fn reads_only_fixed_entries_from_a_simulated_tree() {
        let reader = LinuxPlatformProfileReader::new(
            tree(
                regular("performance\n"),
                regular("low-power balanced performance max-power custom\n"),
            ),
            MAXIMUM_SIZE,
        );
        let result = reader.read().unwrap();
        assert_eq!(
            result.profile.normalized.active.unwrap().name,
            "performance"
        );
        assert_eq!(result.profile.state, TelemetryState::Available);
        assert_eq!(
            result.profile.provenance.artifact,
            TelemetryArtifact::StaticSimulation
        );
        assert!(matches!(
            result.origin,
            LinuxPlatformProfileOrigin::StaticSimulation { .. }
        ));
    }

    #[test]
    fn reports_missing_active_and_choices_explicitly() {
        let missing_active = LinuxPlatformProfileReader::new(
            tree(SimulatedEntry::Missing, regular("performance")),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            missing_active.read(),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::Missing {
                    entry: PlatformProfileEntry::Active
                }
            ))
        ));

        let missing_choices = LinuxPlatformProfileReader::new(
            tree(regular("performance"), SimulatedEntry::Missing),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            missing_choices.read(),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::Missing {
                    entry: PlatformProfileEntry::Choices
                }
            ))
        ));
    }

    #[test]
    fn rejects_empty_malformed_and_inconsistent_content_via_core_parser() {
        let empty = LinuxPlatformProfileReader::new(
            tree(regular("\n"), regular("performance")),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            empty.read(),
            Err(LinuxPlatformProfileError::Parse(
                PlatformProfileError::Empty { field: "active" }
            ))
        ));

        let malformed = LinuxPlatformProfileReader::new(
            tree(regular("performance\u{001b}"), regular("performance")),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            malformed.read(),
            Err(LinuxPlatformProfileError::Parse(
                PlatformProfileError::Malformed {
                    field: "active",
                    ..
                }
            ))
        ));

        let inconsistent = LinuxPlatformProfileReader::new(
            tree(regular("performance"), regular("balanced")),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            inconsistent.read(),
            Err(LinuxPlatformProfileError::Parse(
                PlatformProfileError::ActiveNotAvailable { .. }
            ))
        ));
    }

    #[test]
    fn reports_permission_utf8_and_size_failures_without_a_host_read() {
        let denied = LinuxPlatformProfileReader::new(
            tree(SimulatedEntry::PermissionDenied, regular("performance")),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            denied.read(),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::PermissionDenied { .. }
            ))
        ));

        let invalid_utf8 = LinuxPlatformProfileReader::new(
            tree(SimulatedEntry::Regular(vec![0xff]), regular("performance")),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            invalid_utf8.read(),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::InvalidUtf8 { .. }
            ))
        ));

        let oversized = LinuxPlatformProfileReader::new(
            tree(
                SimulatedEntry::Regular(vec![b'p'; MAXIMUM_SIZE + 1]),
                regular("performance"),
            ),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            oversized.read(),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::TooLarge { .. }
            ))
        ));

        let read_failure = LinuxPlatformProfileReader::new(
            tree(
                SimulatedEntry::ReadFailure {
                    reason: "simulated I/O failure".to_owned(),
                },
                regular("performance"),
            ),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            read_failure.read(),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::ReadFailed { .. }
            ))
        ));

        let partial = LinuxPlatformProfileReader::new(
            tree(
                SimulatedEntry::Partial { bytes_read: 4 },
                regular("performance"),
            ),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            partial.read(),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::PartialRead {
                    entry: PlatformProfileEntry::Active,
                    bytes_read: 4,
                }
            ))
        ));
    }

    #[test]
    fn refuses_simulated_symlinks_and_cannot_accept_path_traversal() {
        let symlink = LinuxPlatformProfileReader::new(
            tree(
                SimulatedEntry::Symlink {
                    target: "../../sys/firmware/acpi/platform_profile".to_owned(),
                },
                regular("performance"),
            ),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            symlink.read(),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::UnsafeEntry {
                    entry: PlatformProfileEntry::Active,
                    ..
                }
            ))
        ));
        assert_eq!(
            PlatformProfileEntry::Active.relative_name(),
            "platform_profile"
        );
        assert_eq!(
            PlatformProfileEntry::Choices.relative_name(),
            "platform_profile_choices"
        );
    }

    #[test]
    fn unknown_profiles_remain_unvalidated_static_information() {
        let reader = LinuxPlatformProfileReader::new(
            tree(regular("vendor-turbo"), regular("performance vendor-turbo")),
            MAXIMUM_SIZE,
        );
        let result = reader.read().unwrap();
        assert_eq!(result.profile.state, TelemetryState::Unvalidated);
        assert_eq!(
            result.profile.normalized.active.unwrap().vocabulary,
            PlatformProfileVocabulary::Unknown
        );
        assert_eq!(
            result.profile.provenance.artifact,
            TelemetryArtifact::StaticSimulation
        );
    }

    #[test]
    fn reserved_linux_origin_has_a_distinct_future_provenance_label() {
        let provenance = provenance_for(&LinuxPlatformProfileOrigin::LinuxReadOnlyReserved {
            interface: "platform_profile",
        });
        assert_eq!(provenance.artifact, TelemetryArtifact::LinuxReadOnly);
        assert!(provenance.source.starts_with("legion-linux::reserved:"));
    }
}
