//! Linux-facing platform-profile architecture with no default public host
//! filesystem access.
//!
//! This crate deliberately accepts bytes from an injected source. Phase 5B.3
//! includes the virtual, in-memory source below. It also contains a private,
//! production-shaped `openat2` provider behind an unconstructable activation
//! capability. There is no public filesystem reader, configurable path, host
//! root, fallback to `/sys`, write API, or CLI integration. Normal builds
//! cannot initiate a host read; tests use only temporary fixture trees. A
//! future real read is compiled only by the non-default, explicitly named
//! experimental feature and is not exercised in this phase.

use legion_core::{
    ConfidenceLevel, PlatformProfileAdapter, PlatformProfileError, PlatformProfileInput,
    PlatformProfileRawInput, PlatformProfileReading, TelemetryArtifact, TelemetryProvenance,
};
#[cfg(any(test, feature = "approved-real-platform-profile-read"))]
use rustix::{
    fs::{CWD, FileType, Mode, OFlags, ResolveFlags, fstat, openat2},
    io::{Errno, read},
};
use serde::Serialize;
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    fmt,
};

mod host_identity;
mod hwmon;
#[cfg(any(test, feature = "approved-real-hwmon-rpm-read"))]
mod hwmon_real;
mod offline_fan_response;

#[cfg(any(
    feature = "experimental-fan-wmi-transport",
    feature = "experimental-fan-wmi-read"
))]
pub mod fan_wmi_transport;
#[cfg(all(
    test,
    not(any(
        feature = "experimental-fan-wmi-transport",
        feature = "experimental-fan-wmi-read"
    ))
))]
mod fan_wmi_transport;

#[cfg(any(
    test,
    feature = "experimental-dmi-identity-read",
    feature = "approved-real-dmi-identity-read"
))]
mod dmi_identity;

pub use host_identity::{
    HostIdentityField, HostIdentitySource, HostIdentitySourceError, LinuxHostIdentityError,
    LinuxHostIdentityProvider, SimulatedLinuxHostIdentitySource,
};
pub use hwmon::{
    HwmonDriverClassification, HwmonFanError, HwmonFanQuery, HwmonFanReading,
    HwmonModuleProvenance, HwmonSourceError, LinuxHwmonFanReader, SimulatedHwmonDevice,
    SimulatedHwmonEntry, SimulatedHwmonTree,
};

#[cfg(feature = "approved-real-hwmon-rpm-read")]
pub use hwmon_real::{HwmonLinuxReading, HwmonRealReadError, approved_one_shot_hwmon_rpm_read};
pub use offline_fan_response::{
    OfflineFanResponseAnalysis, OfflineFanResponseError, OfflineFanResponseProvenance,
    analyze_offline_fan_response_file,
};

#[cfg(any(
    test,
    feature = "experimental-dmi-identity-read",
    feature = "approved-real-dmi-identity-read"
))]
pub use dmi_identity::{
    DmiAttributeEvidence, DmiIdentityError, DmiIdentityEvidence, DmiIdentityField,
    DmiIdentityReading,
};

#[cfg(feature = "approved-real-dmi-identity-read")]
pub use dmi_identity::approved_one_shot_dmi_identity_read;

#[cfg(any(test, feature = "approved-real-platform-profile-read"))]
use std::fs::File;

#[cfg(feature = "approved-real-platform-profile-read")]
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

#[cfg(test)]
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

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

/// Acquisition class kept outside the parsed domain object. The real label is
/// constructed only by the private, capability-gated provider after a complete
/// successful read; public fixtures can only produce `StaticSimulation`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum LinuxPlatformProfileOrigin {
    StaticSimulation { root_label: String },
    LinuxReadOnly { origin: LinuxReadOnlyOrigin },
}

/// Marker attached only by the private provider after it has completed its
/// confined, validated Linux read. Its field and constructor are private, so
/// downstream code cannot manufacture this marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinuxReadOnlyOrigin {
    interface: &'static str,
}

impl LinuxReadOnlyOrigin {
    #[cfg(any(test, feature = "approved-real-platform-profile-read"))]
    fn for_verified_read() -> Self {
        Self {
            interface: "sysfs-platform-profile",
        }
    }

    pub fn interface(&self) -> &'static str {
        self.interface
    }
}

/// Parsed data plus its acquisition class. The embedded core provenance is
/// checked against this origin before the result is published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinuxPlatformProfileReading {
    profile: PlatformProfileReading,
    origin: LinuxPlatformProfileOrigin,
}

impl LinuxPlatformProfileReading {
    /// Returns the validated domain reading. Consumers cannot replace it or
    /// pair it with a different acquisition origin.
    pub fn profile(&self) -> &PlatformProfileReading {
        &self.profile
    }

    /// Returns the origin assigned by the sealed byte source. This is exposed
    /// for inspection only; constructing a reading with a chosen origin is not
    /// public API.
    pub fn origin(&self) -> &LinuxPlatformProfileOrigin {
        &self.origin
    }
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
    Openat2Unavailable {
        entry: PlatformProfileEntry,
    },
    UnsafeEntry {
        entry: PlatformProfileEntry,
        reason: String,
    },
    ReadFailed {
        entry: PlatformProfileEntry,
        reason: String,
    },
    ConcurrentChange,
}

impl fmt::Display for PlatformProfileSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { entry } => {
                write!(f, "missing source entry {}", entry.relative_name())
            }
            Self::PermissionDenied { entry } => {
                write!(
                    f,
                    "permission denied for source entry {}",
                    entry.relative_name()
                )
            }
            Self::InvalidUtf8 { entry } => {
                write!(f, "invalid UTF-8 in source entry {}", entry.relative_name())
            }
            Self::PartialRead { entry, bytes_read } => write!(
                f,
                "partial source read of {} ({bytes_read} bytes)",
                entry.relative_name()
            ),
            Self::TooLarge { entry, limit } => write!(
                f,
                "source entry {} exceeds maximum size of {limit} bytes",
                entry.relative_name()
            ),
            Self::Openat2Unavailable { entry } => write!(
                f,
                "openat2 is unavailable while opening source entry {}",
                entry.relative_name()
            ),
            Self::UnsafeEntry { entry, reason } => {
                write!(f, "unsafe source entry {}: {reason}", entry.relative_name())
            }
            Self::ReadFailed { entry, reason } => {
                write!(
                    f,
                    "failed to read source entry {}: {reason}",
                    entry.relative_name()
                )
            }
            Self::ConcurrentChange => write!(
                f,
                "platform-profile choices changed during a consistency read"
            ),
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
    /// The simulated source deliberately has no value for this entry. This
    /// is distinct from a failed acquisition: it is passed to the adapter as
    /// absent data so it can retain the unavailable state.
    Absent,
    Partial {
        bytes_read: usize,
    },
}

/// Supplies bytes for fixed entries. This trait deliberately has no path
/// argument. Implementations must enforce their own read-size cap before they
/// allocate or return bytes. It is sealed so no external crate can introduce a
/// host-reading provider outside the reviewed Linux implementation.
pub trait PlatformProfileByteSource: private::Sealed {
    fn read(
        &self,
        entry: PlatformProfileEntry,
        maximum_size: usize,
    ) -> Result<PlatformProfileSourceRead, PlatformProfileSourceError>;

    /// The provider chooses its own acquisition class. Callers cannot supply
    /// an origin or label, which prevents fixture data from being relabeled as
    /// a real Linux read.
    fn origin(&self) -> LinuxPlatformProfileOrigin;
}

mod private {
    pub trait Sealed {}
}

/// Explicit, one-shot platform-profile query shared by future applications.
/// Implementations must return after one logical query and must not perform
/// monitoring, retries, writes, or background work.
pub trait PlatformProfileQuery: private::Sealed {
    fn read_once(&self) -> Result<LinuxPlatformProfileReading, LinuxPlatformProfileError>;
}

/// Read-only orchestration over an injected source. It always routes decoded
/// text through `PlatformProfileAdapter`; no result bypasses its validation.
/// Its source remains sealed, so an external caller cannot add a host reader.
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

    pub fn read_once(&self) -> Result<LinuxPlatformProfileReading, LinuxPlatformProfileError> {
        let active = self.read_text(PlatformProfileEntry::Active)?;
        let choices = self.read_text(PlatformProfileEntry::Choices)?;
        self.publish(active, choices)
    }

    /// Compatibility alias for existing simulated callers. New application
    /// code should use `PlatformProfileQuery::read_once` to make its
    /// single-query semantics explicit.
    pub fn read(&self) -> Result<LinuxPlatformProfileReading, LinuxPlatformProfileError> {
        self.read_once()
    }

    /// Reads choices, active profile, then choices again. Equality of the two
    /// choices values detects some concurrent changes but does not claim an
    /// atomic sysfs snapshot. It is private so only the production-shaped
    /// provider can require this stronger sequence.
    fn read_consistent(&self) -> Result<LinuxPlatformProfileReading, LinuxPlatformProfileError> {
        let choices_before = self.read_text(PlatformProfileEntry::Choices)?;
        let active = self.read_text(PlatformProfileEntry::Active)?;
        let choices_after = self.read_text(PlatformProfileEntry::Choices)?;
        if choices_before != choices_after {
            return Err(PlatformProfileSourceError::ConcurrentChange.into());
        }
        self.publish(active, choices_before)
    }

    fn publish(
        &self,
        active: Option<String>,
        choices: Option<String>,
    ) -> Result<LinuxPlatformProfileReading, LinuxPlatformProfileError> {
        let profile = PlatformProfileAdapter.parse(PlatformProfileInput {
            raw: PlatformProfileRawInput { active, choices },
            // The adapter requires provenance, but the source-specific marker
            // is deliberately obtained only after parsing succeeds below.
            provenance: TelemetryProvenance {
                artifact: TelemetryArtifact::StaticSimulation,
                source: "legion-linux::pending-origin".to_owned(),
                confidence: ConfidenceLevel::Low,
            },
        })?;
        let origin = self.source.origin();
        let provenance = provenance_for(&origin);
        let profile = PlatformProfileReading {
            provenance,
            ..profile
        };
        if !provenance_matches(&origin, profile.provenance.artifact) {
            return Err(LinuxPlatformProfileError::ProvenanceMismatch);
        }
        Ok(LinuxPlatformProfileReading { profile, origin })
    }

    fn read_text(
        &self,
        entry: PlatformProfileEntry,
    ) -> Result<Option<String>, LinuxPlatformProfileError> {
        let bytes = match self.source.read(entry, self.maximum_size)? {
            PlatformProfileSourceRead::Complete(bytes) => bytes,
            PlatformProfileSourceRead::Absent => return Ok(None),
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
            .map(Some)
            .map_err(|_| PlatformProfileSourceError::InvalidUtf8 { entry }.into())
    }
}

impl<S> PlatformProfileQuery for LinuxPlatformProfileReader<S>
where
    S: PlatformProfileByteSource,
{
    fn read_once(&self) -> Result<LinuxPlatformProfileReading, LinuxPlatformProfileError> {
        LinuxPlatformProfileReader::read_once(self)
    }
}

impl<S> private::Sealed for LinuxPlatformProfileReader<S> where S: PlatformProfileByteSource {}

fn provenance_for(origin: &LinuxPlatformProfileOrigin) -> TelemetryProvenance {
    match origin {
        LinuxPlatformProfileOrigin::StaticSimulation { root_label } => TelemetryProvenance {
            artifact: TelemetryArtifact::StaticSimulation,
            source: format!("legion-linux::simulated-root:{root_label}"),
            confidence: ConfidenceLevel::Low,
        },
        LinuxPlatformProfileOrigin::LinuxReadOnly { origin } => TelemetryProvenance {
            artifact: TelemetryArtifact::LinuxReadOnly,
            source: format!("legion-linux::readonly:{}", origin.interface()),
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
            LinuxPlatformProfileOrigin::LinuxReadOnly { .. },
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
    Absent,
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
            Some(SimulatedEntry::Absent) => Ok(PlatformProfileSourceRead::Absent),
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

    fn origin(&self) -> LinuxPlatformProfileOrigin {
        LinuxPlatformProfileOrigin::StaticSimulation {
            root_label: self.root_label.clone(),
        }
    }
}

impl private::Sealed for SimulatedPlatformProfileTree {}

/// Closed, in-memory scenarios for the isolated experimental binary. They do
/// not carry a path, descriptor, origin, or activation capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolatedPlatformProfileScenario {
    Valid,
    MissingActive,
    Symlink,
    InvalidUtf8,
    TooLarge,
    Partial,
    PermissionDenied,
    ConcurrentChange,
}

impl IsolatedPlatformProfileScenario {
    pub fn name(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::MissingActive => "missing-active",
            Self::Symlink => "symlink",
            Self::InvalidUtf8 => "invalid-utf8",
            Self::TooLarge => "too-large",
            Self::Partial => "partial",
            Self::PermissionDenied => "permission-denied",
            Self::ConcurrentChange => "concurrent-change",
        }
    }
}

/// Resolves only a closed scenario name. Host paths, flags, and environment
/// values have no representation in this API.
pub fn isolated_platform_profile_scenario(name: &str) -> Option<IsolatedPlatformProfileScenario> {
    match name {
        "valid" => Some(IsolatedPlatformProfileScenario::Valid),
        "missing-active" => Some(IsolatedPlatformProfileScenario::MissingActive),
        "symlink" => Some(IsolatedPlatformProfileScenario::Symlink),
        "invalid-utf8" => Some(IsolatedPlatformProfileScenario::InvalidUtf8),
        "too-large" => Some(IsolatedPlatformProfileScenario::TooLarge),
        "partial" => Some(IsolatedPlatformProfileScenario::Partial),
        "permission-denied" => Some(IsolatedPlatformProfileScenario::PermissionDenied),
        "concurrent-change" => Some(IsolatedPlatformProfileScenario::ConcurrentChange),
        _ => None,
    }
}

/// Executes one closed scenario entirely in memory. Every successful result
/// uses `StaticSimulation`; this function cannot construct or invoke the
/// private sysfs provider.
pub fn run_isolated_platform_profile_experiment(
    scenario: IsolatedPlatformProfileScenario,
) -> Result<LinuxPlatformProfileReading, LinuxPlatformProfileError> {
    match scenario {
        IsolatedPlatformProfileScenario::ConcurrentChange => {
            let source = SequencedStaticSimulationSource::new(vec![
                (
                    PlatformProfileEntry::Choices,
                    PlatformProfileSourceRead::Complete(b"balanced performance\n".to_vec()),
                ),
                (
                    PlatformProfileEntry::Active,
                    PlatformProfileSourceRead::Complete(b"performance\n".to_vec()),
                ),
                (
                    PlatformProfileEntry::Choices,
                    PlatformProfileSourceRead::Complete(b"balanced\n".to_vec()),
                ),
            ]);
            LinuxPlatformProfileReader::new(source, MAXIMUM_PLATFORM_PROFILE_BYTES)
                .read_consistent()
        }
        scenario => {
            let choices = SimulatedEntry::Regular(b"balanced performance\n".to_vec());
            let active = match scenario {
                IsolatedPlatformProfileScenario::Valid => {
                    SimulatedEntry::Regular(b"performance\n".to_vec())
                }
                IsolatedPlatformProfileScenario::MissingActive => SimulatedEntry::Missing,
                IsolatedPlatformProfileScenario::Symlink => SimulatedEntry::Symlink {
                    target: "simulated-link-target".to_owned(),
                },
                IsolatedPlatformProfileScenario::InvalidUtf8 => SimulatedEntry::Regular(vec![0xff]),
                IsolatedPlatformProfileScenario::TooLarge => {
                    SimulatedEntry::Regular(vec![b'p'; MAXIMUM_PLATFORM_PROFILE_BYTES + 1])
                }
                IsolatedPlatformProfileScenario::Partial => {
                    SimulatedEntry::Partial { bytes_read: 3 }
                }
                IsolatedPlatformProfileScenario::PermissionDenied => {
                    SimulatedEntry::PermissionDenied
                }
                IsolatedPlatformProfileScenario::ConcurrentChange => unreachable!(),
            };
            LinuxPlatformProfileReader::new(
                SimulatedPlatformProfileTree::new("isolated-experiment")
                    .with_entry(PlatformProfileEntry::Active, active)
                    .with_entry(PlatformProfileEntry::Choices, choices),
                MAXIMUM_PLATFORM_PROFILE_BYTES,
            )
            .read()
        }
    }
}

/// Internal in-memory sequence for checking the three-read consistency rule.
/// It has no path or descriptor and always reports static simulation.
struct SequencedStaticSimulationSource {
    responses: RefCell<VecDeque<(PlatformProfileEntry, PlatformProfileSourceRead)>>,
}

impl SequencedStaticSimulationSource {
    fn new(responses: Vec<(PlatformProfileEntry, PlatformProfileSourceRead)>) -> Self {
        Self {
            responses: RefCell::new(responses.into()),
        }
    }
}

impl private::Sealed for SequencedStaticSimulationSource {}

impl PlatformProfileByteSource for SequencedStaticSimulationSource {
    fn read(
        &self,
        entry: PlatformProfileEntry,
        _maximum_size: usize,
    ) -> Result<PlatformProfileSourceRead, PlatformProfileSourceError> {
        let (expected_entry, response) =
            self.responses.borrow_mut().pop_front().ok_or_else(|| {
                PlatformProfileSourceError::ReadFailed {
                    entry,
                    reason: "isolated scenario exhausted its fixed response sequence".to_owned(),
                }
            })?;
        if entry != expected_entry {
            return Err(PlatformProfileSourceError::ReadFailed {
                entry,
                reason: "isolated scenario requested an unexpected entry".to_owned(),
            });
        }
        Ok(response)
    }

    fn origin(&self) -> LinuxPlatformProfileOrigin {
        LinuxPlatformProfileOrigin::StaticSimulation {
            root_label: "isolated-experiment".to_owned(),
        }
    }
}

const MAXIMUM_PLATFORM_PROFILE_BYTES: usize = 4096;
#[cfg(any(test, feature = "approved-real-platform-profile-read"))]
const MAX_INTERRUPTED_READS: usize = 2;

/// Descriptor-confined implementation shared by isolated tests and the
/// private production-shaped provider. It is intentionally private: callers
/// cannot supply a directory, a path, or an origin label.
#[cfg(any(test, feature = "approved-real-platform-profile-read"))]
struct ConfinedOpenat2PlatformProfileSource {
    root: File,
    origin_kind: ConfinedSourceOrigin,
}

#[cfg(any(test, feature = "approved-real-platform-profile-read"))]
enum ConfinedSourceOrigin {
    #[cfg(test)]
    StaticSimulation { root_label: String },
    #[cfg(feature = "approved-real-platform-profile-read")]
    VerifiedLinuxOnly,
}

#[cfg(any(test, feature = "approved-real-platform-profile-read"))]
impl ConfinedOpenat2PlatformProfileSource {
    #[cfg(test)]
    fn from_test_directory(root: File, root_label: impl Into<String>) -> Self {
        Self {
            root,
            origin_kind: ConfinedSourceOrigin::StaticSimulation {
                root_label: root_label.into(),
            },
        }
    }

    /// This constructor is private and compiled only with the separately
    /// approved experimental feature. It is reachable only through the
    /// private production provider below. Its existence alone does not open
    /// sysfs: it only takes an already-open descriptor.
    #[cfg(feature = "approved-real-platform-profile-read")]
    fn from_linux_directory(root: File) -> Self {
        Self {
            root,
            origin_kind: ConfinedSourceOrigin::VerifiedLinuxOnly,
        }
    }

    fn open_entry(&self, entry: PlatformProfileEntry) -> Result<File, PlatformProfileSourceError> {
        // All resolution happens relative to `root`. The literal supplied
        // to openat2 originates only from PlatformProfileEntry, never from
        // CLI, environment, configuration, or a caller-supplied path.
        openat2(
            &self.root,
            entry.relative_name(),
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_SYMLINKS,
        )
        .map(File::from)
        .map_err(|error| map_openat2_error(entry, error))
        .and_then(|file| {
            let stat = fstat(&file).map_err(|error| PlatformProfileSourceError::ReadFailed {
                entry,
                reason: format!("could not inspect opened entry: {error}"),
            })?;
            if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
                return Err(PlatformProfileSourceError::UnsafeEntry {
                    entry,
                    reason: "opened entry is not a regular file".to_owned(),
                });
            }
            Ok(file)
        })
    }

    fn read_complete(
        &self,
        entry: PlatformProfileEntry,
    ) -> Result<PlatformProfileSourceRead, PlatformProfileSourceError> {
        let file = self.open_entry(entry)?;
        let mut bytes = Vec::with_capacity(512);
        let mut interrupted_reads = 0;

        loop {
            let mut chunk = [0_u8; 512];
            // Ask for at most one byte beyond the cap. This detects excess
            // content while never retaining more than 4097 bytes.
            let requested = (MAXIMUM_PLATFORM_PROFILE_BYTES + 1 - bytes.len()).min(chunk.len());
            match read(&file, &mut chunk[..requested]) {
                Ok(0) => return Ok(PlatformProfileSourceRead::Complete(bytes)),
                Ok(count) => {
                    bytes.extend_from_slice(&chunk[..count]);
                    if bytes.len() > MAXIMUM_PLATFORM_PROFILE_BYTES {
                        return Err(PlatformProfileSourceError::TooLarge {
                            entry,
                            limit: MAXIMUM_PLATFORM_PROFILE_BYTES,
                        });
                    }
                }
                Err(Errno::INTR) if interrupted_reads < MAX_INTERRUPTED_READS => {
                    interrupted_reads += 1;
                }
                Err(Errno::INTR) => {
                    return Err(PlatformProfileSourceError::ReadFailed {
                        entry,
                        reason: "interrupted reads exceeded the retry limit".to_owned(),
                    });
                }
                Err(error) if bytes.is_empty() => {
                    return Err(PlatformProfileSourceError::ReadFailed {
                        entry,
                        reason: format!("read failed before EOF: {error}"),
                    });
                }
                Err(_) => {
                    return Ok(PlatformProfileSourceRead::Partial {
                        bytes_read: bytes.len(),
                    });
                }
            }
        }
    }
}

#[cfg(any(test, feature = "approved-real-platform-profile-read"))]
impl PlatformProfileByteSource for ConfinedOpenat2PlatformProfileSource {
    fn read(
        &self,
        entry: PlatformProfileEntry,
        maximum_size: usize,
    ) -> Result<PlatformProfileSourceRead, PlatformProfileSourceError> {
        // The isolated provider does not permit a caller to enlarge its cap.
        // Keeping the cap exact makes the 4096-byte policy independently
        // enforceable even if a generic reader is misconfigured in a test.
        if maximum_size != MAXIMUM_PLATFORM_PROFILE_BYTES {
            return Err(PlatformProfileSourceError::ReadFailed {
                entry,
                reason: format!(
                    "confined openat2 source requires a {}-byte limit",
                    MAXIMUM_PLATFORM_PROFILE_BYTES
                ),
            });
        }
        self.read_complete(entry)
    }

    fn origin(&self) -> LinuxPlatformProfileOrigin {
        match &self.origin_kind {
            #[cfg(test)]
            ConfinedSourceOrigin::StaticSimulation { root_label } => {
                LinuxPlatformProfileOrigin::StaticSimulation {
                    root_label: root_label.clone(),
                }
            }
            #[cfg(feature = "approved-real-platform-profile-read")]
            ConfinedSourceOrigin::VerifiedLinuxOnly => LinuxPlatformProfileOrigin::LinuxReadOnly {
                origin: LinuxReadOnlyOrigin::for_verified_read(),
            },
        }
    }
}

#[cfg(any(test, feature = "approved-real-platform-profile-read"))]
impl private::Sealed for ConfinedOpenat2PlatformProfileSource {}

#[cfg(any(test, feature = "approved-real-platform-profile-read"))]
fn map_openat2_error(entry: PlatformProfileEntry, error: Errno) -> PlatformProfileSourceError {
    match error {
        Errno::NOENT => PlatformProfileSourceError::Missing { entry },
        Errno::ACCESS | Errno::PERM => PlatformProfileSourceError::PermissionDenied { entry },
        Errno::NOSYS => PlatformProfileSourceError::Openat2Unavailable { entry },
        // `ELOOP`, `EXDEV`, and `EAGAIN` mean either a prohibited link or an
        // inability to prove the requested resolution stayed confined. There
        // is intentionally no open/openat fallback or retry through a path.
        Errno::LOOP | Errno::XDEV | Errno::AGAIN => PlatformProfileSourceError::UnsafeEntry {
            entry,
            reason: format!("openat2 could not guarantee confined resolution: {error}"),
        },
        _ => PlatformProfileSourceError::ReadFailed {
            entry,
            reason: format!("openat2 failed: {error}"),
        },
    }
}

/// Production integration is compiled only with the non-default experimental
/// feature. Without that feature this private type remains uninhabited, so no
/// safe code path can construct the provider or open sysfs.
#[cfg(feature = "approved-real-platform-profile-read")]
enum ApprovedRealReadActivation {
    #[cfg(feature = "approved-real-platform-profile-read")]
    ApprovedOneShot,
}

#[cfg(feature = "approved-real-platform-profile-read")]
const REQUIRED_REAL_READ_CONFIRMATION: &str = "I-UNDERSTAND-READ-ONLY-SYSFS";

/// This guard limits the experimental facade to one attempt per process. It
/// intentionally does not record permission for another process invocation.
#[cfg(feature = "approved-real-platform-profile-read")]
static ONE_SHOT_CONSUMED: AtomicBool = AtomicBool::new(false);

/// Errors returned by the feature-gated experimental facade. They do not
/// contain a partial reading or assign Linux provenance on any failure.
#[cfg(feature = "approved-real-platform-profile-read")]
#[derive(Debug)]
pub enum ExperimentalRealReadError {
    ConfirmationRequired,
    AlreadyAttempted,
    Initialization { stage: &'static str, error: Errno },
    UnexpectedObject { component: &'static str },
    Reading(LinuxPlatformProfileError),
}

#[cfg(feature = "approved-real-platform-profile-read")]
impl fmt::Display for ExperimentalRealReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfirmationRequired => write!(f, "the exact read-only confirmation is required"),
            Self::AlreadyAttempted => write!(
                f,
                "the one-shot real read was already attempted in this process"
            ),
            Self::Initialization { stage, error } => {
                write!(
                    f,
                    "could not initialize confined sysfs access at {stage}: {error}"
                )
            }
            Self::UnexpectedObject { component } => {
                write!(f, "fixed sysfs component is not a directory: {component}")
            }
            Self::Reading(error) => error.fmt(f),
        }
    }
}

#[cfg(feature = "approved-real-platform-profile-read")]
impl std::error::Error for ExperimentalRealReadError {}

/// Performs the feature-gated, explicit one-shot real read. This is the sole
/// public activation surface; it has no root/path parameter, environment
/// configuration, fallback, retry, or monitoring behavior. It is not built in
/// normal configurations and must only be called by the separately approved
/// experimental binary.
#[cfg(feature = "approved-real-platform-profile-read")]
#[doc(hidden)]
pub fn approved_one_shot_platform_profile_read(
    confirmation: &str,
) -> Result<LinuxPlatformProfileReading, ExperimentalRealReadError> {
    if confirmation != REQUIRED_REAL_READ_CONFIRMATION {
        return Err(ExperimentalRealReadError::ConfirmationRequired);
    }
    ONE_SHOT_CONSUMED
        .compare_exchange(false, true, AtomicOrdering::SeqCst, AtomicOrdering::SeqCst)
        .map_err(|_| ExperimentalRealReadError::AlreadyAttempted)?;

    let provider = SysfsPlatformProfileProvider::new(ApprovedRealReadActivation::ApprovedOneShot)
        .map_err(ExperimentalRealReadError::from)?;
    PlatformProfileQuery::read_once(&provider).map_err(ExperimentalRealReadError::Reading)
}

/// Private production-shaped provider. It is neither exported nor connected to
/// the CLI. Its fixed descent is written here for static review only; no code
/// path can construct it in this phase.
#[cfg(feature = "approved-real-platform-profile-read")]
struct SysfsPlatformProfileProvider {
    reader: LinuxPlatformProfileReader<ConfinedOpenat2PlatformProfileSource>,
}

#[cfg(feature = "approved-real-platform-profile-read")]
impl SysfsPlatformProfileProvider {
    const SYSFS_ROOT: &'static str = "/sys";
    const FIRMWARE_DIRECTORY: &'static str = "firmware";
    const ACPI_DIRECTORY: &'static str = "acpi";

    #[allow(dead_code)]
    fn new(_activation: ApprovedRealReadActivation) -> Result<Self, SysfsProviderInitError> {
        let sysfs = open_fixed_sysfs_root()?;
        let firmware = open_confined_directory(&sysfs, Self::FIRMWARE_DIRECTORY)?;
        let acpi = open_confined_directory(&firmware, Self::ACPI_DIRECTORY)?;
        Ok(Self {
            reader: LinuxPlatformProfileReader::new(
                ConfinedOpenat2PlatformProfileSource::from_linux_directory(acpi),
                MAXIMUM_PLATFORM_PROFILE_BYTES,
            ),
        })
    }
}

/// The private Linux provider keeps the stronger three-read consistency rule
/// behind the shared one-shot application interface. It is compiled only with
/// the experimental feature and remains unreachable from the normal CLI.
#[cfg(feature = "approved-real-platform-profile-read")]
impl PlatformProfileQuery for SysfsPlatformProfileProvider {
    fn read_once(&self) -> Result<LinuxPlatformProfileReading, LinuxPlatformProfileError> {
        self.reader.read_consistent()
    }
}

#[cfg(feature = "approved-real-platform-profile-read")]
impl private::Sealed for SysfsPlatformProfileProvider {}

#[cfg(feature = "approved-real-platform-profile-read")]
#[derive(Debug)]
enum SysfsProviderInitError {
    OpenRoot(Errno),
    NotDirectory {
        component: &'static str,
    },
    OpenComponent {
        component: &'static str,
        error: Errno,
    },
}

#[cfg(feature = "approved-real-platform-profile-read")]
impl From<SysfsProviderInitError> for ExperimentalRealReadError {
    fn from(error: SysfsProviderInitError) -> Self {
        match error {
            SysfsProviderInitError::OpenRoot(error) => Self::Initialization {
                stage: "/sys",
                error,
            },
            SysfsProviderInitError::NotDirectory { component } => {
                Self::UnexpectedObject { component }
            }
            SysfsProviderInitError::OpenComponent { component, error } => Self::Initialization {
                stage: component,
                error,
            },
        }
    }
}

#[cfg(feature = "approved-real-platform-profile-read")]
impl fmt::Display for SysfsProviderInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenRoot(error) => write!(f, "could not open the fixed sysfs root: {error}"),
            Self::NotDirectory { component } => {
                write!(f, "fixed sysfs component is not a directory: {component}")
            }
            Self::OpenComponent { component, error } => {
                write!(
                    f,
                    "could not open fixed sysfs component {component}: {error}"
                )
            }
        }
    }
}

#[cfg(feature = "approved-real-platform-profile-read")]
impl std::error::Error for SysfsProviderInitError {}

#[cfg(feature = "approved-real-platform-profile-read")]
fn open_fixed_sysfs_root() -> Result<File, SysfsProviderInitError> {
    // This uses the only absolute path in the implementation. It is a private,
    // fixed literal and cannot be reached until a future phase creates the
    // activation capability. Relative components thereafter are descriptor
    // confined by `open_confined_directory`.
    let root = openat2(
        CWD,
        SysfsPlatformProfileProvider::SYSFS_ROOT,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_SYMLINKS,
    )
    .map(File::from)
    .map_err(SysfsProviderInitError::OpenRoot)?;
    ensure_directory(&root, "/sys")?;
    Ok(root)
}

#[cfg(feature = "approved-real-platform-profile-read")]
fn open_confined_directory(
    parent: &File,
    component: &'static str,
) -> Result<File, SysfsProviderInitError> {
    let directory = openat2(
        parent,
        component,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_SYMLINKS,
    )
    .map(File::from)
    .map_err(|error| SysfsProviderInitError::OpenComponent { component, error })?;
    ensure_directory(&directory, component)?;
    Ok(directory)
}

#[cfg(feature = "approved-real-platform-profile-read")]
fn ensure_directory(file: &File, component: &'static str) -> Result<(), SysfsProviderInitError> {
    let stat =
        fstat(file).map_err(|error| SysfsProviderInitError::OpenComponent { component, error })?;
    if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
        Ok(())
    } else {
        Err(SysfsProviderInitError::NotDirectory { component })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_core::{PlatformProfileVocabulary, TelemetryState};

    const MAXIMUM_SIZE: usize = 128;
    const OPENAT2_MAXIMUM_SIZE: usize = MAXIMUM_PLATFORM_PROFILE_BYTES;
    static NEXT_TEMPORARY_TREE: AtomicUsize = AtomicUsize::new(0);

    struct TemporaryTestTree {
        root: PathBuf,
    }

    impl TemporaryTestTree {
        fn new() -> Self {
            // The fixture location is fixed rather than taken from TMPDIR or
            // another environment variable. Each test still creates its own
            // exact child directory below this conventional temporary root.
            let base = Path::new("/tmp");
            for _ in 0..32 {
                let sequence = NEXT_TEMPORARY_TREE.fetch_add(1, Ordering::Relaxed);
                let root = base.join(format!(
                    "legion-linux-openat2-test-{}-{sequence}",
                    process::id()
                ));
                match fs::create_dir(&root) {
                    Ok(()) => return Self { root },
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("could not create isolated test root: {error}"),
                }
            }
            panic!("could not allocate an isolated temporary test root")
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn entry_path(&self, entry: PlatformProfileEntry) -> PathBuf {
            self.root.join(entry.relative_name())
        }

        fn write(&self, entry: PlatformProfileEntry, contents: &[u8]) {
            fs::write(self.entry_path(entry), contents)
                .expect("test fixture writes only below its isolated root");
        }

        fn source(&self) -> ConfinedOpenat2PlatformProfileSource {
            // Test setup also uses openat2, refusing symlinks before handing
            // the resulting directory descriptor to the source. The source
            // itself never receives a path.
            let root = openat2(
                CWD,
                self.path(),
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
                Mode::empty(),
                ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_SYMLINKS,
            )
            .expect("test root must be an isolated non-symlink directory");
            ConfinedOpenat2PlatformProfileSource::from_test_directory(
                File::from(root),
                "temporary-openat2-test-tree",
            )
        }
    }

    impl Drop for TemporaryTestTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn openat2_reader(
        tree: &TemporaryTestTree,
    ) -> LinuxPlatformProfileReader<ConfinedOpenat2PlatformProfileSource> {
        LinuxPlatformProfileReader::new(tree.source(), OPENAT2_MAXIMUM_SIZE)
    }

    fn tree(active: SimulatedEntry, choices: SimulatedEntry) -> SimulatedPlatformProfileTree {
        SimulatedPlatformProfileTree::new("temporary-test-tree")
            .with_entry(PlatformProfileEntry::Active, active)
            .with_entry(PlatformProfileEntry::Choices, choices)
    }

    fn regular(value: &str) -> SimulatedEntry {
        SimulatedEntry::Regular(value.as_bytes().to_vec())
    }

    #[test]
    fn openat2_source_reads_fixed_entries_until_normal_eof() {
        let tree = TemporaryTestTree::new();
        tree.write(PlatformProfileEntry::Active, b"performance\n");
        tree.write(
            PlatformProfileEntry::Choices,
            b"low-power balanced performance max-power custom\n",
        );

        let result = openat2_reader(&tree).read().unwrap();
        assert_eq!(
            result.profile.normalized.active.unwrap().name,
            "performance"
        );
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
    fn openat2_source_rejects_missing_oversized_and_invalid_utf8_entries() {
        let missing = TemporaryTestTree::new();
        missing.write(PlatformProfileEntry::Choices, b"performance\n");
        assert!(matches!(
            openat2_reader(&missing).read(),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::Missing {
                    entry: PlatformProfileEntry::Active
                }
            ))
        ));

        let oversized = TemporaryTestTree::new();
        oversized.write(
            PlatformProfileEntry::Active,
            &vec![b'p'; OPENAT2_MAXIMUM_SIZE + 1],
        );
        oversized.write(PlatformProfileEntry::Choices, b"performance\n");
        assert!(matches!(
            openat2_reader(&oversized).read(),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::TooLarge {
                    entry: PlatformProfileEntry::Active,
                    limit: OPENAT2_MAXIMUM_SIZE,
                }
            ))
        ));

        let invalid_utf8 = TemporaryTestTree::new();
        invalid_utf8.write(PlatformProfileEntry::Active, &[0xff]);
        invalid_utf8.write(PlatformProfileEntry::Choices, b"performance\n");
        assert!(matches!(
            openat2_reader(&invalid_utf8).read(),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::InvalidUtf8 {
                    entry: PlatformProfileEntry::Active
                }
            ))
        ));
    }

    #[test]
    fn openat2_source_rejects_symbolic_links_and_keeps_a_dirfd_after_substitution() {
        let symlink_tree = TemporaryTestTree::new();
        symlink_tree.write(PlatformProfileEntry::Choices, b"performance\n");
        symlink_tree.write(PlatformProfileEntry::Active, b"balanced\n");
        fs::remove_file(symlink_tree.entry_path(PlatformProfileEntry::Active))
            .expect("remove only the temporary fixture entry");
        symlink(
            symlink_tree.entry_path(PlatformProfileEntry::Choices),
            symlink_tree.entry_path(PlatformProfileEntry::Active),
        )
        .expect("fixture symlink is created only inside its temporary tree");
        assert!(matches!(
            openat2_reader(&symlink_tree).read(),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::UnsafeEntry {
                    entry: PlatformProfileEntry::Active,
                    ..
                }
            ))
        ));

        let tree = TemporaryTestTree::new();
        tree.write(PlatformProfileEntry::Active, b"performance\n");
        tree.write(PlatformProfileEntry::Choices, b"performance balanced\n");
        let source = tree.source();
        let detached_root = tree.path().with_extension("detached");
        fs::rename(tree.path(), &detached_root).expect("rename only the test root");
        fs::create_dir(tree.path()).expect("recreate only the test root name");
        fs::write(
            tree.entry_path(PlatformProfileEntry::Active),
            b"malicious-replacement\n",
        )
        .expect("write only the replacement test root");
        fs::write(
            tree.entry_path(PlatformProfileEntry::Choices),
            b"malicious-replacement\n",
        )
        .expect("write only the replacement test root");

        let reading = LinuxPlatformProfileReader::new(source, OPENAT2_MAXIMUM_SIZE)
            .read()
            .unwrap();
        assert_eq!(
            reading.profile.normalized.active.unwrap().name,
            "performance"
        );
        fs::remove_dir_all(detached_root).expect("remove only the detached test root");
    }

    #[test]
    fn openat2_source_fails_closed_for_unavailable_or_unconfined_opening() {
        assert!(matches!(
            map_openat2_error(PlatformProfileEntry::Active, Errno::NOSYS),
            PlatformProfileSourceError::Openat2Unavailable {
                entry: PlatformProfileEntry::Active
            }
        ));
        assert!(matches!(
            map_openat2_error(PlatformProfileEntry::Choices, Errno::ACCESS),
            PlatformProfileSourceError::PermissionDenied {
                entry: PlatformProfileEntry::Choices
            }
        ));
        assert!(matches!(
            map_openat2_error(PlatformProfileEntry::Choices, Errno::PERM),
            PlatformProfileSourceError::PermissionDenied {
                entry: PlatformProfileEntry::Choices
            }
        ));
        assert!(matches!(
            map_openat2_error(PlatformProfileEntry::Active, Errno::LOOP),
            PlatformProfileSourceError::UnsafeEntry {
                entry: PlatformProfileEntry::Active,
                ..
            }
        ));
        assert!(matches!(
            map_openat2_error(PlatformProfileEntry::Active, Errno::XDEV),
            PlatformProfileSourceError::UnsafeEntry {
                entry: PlatformProfileEntry::Active,
                ..
            }
        ));
        assert!(matches!(
            map_openat2_error(PlatformProfileEntry::Active, Errno::AGAIN),
            PlatformProfileSourceError::UnsafeEntry {
                entry: PlatformProfileEntry::Active,
                ..
            }
        ));
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
    fn shared_read_once_interface_validates_and_preserves_simulated_provenance() {
        let valid_source = tree(regular("performance\n"), regular("balanced performance\n"));
        let valid = LinuxPlatformProfileReader::new(valid_source.clone(), MAXIMUM_SIZE);
        let reading = PlatformProfileQuery::read_once(&valid).unwrap();
        assert_eq!(reading.profile.state, TelemetryState::Available);
        assert_eq!(
            reading.profile.provenance.artifact,
            TelemetryArtifact::StaticSimulation
        );
        assert!(matches!(
            reading.origin,
            LinuxPlatformProfileOrigin::StaticSimulation { .. }
        ));

        let alias_reading = LinuxPlatformProfileReader::new(valid_source, MAXIMUM_SIZE)
            .read()
            .unwrap();
        assert_eq!(reading, alias_reading);

        let missing = LinuxPlatformProfileReader::new(
            tree(SimulatedEntry::Missing, regular("performance\n")),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            PlatformProfileQuery::read_once(&missing),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::Missing {
                    entry: PlatformProfileEntry::Active
                }
            ))
        ));

        let inconsistent = LinuxPlatformProfileReader::new(
            tree(regular("performance\n"), regular("balanced\n")),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            PlatformProfileQuery::read_once(&inconsistent),
            Err(LinuxPlatformProfileError::Parse(
                PlatformProfileError::ActiveNotAvailable { .. }
            ))
        ));
    }

    #[test]
    fn simulated_absence_stays_distinct_from_acquisition_failure() {
        let reader = LinuxPlatformProfileReader::new(
            tree(SimulatedEntry::Absent, regular("balanced performance\n")),
            MAXIMUM_SIZE,
        );
        let reading = PlatformProfileQuery::read_once(&reader).unwrap();
        assert_eq!(reading.profile().raw.active, None);
        assert_eq!(reading.profile().state, TelemetryState::Unavailable);
        assert!(matches!(
            reading.origin(),
            LinuxPlatformProfileOrigin::StaticSimulation { .. }
        ));

        let failed = LinuxPlatformProfileReader::new(
            tree(SimulatedEntry::Missing, regular("balanced performance\n")),
            MAXIMUM_SIZE,
        );
        assert!(matches!(
            PlatformProfileQuery::read_once(&failed),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::Missing {
                    entry: PlatformProfileEntry::Active
                }
            ))
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
    fn isolated_experiment_scenarios_are_closed_and_never_publish_linux_origin() {
        for rejected in [
            "/sys",
            "/proc",
            "/dev",
            "--real",
            "--sysfs",
            "relative/path",
        ] {
            assert_eq!(isolated_platform_profile_scenario(rejected), None);
        }

        let valid =
            run_isolated_platform_profile_experiment(IsolatedPlatformProfileScenario::Valid)
                .unwrap();
        assert!(matches!(
            valid.origin,
            LinuxPlatformProfileOrigin::StaticSimulation { .. }
        ));
        assert_eq!(
            valid.profile.provenance.artifact,
            TelemetryArtifact::StaticSimulation
        );

        assert!(matches!(
            run_isolated_platform_profile_experiment(IsolatedPlatformProfileScenario::Symlink),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::UnsafeEntry { .. }
            ))
        ));
        assert!(matches!(
            run_isolated_platform_profile_experiment(IsolatedPlatformProfileScenario::InvalidUtf8),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::InvalidUtf8 { .. }
            ))
        ));
        assert!(matches!(
            run_isolated_platform_profile_experiment(IsolatedPlatformProfileScenario::TooLarge),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::TooLarge { .. }
            ))
        ));
        assert!(matches!(
            run_isolated_platform_profile_experiment(IsolatedPlatformProfileScenario::Partial),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::PartialRead { .. }
            ))
        ));
        assert!(matches!(
            run_isolated_platform_profile_experiment(
                IsolatedPlatformProfileScenario::PermissionDenied
            ),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::PermissionDenied { .. }
            ))
        ));
        assert!(matches!(
            run_isolated_platform_profile_experiment(
                IsolatedPlatformProfileScenario::ConcurrentChange
            ),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::ConcurrentChange
            ))
        ));
    }

    #[test]
    fn coherence_read_rejects_changed_choices_without_publishing_a_profile() {
        let source = SequencedStaticSimulationSource::new(vec![
            (
                PlatformProfileEntry::Choices,
                PlatformProfileSourceRead::Complete(b"balanced performance\n".to_vec()),
            ),
            (
                PlatformProfileEntry::Active,
                PlatformProfileSourceRead::Complete(b"performance\n".to_vec()),
            ),
            (
                PlatformProfileEntry::Choices,
                PlatformProfileSourceRead::Complete(b"balanced\n".to_vec()),
            ),
        ]);
        let reader = LinuxPlatformProfileReader::new(source, MAXIMUM_SIZE);
        assert!(matches!(
            reader.read_consistent(),
            Err(LinuxPlatformProfileError::Source(
                PlatformProfileSourceError::ConcurrentChange
            ))
        ));
    }

    #[test]
    fn linux_origin_has_a_distinct_read_only_provenance_label() {
        let provenance = provenance_for(&LinuxPlatformProfileOrigin::LinuxReadOnly {
            origin: LinuxReadOnlyOrigin::for_verified_read(),
        });
        assert_eq!(provenance.artifact, TelemetryArtifact::LinuxReadOnly);
        assert!(provenance.source.starts_with("legion-linux::readonly:"));
    }
}
