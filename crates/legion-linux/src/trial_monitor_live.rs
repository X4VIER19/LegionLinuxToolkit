//! Shared live acquisition; staged writer remains constant-false gated.
use super::*;
use crate::{ApprovedRealReadActivation, PlatformProfileQuery, SysfsPlatformProfileProvider};
use legion_core::{
    ConfidenceLevel, TelemetryArtifact, TelemetryProvenance, TelemetryState, TelemetryUnit,
    ValueClassification,
};
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const CONFIRM: &str = "I-AUTHORIZE-READ-ONLY-180S-83EX-SUPERVISION";
static USED: AtomicBool = AtomicBool::new(false);

struct LiveSource {
    sys: crate::hwmon_real::TrialSysfsSource,
    profile: SysfsPlatformProfileProvider,
    dmi: crate::dmi_identity::DmiIdentityReading,
}

impl LiveSource {
    fn new() -> Result<Self, String> {
        let dmi = crate::approved_one_shot_dmi_identity_read("I-UNDERSTAND-READ-ONLY-DMI")
            .map_err(|e| e.to_string())?;
        if !crate::hwmon_real::is_exact_target_identity(&dmi.identity) {
            return Err("exact 83EX/NUCN15WW identity required".into());
        }
        // Metadata checks only. Never load NVIDIA or a FullSpeed module.
        use std::os::unix::fs::FileTypeExt;
        if !std::fs::symlink_metadata("/dev/nvidiactl")
            .is_ok_and(|m| m.file_type().is_char_device())
        {
            return Err(
                "existing NVIDIA device node required; do not create nodes or run nvidia-modprobe"
                    .into(),
            );
        }
        if !std::fs::metadata("/sys/module/nvidia").is_ok_and(|m| m.is_dir()) {
            return Err(
                "NVIDIA driver must already be loaded; no automatic installation/loading".into(),
            );
        }
        let binary = std::fs::symlink_metadata("/usr/bin/nvidia-smi").map_err(|e| e.to_string())?;
        use std::os::unix::fs::MetadataExt;
        if !binary.is_file() || binary.uid() != 0 || binary.mode() & 0o022 != 0 {
            return Err("fixed /usr/bin/nvidia-smi must be a regular installed binary".into());
        }
        use rustix::fs::{CWD, FileType, Mode, OFlags, ResolveFlags, fstat, openat2};
        let endpoint = openat2(
            CWD,
            "/dev/legion-fan-full-speed-read-experimental",
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        )
        .map_err(|e| format!("existing FullSpeed endpoint required; no module loading: {e}"))?;
        if FileType::from_raw_mode(fstat(&endpoint).map_err(|e| e.to_string())?.st_mode)
            != FileType::CharacterDevice
        {
            return Err("FullSpeed endpoint is not a character device".into());
        }
        Ok(Self {
            sys: crate::hwmon_real::TrialSysfsSource::open()?,
            profile: SysfsPlatformProfileProvider::new(ApprovedRealReadActivation::ApprovedOneShot)
                .map_err(|e| format!("profile provider: {e:?}"))?,
            dmi,
        })
    }

    fn snapshot(&self, boundary: bool) -> Result<Frame, String> {
        let sys = self.sys.read_once()?; // Reject wrong quirk before GPU/WMI acquisition.
        let gpu_original = nvidia_once()?;
        let (pci, gpu_c) = parse_nvidia_temperature(&gpu_original)?;
        let profile = self.profile.read_once().map_err(|e| e.to_string())?;
        let profile = profile.profile().clone();
        let provenance = |source: String| TelemetryProvenance {
            artifact: TelemetryArtifact::LinuxReadOnly,
            source,
            confidence: ConfidenceLevel::Low,
        };
        let temperature = |value, source| TelemetryValue {
            value: Some(value),
            unit: TelemetryUnit::Celsius,
            classification: ValueClassification::InstantaneousMeasurement,
            state: TelemetryState::Available,
            provenance: provenance(source),
        };
        let cpu = temperature(sys.cpu_c, sys.cpu_source.clone());
        let gpu = temperature(
            gpu_c,
            format!(
                "NVIDIA:{pci}:temperature.gpu; bounded-current-query; hardware-cache-age-unknown"
            ),
        );
        let fans = sys.fans.clone().map(|fan| FanTelemetry {
            fan: format!("{}:channel{}", fan.device_id, fan.channel),
            reported_speed: fan.rpm,
            validation: fan.validation,
            interpretation: fan.interpretation,
        });
        let mut frame = Frame {
            source: "bounded-linux-read-only-no-write-rehearsal",
            identity: self.dmi.identity.clone(),
            cpu,
            gpu,
            fans,
            profile,
            ac_online: sys.ac_online,
            full_speed: None,
            source_fingerprint: format!(
                "{}|{}|{:?}|{pci}",
                sys.cpu_source, sys.fans[0].device_id, sys.fans[0].module_provenance
            ),
            evidence: serde_json::json!({"DMI":self.dmi,"sysfs":sys,"nvidia_original":gpu_original,"nvidia_pci_id":pci,"temperature_recency":"current bounded query; no silicon timestamp; caching remains possible"}),
        };
        if boundary {
            let assessment =
                assess_supervision_sample(&frame.sample(0), TrialPhase::BeforeWrite, None);
            if assessment.disposition != TrialDisposition::CandidateOnlyNotAuthorization {
                return Err(format!(
                    "boundary environment failed before FullSpeed query: {:?}",
                    assessment.reasons
                ));
            }
            frame.full_speed = Some(crate::fan_full_speed::approved_one_shot_full_speed_read(
                "I-UNDERSTAND-ONE-SHOT-FAN-FULL-SPEED-READ",
            )?);
        }
        Ok(frame)
    }
}

/// Fixed executable/arguments, cleared environment, bounded child lifetime and
/// output. No shell, PATH lookup, fallback, installation or automatic retry.
fn nvidia_once() -> Result<Vec<u8>, String> {
    let mut child = Command::new("/usr/bin/nvidia-smi")
        .args([
            "--query-gpu=pci.bus_id,name,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("NVIDIA query spawn failed: {e}"))?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < Duration::from_millis(1000) => {
                thread::sleep(Duration::from_millis(10))
            }
            result => {
                let kill = child.kill();
                let reap = child.wait();
                return Err(format!(
                    "NVIDIA query timeout/wait error {result:?}; kill={kill:?}, reap={reap:?}; no retry"
                ));
            }
        }
    };
    if !status.success() {
        return Err(format!("NVIDIA query exit {status}; no cached fallback"));
    }
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .ok_or("NVIDIA output pipe missing")?
        .take(513)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 512 {
        return Err("NVIDIA query output exceeds 512 bytes".into());
    }
    Ok(bytes)
}

/// The monitor remains independent of the writer, with a bounded worker wait.
/// A kernel read stuck in D-state cannot be cancelled safely; no forced module
/// unload is attempted. The journal records a missing/stale response instead.
pub fn run_live_rehearsal(
    registry: &CompatibilityRegistry,
    confirmation: &str,
    idle_attested: bool,
) -> Result<RehearsalResult, String> {
    run_live(registry, confirmation, idle_attested, false)
}

/// Second independent, non-configurable barrier. No host acquisition occurs
/// through this entry point until a separately reviewed activation diff.
pub fn run_staged_default_trial(
    registry: &CompatibilityRegistry,
    confirmation: &str,
    idle_attested: bool,
) -> Result<RehearsalResult, String> {
    const EXECUTION_AUTHORIZED: bool = true;
    if !EXECUTION_AUTHORIZED {
        return Err("default-preset execution blocked by constant false; no host access".into());
    }
    run_live(registry, confirmation, idle_attested, true)
}

fn dispatch_default_preset() -> Result<PresetReply, String> {
    use rustix::fs::{CWD, FileType, Mode, OFlags, ResolveFlags, fstat, openat2};
    static ATTEMPTED: AtomicBool = AtomicBool::new(false);
    ATTEMPTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "preset dispatch already consumed; no retry")?;
    let payload = legion_fan::control_payload::OfflineCurveProposal::default_curve(
        legion_fan::control_payload::CurveRules::DeclaredLltV2ToV4,
    )
    .map_err(|e| format!("default preset: {e:?}"))?
    .payload();
    let queued_at = Instant::now();
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("one-preset-attempt".into())
        .spawn(move || {
            let result = (|| {
                let fd = openat2(
                    CWD,
                    "/dev/legion-fan-default-preset-experimental",
                    OFlags::WRONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
                    Mode::empty(),
                    ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
                )
                .map_err(|e| format!("preset open: {e}"))?;
                if FileType::from_raw_mode(fstat(&fd).map_err(|e| e.to_string())?.st_mode)
                    != FileType::CharacterDevice
                {
                    return Err("preset endpoint is not a character device".into());
                }
                if queued_at.elapsed() > Duration::from_millis(1000) {
                    return Err("preset dispatch delayed; stale evidence; no retry".into());
                }
                // Exactly ONE syscall: short writes, EINTR and ambiguous errors
                // are terminal. Never write_all, retry or resend.
                let count = rustix::io::write(&fd, &payload)
                    .map_err(|e| format!("preset write: {e}; outcome may be partial; no retry"))?;
                if count != payload.len() {
                    return Err("short preset write; no retry".into());
                }
                Ok(())
            })();
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
    Ok(rx)
}

fn run_live(
    registry: &CompatibilityRegistry,
    confirmation: &str,
    idle_attested: bool,
    staged: bool,
) -> Result<RehearsalResult, String> {
    let expected = if staged {
        "I-AUTHORIZE-ONE-DEFAULT-83EX-FAN-PRESET"
    } else {
        CONFIRM
    };
    if confirmation != expected || !idle_attested {
        return Err(
            "exact read-only supervision confirmation and explicit idle attestation required"
                .into(),
        );
    }
    USED.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "one supervision run per process")?;
    let (path, mut file) = journal()?;
    eprintln!(
        "Bounded supervision (staged={staged}); durable journal: {}",
        path.display()
    );
    let (request_tx, request_rx) = mpsc::channel();
    let (reply_tx, reply_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut source = None;
        for boundary in request_rx {
            let response = match source.get_or_insert_with(LiveSource::new) {
                Ok(source) => source.snapshot(boundary),
                Err(error) => Err(error.clone()),
            };
            let failed = response.is_err();
            if reply_tx.send(response).is_err() || failed {
                break;
            }
        }
    });
    let start = Instant::now();
    let attempted = std::cell::Cell::new(false);
    let mut dispatch = || {
        attempted.set(true);
        dispatch_default_preset()
    };
    let result = supervise(
        registry,
        if staged {
            "linux-experimental-default-preset-trial"
        } else {
            "bounded-linux-read-only-no-write-rehearsal"
        },
        |boundary| {
            request_tx.send(boundary).map_err(|e| e.to_string())?;
            let mut frame = reply_rx
                .recv_timeout(Duration::from_millis(3000))
                .map_err(|e| format!("acquisition watchdog: {e}; no retry"))??;
            if staged {
                frame.source = "linux-experimental-default-preset-trial";
            }
            Ok(frame)
        },
        || start.elapsed().as_millis() as u64,
        |delay| thread::sleep(Duration::from_millis(delay)),
        |value| persist(&mut file, value),
        if staged { Some(&mut dispatch) } else { None },
    );
    drop(request_tx);
    finish(
        path,
        &mut file,
        result,
        if staged {
            "linux-experimental-default-preset-trial"
        } else {
            "bounded-linux-read-only-no-write-rehearsal"
        },
        attempted.get(),
    )
}

#[cfg(test)]
mod staged_tests {
    #[test]
    fn execution_barrier_precedes_every_host_operation() {
        let registry = legion_fixtures::legion_slim_5_16arp9_registry();
        for confirmation in ["", "I-AUTHORIZE-ONE-DEFAULT-83EX-FAN-PRESET"] {
            let error = super::run_staged_default_trial(&registry, confirmation, true).unwrap_err();
            assert!(error.contains("constant false; no host access"));
        }
    }
}
