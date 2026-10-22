//! Descriptor-confined DMI identity acquisition shape.
//!
//! This experimental module is compiled only for its non-default feature (or
//! tests). It has no public constructor, no activation function, and no host
//! root. Tests provide an already-open temporary directory and retain static
//! simulation provenance.

use crate::host_identity::private;
use crate::{
    HostIdentityField, HostIdentitySource, HostIdentitySourceError, LinuxHostIdentityError,
    LinuxHostIdentityProvider,
};
use legion_core::HostIdentity;
use rustix::{
    fs::{FileType, Mode, OFlags, ResolveFlags, fstat, openat2},
    io::{Errno, read},
};
use std::{fmt, fs::File};

const MAXIMUM_DMI_IDENTITY_BYTES: usize = 1024;

/// Fixed DMI attributes considered by the future provider. Their mapping is a
/// design hypothesis until it is separately validated on the target computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmiIdentityField {
    Manufacturer,
    ProductIdentifier,
    Model,
    BiosVersion,
}

impl DmiIdentityField {
    const fn relative_name(self) -> &'static str {
        match self {
            Self::Manufacturer => "sys_vendor",
            Self::ProductIdentifier => "product_version",
            Self::Model => "product_name",
            Self::BiosVersion => "bios_version",
        }
    }

    const fn host_field(self) -> HostIdentityField {
        match self {
            Self::Manufacturer => HostIdentityField::Manufacturer,
            Self::ProductIdentifier => HostIdentityField::ProductIdentifier,
            Self::Model => HostIdentityField::Model,
            Self::BiosVersion => HostIdentityField::BiosVersion,
        }
    }
}

/// Errors specific to confined DMI acquisition. They are converted into the
/// existing source-error boundary before identity validation begins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DmiIdentityError {
    Missing {
        field: DmiIdentityField,
    },
    TooLarge {
        field: DmiIdentityField,
        limit: usize,
    },
    InvalidUtf8 {
        field: DmiIdentityField,
    },
    UnsafeEntry {
        field: DmiIdentityField,
        reason: String,
    },
    Openat2Unavailable {
        field: DmiIdentityField,
    },
    ReadFailed {
        field: DmiIdentityField,
        reason: String,
    },
}

impl fmt::Display for DmiIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { field } => write!(f, "missing DMI attribute {}", field.relative_name()),
            Self::TooLarge { field, limit } => write!(
                f,
                "DMI attribute {} exceeds the {limit}-byte limit",
                field.relative_name()
            ),
            Self::InvalidUtf8 { field } => {
                write!(f, "DMI attribute {} is not UTF-8", field.relative_name())
            }
            Self::UnsafeEntry { field, reason } => {
                write!(
                    f,
                    "unsafe DMI attribute {}: {reason}",
                    field.relative_name()
                )
            }
            Self::Openat2Unavailable { field } => write!(
                f,
                "openat2 is unavailable for DMI attribute {}",
                field.relative_name()
            ),
            Self::ReadFailed { field, reason } => {
                write!(
                    f,
                    "failed to read DMI attribute {}: {reason}",
                    field.relative_name()
                )
            }
        }
    }
}

impl std::error::Error for DmiIdentityError {}

/// A private, descriptor-confined source. It accepts only an already-open
/// directory descriptor and asks for fixed literal attribute names.
struct ConfinedDmiIdentitySource {
    root: File,
    simulation_label: String,
}

impl ConfinedDmiIdentitySource {
    #[cfg(test)]
    fn from_test_directory(root: File, simulation_label: impl Into<String>) -> Self {
        Self {
            root,
            simulation_label: simulation_label.into(),
        }
    }

    fn read_field(&self, field: DmiIdentityField) -> Result<String, DmiIdentityError> {
        let file = openat2(
            &self.root,
            field.relative_name(),
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_SYMLINKS,
        )
        .map(File::from)
        .map_err(|error| map_open_error(field, error))?;
        let stat = fstat(&file).map_err(|error| DmiIdentityError::ReadFailed {
            field,
            reason: format!("could not inspect opened attribute: {error}"),
        })?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(DmiIdentityError::UnsafeEntry {
                field,
                reason: "opened attribute is not a regular file".to_owned(),
            });
        }

        let mut bytes = Vec::with_capacity(128);
        loop {
            let mut chunk = [0_u8; 256];
            let requested = (MAXIMUM_DMI_IDENTITY_BYTES + 1 - bytes.len()).min(chunk.len());
            match read(&file, &mut chunk[..requested]) {
                Ok(0) => break,
                Ok(count) => {
                    bytes.extend_from_slice(&chunk[..count]);
                    if bytes.len() > MAXIMUM_DMI_IDENTITY_BYTES {
                        return Err(DmiIdentityError::TooLarge {
                            field,
                            limit: MAXIMUM_DMI_IDENTITY_BYTES,
                        });
                    }
                }
                Err(Errno::INTR) => {
                    return Err(DmiIdentityError::ReadFailed {
                        field,
                        reason: "read interrupted before EOF".to_owned(),
                    });
                }
                Err(error) => {
                    return Err(DmiIdentityError::ReadFailed {
                        field,
                        reason: format!("read failed before EOF: {error}"),
                    });
                }
            }
        }
        let text = String::from_utf8(bytes).map_err(|_| DmiIdentityError::InvalidUtf8 { field })?;
        // sysfs attributes conventionally terminate a textual value with one
        // newline. Remove only that delimiter; any remaining control byte is
        // rejected later by `HostIdentity` validation.
        Ok(text.strip_suffix('\n').unwrap_or(&text).to_owned())
    }
}

impl HostIdentitySource for ConfinedDmiIdentitySource {
    fn acquire(&self) -> Result<legion_core::HostIdentityInput, HostIdentitySourceError> {
        let read = |field| {
            self.read_field(field).map_err(|error| match error {
                DmiIdentityError::Missing { .. } => HostIdentitySourceError::Missing {
                    field: field.host_field(),
                },
                error => HostIdentitySourceError::Unavailable {
                    reason: error.to_string(),
                },
            })
        };
        Ok(legion_core::HostIdentityInput {
            manufacturer: Some(read(DmiIdentityField::Manufacturer)?),
            product_identifier: Some(read(DmiIdentityField::ProductIdentifier)?),
            model: Some(read(DmiIdentityField::Model)?),
            bios_version: Some(read(DmiIdentityField::BiosVersion)?),
            kernel_version: None,
            distribution: None,
        })
    }

    fn simulation_label(&self) -> &str {
        &self.simulation_label
    }
}

impl private::Sealed for ConfinedDmiIdentitySource {}

/// Private production-shaped wrapper. The uninhabited activation marker below
/// keeps it impossible to instantiate outside tests, even with the feature.
struct ExperimentalDmiIdentityProvider {
    provider: LinuxHostIdentityProvider<ConfinedDmiIdentitySource>,
}

impl ExperimentalDmiIdentityProvider {
    #[cfg(test)]
    fn from_test_directory(root: File, simulation_label: impl Into<String>) -> Self {
        Self {
            provider: LinuxHostIdentityProvider::new(
                ConfinedDmiIdentitySource::from_test_directory(root, simulation_label),
            ),
        }
    }

    fn read_once(&self) -> Result<HostIdentity, LinuxHostIdentityError> {
        self.provider.read_once()
    }
}

/// A future real constructor would require a distinct reviewed change that
/// adds exactly one approved activation value. This phase leaves it empty.
#[allow(dead_code)]
enum ApprovedDmiIdentityReadActivation {}

fn map_open_error(field: DmiIdentityField, error: Errno) -> DmiIdentityError {
    match error {
        Errno::NOENT => DmiIdentityError::Missing { field },
        Errno::NOSYS => DmiIdentityError::Openat2Unavailable { field },
        Errno::LOOP | Errno::XDEV | Errno::AGAIN => DmiIdentityError::UnsafeEntry {
            field,
            reason: format!("openat2 could not guarantee confined resolution: {error}"),
        },
        _ => DmiIdentityError::ReadFailed {
            field,
            reason: format!("openat2 failed: {error}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_core::HostIdentityOrigin;
    use std::{
        fs,
        os::unix::fs::symlink,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static NEXT_TEST_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    fn test_directory(name: &str) -> PathBuf {
        let number = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "legion-linux-dmi-identity-{name}-{}-{number}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn write_valid_dmi_tree(path: &std::path::Path) {
        fs::write(path.join("sys_vendor"), b"Lenovo\n").unwrap();
        fs::write(path.join("product_version"), b"83EX\n").unwrap();
        fs::write(path.join("product_name"), b"Legion Slim 5 16ARP9\n").unwrap();
        fs::write(path.join("bios_version"), b"NUCN15WW\n").unwrap();
    }

    fn provider(path: &std::path::Path) -> ExperimentalDmiIdentityProvider {
        ExperimentalDmiIdentityProvider::from_test_directory(
            File::open(path).unwrap(),
            "dmi-test-tree",
        )
    }

    #[test]
    fn confined_test_tree_produces_static_simulation_only() {
        let path = test_directory("valid");
        write_valid_dmi_tree(&path);
        let identity = provider(&path).read_once().unwrap();
        assert_eq!(identity.manufacturer(), Some("Lenovo"));
        assert!(matches!(
            identity.origin(),
            HostIdentityOrigin::StaticSimulation { label } if label == "dmi-test-tree"
        ));
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn missing_dmi_attribute_is_a_source_error() {
        let path = test_directory("missing");
        write_valid_dmi_tree(&path);
        fs::remove_file(path.join("bios_version")).unwrap();
        assert!(matches!(
            provider(&path).read_once(),
            Err(LinuxHostIdentityError::Source(
                HostIdentitySourceError::Missing {
                    field: HostIdentityField::BiosVersion,
                }
            ))
        ));
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn invalid_utf8_and_excessive_content_fail_before_identity_publication() {
        let invalid_path = test_directory("invalid-utf8");
        write_valid_dmi_tree(&invalid_path);
        fs::write(invalid_path.join("product_name"), [0xff]).unwrap();
        assert!(matches!(
            provider(&invalid_path).read_once(),
            Err(LinuxHostIdentityError::Source(
                HostIdentitySourceError::Unavailable { .. }
            ))
        ));
        fs::remove_dir_all(invalid_path).unwrap();

        let large_path = test_directory("large");
        write_valid_dmi_tree(&large_path);
        fs::write(
            large_path.join("product_name"),
            vec![b'x'; MAXIMUM_DMI_IDENTITY_BYTES + 1],
        )
        .unwrap();
        assert!(matches!(
            provider(&large_path).read_once(),
            Err(LinuxHostIdentityError::Source(
                HostIdentitySourceError::Unavailable { .. }
            ))
        ));
        fs::remove_dir_all(large_path).unwrap();
    }

    #[test]
    fn symbolic_links_are_rejected_without_following_them() {
        let path = test_directory("symlink");
        write_valid_dmi_tree(&path);
        fs::remove_file(path.join("product_version")).unwrap();
        symlink("/not-a-dmi-value", path.join("product_version")).unwrap();
        assert!(matches!(
            provider(&path).read_once(),
            Err(LinuxHostIdentityError::Source(
                HostIdentitySourceError::Unavailable { .. }
            ))
        ));
        fs::remove_dir_all(path).unwrap();
    }
}
