//! Bounded supervision, no-write by default. The staged preset caller has an
//! independent constant-false barrier; no module loader or profile setter.
use legion_core::{
    CompatibilityRegistry, FanTelemetry, HostIdentity, PlatformProfileReading, TelemetryValue,
};
use legion_fan::control_trial::{
    TrialDisposition, TrialPhase, TrialSample, assess_supervision_sample, assess_trial_sample,
};
use serde::Serialize;
use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "experimental-fan-trial-monitor")]
#[path = "trial_monitor_live.rs"]
mod live;
#[cfg(feature = "experimental-fan-trial-monitor")]
pub use live::run_live_rehearsal;
#[cfg(feature = "experimental-fan-trial-monitor")]
pub use live::run_staged_default_trial;

type PresetReply = std::sync::mpsc::Receiver<Result<(), String>>;

#[derive(Serialize)]
struct Frame {
    source: &'static str,
    identity: HostIdentity,
    cpu: TelemetryValue<f64>,
    gpu: TelemetryValue<f64>,
    fans: [FanTelemetry; 2],
    profile: PlatformProfileReading,
    ac_online: bool,
    full_speed: Option<crate::fan_full_speed::FullSpeedObservation>,
    evidence: serde_json::Value,
    /// Pins CPU hwmon source, quirk device/module/channels and NVIDIA PCI ID.
    source_fingerprint: String,
}

impl Frame {
    fn sample(&self, age_ms: u64) -> TrialSample<'_> {
        TrialSample {
            cpu: &self.cpu,
            gpu: &self.gpu,
            fans: [&self.fans[0], &self.fans[1]],
            ages_ms: [Some(age_ms); 4],
            independent_sensor_mapping_confirmed: true,
            ac_online: Some(self.ac_online),
            custom_profile: Some(
                self.profile.state == legion_core::TelemetryState::Available
                    && self
                        .profile
                        .normalized
                        .active
                        .as_ref()
                        .is_some_and(|p| p.name == "custom"),
            ),
            fan_full_speed: self
                .full_speed
                .as_ref()
                .and_then(|f| match f.result().state {
                    legion_fan::fan_full_speed::FanFullSpeedState::Disabled => Some(false),
                    legion_fan::fan_full_speed::FanFullSpeedState::Enabled => Some(true),
                    _ => None,
                }),
            // Explicit operator attestation, not automatic load measurement.
            intensive_workload_absent: Some(true),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RehearsalResult {
    pub source: &'static str,
    pub log_file: PathBuf,
    pub completed: bool,
    pub write_attempted: bool,
    pub execution_enabled: bool,
}

fn event(kind: &str, index: usize, extra: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"event":kind,"sample_index":index,
        "unix_ms":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis(),
        "write_attempted":false,"execution_enabled":false,"data":extra})
}

/// Injected acquisition, monotonic clock and journal. Tests never instantiate
/// live sources. FullSpeed is requested only at the no-write boundary (60 s).
fn supervise(
    registry: &CompatibilityRegistry,
    origin: &'static str,
    mut acquire: impl FnMut(bool) -> Result<Frame, String>,
    mut now: impl FnMut() -> u64,
    mut wait: impl FnMut(u64),
    mut journal: impl FnMut(&serde_json::Value) -> Result<(), String>,
    mut preset: Option<&mut dyn FnMut() -> Result<PresetReply, String>>,
) -> Result<(), String> {
    let staged = preset.is_some();
    let dispatch_attempted = std::cell::Cell::new(false);
    let mut log = |value: &serde_json::Value| {
        let mut value = value.clone();
        value["source"] = serde_json::json!(origin);
        value["write_attempted"] = serde_json::json!(dispatch_attempted.get());
        value["execution_enabled"] =
            serde_json::json!(staged && origin == "linux-experimental-default-preset-trial");
        journal(&value)
    };
    let started = now();
    let mut baseline_anchor = started;
    let mut post_anchor = None;
    let mut baseline: Option<Frame> = None;
    let mut fingerprint = None;
    let mut cpu_minimum = f64::INFINITY;
    let mut gpu_minimum = f64::INFINITY;
    let mut preset_reply = None;
    let mut preset_result = None;
    let mut post_anomaly = false;
    log(&event(
        if preset.is_some() {
            "start-staged-default-trial"
        } else {
            "start-no-write-rehearsal"
        },
        0,
        serde_json::json!({"baseline_seconds":60,"post_marker_seconds":120,"cadence_ms":1000,"idle":"operator-attested-not-measured","FullSpeed":"single-query-at-boundary-not-continuously-refreshed"}),
    ))?;
    for index in 0..=180 {
        let context = if index <= 60 {
            "cancel-before-write"
        } else {
            if staged {
                "post-preset-dispatch-anomaly; cannot-rollback-SFAN"
            } else {
                "post-no-write-marker-anomaly; not-SFAN-rollback"
            }
        };
        let due = if index > 60 {
            post_anchor.ok_or("no durable no-write marker")? + (index as u64 - 60) * 1000
        } else {
            baseline_anchor + index as u64 * 1000
        };
        let current = now();
        if current > due + 250 {
            return Err(format!(
                "{context}, sample {index}: scheduler late; no catch-up polling or retry"
            ));
        }
        if current < due {
            wait(due - current);
        }
        let begin = now();
        log(&event(
            "sample-begin",
            index,
            serde_json::json!({"phase":context,"FullSpeed_query_requested":index==60}),
        ))?;
        let frame = match acquire(index == 60) {
            Ok(frame) => frame,
            Err(error) if dispatch_attempted.get() => {
                post_anomaly = true;
                log(&event(
                    "post-acquisition-error",
                    index,
                    serde_json::json!({"error":error,"no_cached_value":true}),
                ))?;
                continue;
            }
            Err(error) => return Err(format!("{context}, sample {index}: {error}")),
        };
        if !matches!(
            crate::fan_control_transport::control_identity_match(registry, &frame.identity),
            legion_core::MatchResult::Exact { .. }
        ) {
            return Err(
                "identity not matched unambiguously by existing compatibility registry".into(),
            );
        }
        let done = now();
        let duration = done.saturating_sub(begin);
        if duration > 3000 || done > due + 1000 {
            return Err(format!(
                "{context}, sample {index}: sample stale/over cadence; no retry"
            ));
        }
        if fingerprint
            .as_ref()
            .is_some_and(|known| known != &frame.source_fingerprint)
        {
            return Err(format!(
                "{context}, sample {index}: sensor/GPU/quirk binding changed; no automatic substitution"
            ));
        }
        fingerprint = Some(frame.source_fingerprint.clone());
        let phase = if index <= 60 {
            TrialPhase::BeforeWrite
        } else {
            TrialPhase::AfterAttempt
        };
        let sample = frame.sample(duration);
        let baseline_sample = baseline.as_ref().map(|f| f.sample(0));
        let assessment = if index == 60 {
            assess_trial_sample(&sample, TrialPhase::BeforeWrite, None)
        } else {
            assess_supervision_sample(
                &sample,
                phase,
                if index > 60 {
                    baseline_sample.as_ref()
                } else {
                    None
                },
            )
        };
        // AfterAttempt is the conservative POST policy used by the rehearsal,
        // not evidence of an actual attempt. Every event states write=false.
        log(&event(
            if index <= 60 {
                "baseline-sample"
            } else {
                if staged {
                    "post-preset-dispatch-sample"
                } else {
                    "post-no-write-marker-sample"
                }
            },
            index,
            serde_json::json!({"source":frame.source,"elapsed_ms":done-started,"acquisition_ms":duration,
                "readings":frame,"policy":assessment,"preset_endpoint_attempted":dispatch_attempted.get(),
                "FullSpeed_current":index==60}),
        ))?;
        let good = matches!(
            assessment.disposition,
            TrialDisposition::CandidateOnlyNotAuthorization
                | TrialDisposition::WithinObservationEnvelopeNotAcceptance
        );
        if index == 0 {
            baseline_anchor = done;
        }
        if !good && staged && index > 60 {
            // Detection after dispatch cannot cancel or roll back SFAN.
            // Keep observing the remaining POST window; fail the final result.
            post_anomaly = true;
        } else if !good {
            return Err(format!(
                "{}: {:?}",
                if index <= 60 {
                    "cancel-before-any-write"
                } else {
                    if staged {
                        "post-preset-anomaly; cannot-rollback-SFAN"
                    } else {
                        "post-marker-anomaly-no-write-occurred; cannot-rollback-SFAN"
                    }
                },
                assessment.reasons
            ));
        }
        if index <= 60 {
            let cpu = frame.cpu.value.ok_or("CPU missing")?;
            let gpu = frame.gpu.value.ok_or("GPU missing")?;
            // Reject warming from a previously observed minimum, not cooling
            // from an earlier maximum. Absolute limits were checked above.
            if cpu - cpu_minimum > 3.0 || gpu - gpu_minimum > 3.0 {
                return Err("baseline not stable: temperature rise exceeds 3 C from a prior minimum; do not restart automatically".into());
            }
            cpu_minimum = cpu_minimum.min(cpu);
            gpu_minimum = gpu_minimum.min(gpu);
        }
        if index == 60 {
            use crate::fan_control_transport::{LinuxMetadataCapture, recorded_control_preflight};
            let full_speed = frame
                .full_speed
                .clone()
                .ok_or("fresh boundary FanFullSpeed missing")?;
            let report = recorded_control_preflight(
                registry,
                &frame.identity,
                LinuxMetadataCapture {
                    observed_date: "see-runtime-monotonic-journal",
                    source: frame.source,
                    platform_profile: "custom",
                    ac_online: frame.ac_online,
                    rpm_sources: vec!["identified yogafan_83ex channels 1 and 2; unassigned"],
                    thermal_sources: vec![
                        "k10temp:Tctl",
                        "NVIDIA RTX 4060 Laptop GPU temperature.gpu",
                    ],
                },
            )
            .with_trial_supervision(
                frame.source,
                true,
                frame.ac_online,
                true,
                full_speed,
            );
            log(&event(
                "boundary-preflight-no-write",
                index,
                serde_json::to_value(report).map_err(|e| e.to_string())?,
            ))?;
            if let Some(dispatch) = preset.as_mut() {
                // Persist intent BEFORE dispatch. A lost reply is ambiguous,
                // never proof that firmware was not called; never retry.
                log(&event(
                    "preset-dispatch-intent",
                    index,
                    serde_json::json!({"preset":"1..10","response_is_not_EC_acceptance":true}),
                ))?;
                if now().saturating_sub(begin) > 3000 {
                    return Err("boundary evidence stale before preset dispatch".into());
                }
                dispatch_attempted.set(true);
                match dispatch() {
                    Ok(reply) => preset_reply = Some(reply),
                    Err(error) => preset_result = Some(Err(error)),
                }
            }
            log(&event(
                if preset.is_some() {
                    "PRESET-DISPATCH-MARKER"
                } else {
                    "NO-WRITE-MARKER"
                },
                index,
                serde_json::json!({"authorization":false,"preset_endpoint_attempted":dispatch_attempted.get(),"firmware_acceptance":"not-established"}),
            ))?;
            post_anchor = Some(now());
            baseline = Some(frame);
        }
        if let Some(reply) = preset_reply.as_ref() {
            let result = match reply.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(
                    "preset worker disconnected; outcome ambiguous; no retry".into(),
                )),
            };
            if let Some(result) = result {
                log(&event(
                    "preset-endpoint-result",
                    index,
                    serde_json::json!({"result":result,"not_readback":true}),
                ))?;
                preset_result = Some(result);
                preset_reply = None;
            }
        }
    }
    if dispatch_attempted.get() {
        match preset_result {
            Some(Ok(())) => {},
            Some(Err(error)) => return Err(format!("POST supervision finished; preset endpoint failed: {error}; no retry")),
            None => return Err("POST supervision finished; preset worker still pending; do not force unload; outcome ambiguous".into()),
        }
    }
    if post_anomaly {
        return Err(
            "POST window finished with anomalies; no rollback or acceptance established".into(),
        );
    }
    log(&event(
        if preset.is_some() {
            "completed-staged-default-trial"
        } else {
            "completed-no-write-rehearsal"
        },
        180,
        serde_json::json!({"samples":181,"FullSpeed_queries":1,"not_a_permanent_authorization":true}),
    ))
}

fn journal() -> Result<(PathBuf, File), String> {
    use rustix::fs::{CWD, Mode, OFlags, ResolveFlags, mkdirat, openat2};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .ok_or("repository root")?;
    let directory = openat2(
        CWD,
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(|e| e.to_string())?;
    match mkdirat(&directory, "trial-logs", Mode::RWXU) {
        Ok(()) => {}
        Err(rustix::io::Errno::EXIST) => {}
        Err(e) => return Err(e.to_string()),
    }
    let logs = openat2(
        &directory,
        "trial-logs",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(|e| e.to_string())?;
    if rustix::fs::fstat(&logs).map_err(|e| e.to_string())?.st_mode & 0o077 != 0 {
        return Err("existing trial-logs must be private (0700); no automatic chmod".into());
    }
    let name = format!(
        "supervision-{}-{}.jsonl",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos(),
        std::process::id()
    );
    let fd = openat2(
        &logs,
        name.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(|e| e.to_string())?;
    // Persist creation of both the log directory and its new directory entry.
    File::from(logs).sync_all().map_err(|e| e.to_string())?;
    File::from(directory)
        .sync_all()
        .map_err(|e| e.to_string())?;
    Ok((root.join("trial-logs").join(name), File::from(fd)))
}

fn persist(file: &mut File, value: &serde_json::Value) -> Result<(), String> {
    serde_json::to_writer(&mut *file, value).map_err(|e| e.to_string())?;
    file.write_all(b"\n")
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_data())
        .map_err(|e| format!("journal failure; stopping: {e}"))?;
    if matches!(
        value["event"].as_str(),
        Some("baseline-sample" | "post-no-write-marker-sample" | "post-preset-dispatch-sample")
    ) {
        let readings = &value["data"]["readings"];
        eprintln!(
            "[{}] sample={} CPU={} C GPU={} C RPM1={} RPM2={} AC={} profile={} policy={} PRESET_ATTEMPTED={}",
            value["source"],
            value["sample_index"],
            readings["cpu"]["value"],
            readings["gpu"]["value"],
            readings["fans"][0]["reported_speed"]["value"],
            readings["fans"][1]["reported_speed"]["value"],
            readings["ac_online"],
            readings["profile"]["normalized"]["active"]["name"],
            value["data"]["policy"]["disposition"],
            value["write_attempted"]
        );
    }
    Ok(())
}

fn finish(
    path: PathBuf,
    file: &mut File,
    result: Result<(), String>,
    source: &'static str,
    preset_attempted: bool,
) -> Result<RehearsalResult, String> {
    match result {
        Ok(()) => Ok(RehearsalResult {
            source,
            log_file: path,
            completed: true,
            write_attempted: preset_attempted,
            execution_enabled: preset_attempted,
        }),
        Err(error) => {
            let mut stopped = event(
                if preset_attempted {
                    "stopped-after-preset-dispatch"
                } else {
                    "stopped-no-write"
                },
                0,
                serde_json::json!({"source":source,"error":error,"preset_endpoint_attempted":preset_attempted,"no_retry":true}),
            );
            stopped["write_attempted"] = serde_json::json!(preset_attempted);
            stopped["execution_enabled"] = serde_json::json!(preset_attempted);
            let log_result = persist(file, &stopped);
            Err(format!(
                "{error}; journal={}; final-log={log_result:?}; preset_endpoint_attempted={preset_attempted}; no retry",
                path.display()
            ))
        }
    }
}

fn simulated_frame(boundary: bool) -> Frame {
    use legion_core::*;
    let provenance = TelemetryProvenance {
        artifact: TelemetryArtifact::StaticSimulation,
        source: "static-simulation".into(),
        confidence: ConfidenceLevel::Low,
    };
    let cpu = TelemetryValue {
        value: Some(43.5),
        unit: TelemetryUnit::Celsius,
        classification: ValueClassification::InstantaneousMeasurement,
        state: TelemetryState::Available,
        provenance: provenance.clone(),
    };
    let mut gpu = cpu.clone();
    gpu.value = Some(34.0); // A fixture, NOT a fresh hardware observation.
    let fan = |channel| FanTelemetry {
        fan: format!("fixture-quirk-channel{channel}"),
        reported_speed: TelemetryValue {
            value: Some(1500),
            unit: TelemetryUnit::Rpm,
            classification: ValueClassification::InstantaneousMeasurement,
            state: TelemetryState::Available,
            provenance: provenance.clone(),
        },
        validation: FanValidation::Validated,
        interpretation: FanInterpretation::ReportedValue,
    };
    let profile = PlatformProfileAdapter
        .parse(PlatformProfileInput {
            raw: PlatformProfileRawInput {
                active: Some("custom\n".into()),
                choices: Some("balanced performance custom\n".into()),
            },
            provenance: provenance.clone(),
        })
        .expect("fixture profile");
    let identity = HostIdentity::declared_simulation(
        HostIdentityInput {
            manufacturer: Some("Lenovo".into()),
            product_identifier: Some("83EX".into()),
            model: Some("Legion Slim 5 16ARP9".into()),
            bios_version: Some("NUCN15WW".into()),
            ..Default::default()
        },
        "monitor-fixture",
    )
    .expect("fixture identity");
    let mut bytes = [0; 20];
    bytes[0] = 1;
    bytes[2] = 12;
    bytes[8] = 8;
    Frame {
        source: "static-simulation; virtual-time; no-host-access",
        identity,
        cpu,
        gpu,
        fans: [fan(1), fan(2)],
        profile,
        ac_online: true,
        full_speed: boundary
            .then(|| crate::fan_full_speed::FullSpeedObservation::simulation(&bytes)),
        evidence: serde_json::json!({"all_inputs":"in-memory-fixture-not-historical-readings"}),
        source_fingerprint: "fixture-sources".into(),
    }
}

pub fn run_simulated_rehearsal(
    registry: &CompatibilityRegistry,
) -> Result<RehearsalResult, String> {
    let (path, mut file) = journal()?;
    eprintln!(
        "SIMULATION / virtual time; durable journal: {}",
        path.display()
    );
    let clock = std::cell::Cell::new(0);
    let result = supervise(
        registry,
        "static-simulation; virtual-time; no-host-access",
        |boundary| Ok(simulated_frame(boundary)),
        || clock.get(),
        |delay| clock.set(clock.get() + delay),
        |value| persist(&mut file, value),
        None,
    );
    finish(
        path,
        &mut file,
        result,
        "static-simulation; virtual-time",
        false,
    )
}

/// Strict parser for the fixed NVIDIA query: exactly one RTX 4060 Laptop GPU,
/// PCI identity plus integer Celsius. Acquisition recency is timed separately;
/// NVIDIA does not expose a silicon sample timestamp in this output.
pub fn parse_nvidia_temperature(bytes: &[u8]) -> Result<(String, f64), String> {
    if bytes.len() > 512 || !bytes.is_ascii() {
        return Err("NVIDIA output too large".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let line = text.strip_suffix('\n').unwrap_or(text);
    if line.chars().any(|c| c.is_control()) {
        return Err("NVIDIA output has extra lines/control characters".into());
    }
    let fields: Vec<_> = line.split(',').map(str::trim).collect();
    if fields.len() != 3 || fields[1] != "NVIDIA GeForce RTX 4060 Laptop GPU" {
        return Err("exactly one identified RTX 4060 Laptop GPU required".into());
    }
    let pci = fields[0];
    let parts: Vec<_> = pci.split(':').collect();
    let hex = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_hexdigit());
    if parts.len() != 3
        || !(hex(parts[0], 4) || hex(parts[0], 8))
        || !hex(parts[1], 2)
        || parts[2].len() != 4
        || parts[2].as_bytes()[2] != b'.'
        || !hex(&parts[2][..2], 2)
        || !matches!(parts[2].as_bytes()[3], b'0'..=b'7')
    {
        return Err("invalid NVIDIA PCI identity".into());
    }
    if fields[2].is_empty() || !fields[2].bytes().all(|c| c.is_ascii_digit()) {
        return Err("NVIDIA temperature unavailable or malformed".into());
    }
    let temperature = fields[2].parse::<u32>().map_err(|e| e.to_string())?;
    if !(5..=110).contains(&temperature) {
        return Err("GPU temperature outside trial range".into());
    }
    Ok((pci.to_ascii_lowercase(), f64::from(temperature)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn staged_preset_dispatch_is_once_and_post_monitoring_does_not_wait_for_wmi() {
        use std::{cell::RefCell, sync::mpsc};
        for scenario in 0..8 {
            let registry = legion_fixtures::legion_slim_5_16arp9_registry();
            let clock = Cell::new(0);
            let calls = Cell::new(0);
            let dispatches = Cell::new(0);
            let pending = RefCell::new(None::<mpsc::Sender<Result<(), String>>>);
            let mut records = Vec::new();
            let mut dispatch = || {
                dispatches.set(dispatches.get() + 1);
                if scenario == 6 {
                    return Err("mock dispatch error; no retry".into());
                }
                let (tx, rx) = mpsc::channel();
                *pending.borrow_mut() = Some(tx);
                Ok(rx)
            };
            let result = supervise(
                &registry,
                "static-simulation; preset-mocked",
                |boundary| {
                    let index = calls.get();
                    calls.set(index + 1);
                    if index == 90 && scenario != 2 {
                        if let Some(tx) = pending.borrow_mut().take() {
                            tx.send(if scenario == 1 {
                                Err("mock WMI error".into())
                            } else {
                                Ok(())
                            })
                            .unwrap();
                        }
                    }
                    let mut frame = simulated_frame(boundary);
                    if scenario == 7 && index == 90 {
                        return Err("mock sensor read failure".into());
                    }
                    if scenario == 5 && index == 61 {
                        frame.cpu.value = Some(75.0);
                    }
                    if scenario == 3 {
                        frame.cpu.value = Some(56.0);
                    }
                    if scenario == 4 {
                        frame.full_speed = None;
                    }
                    Ok(frame)
                },
                || clock.get(),
                |delay| clock.set(clock.get() + delay),
                |value| {
                    records.push(value.clone());
                    Ok(())
                },
                Some(&mut dispatch),
            );
            if (3..=4).contains(&scenario) {
                assert_eq!(dispatches.get(), 0);
                assert!(result.is_err());
            } else {
                assert_eq!(dispatches.get(), 1);
                assert_eq!(calls.get(), 181);
                assert_eq!(result.is_ok(), scenario == 0);
                assert!(records.iter().any(|r| r["sample_index"] == 180));
                assert_eq!(
                    records
                        .iter()
                        .filter(|r| r["event"] == "preset-dispatch-intent")
                        .count(),
                    1
                );
            }
        }
    }

    fn exercise(
        mut alter: impl FnMut(usize, &mut Frame),
        slow_at: Option<usize>,
        fail_log: bool,
    ) -> (Result<(), String>, usize, Vec<serde_json::Value>) {
        let registry = legion_fixtures::legion_slim_5_16arp9_registry();
        let clock = Cell::new(0);
        let mut calls = 0;
        let mut records = Vec::new();
        let result = supervise(
            &registry,
            "static-simulation; virtual-time",
            |boundary| {
                let index = calls;
                calls += 1;
                assert_eq!(boundary, index == 60);
                let mut frame = simulated_frame(boundary);
                alter(index, &mut frame);
                if slow_at == Some(index) {
                    clock.set(clock.get() + 3001);
                }
                Ok(frame)
            },
            || clock.get(),
            |delay| clock.set(clock.get() + delay),
            |value| {
                if fail_log && !records.is_empty() {
                    return Err("simulated journal failure".into());
                }
                records.push(value.clone());
                Ok(())
            },
            None,
        );
        (result, calls, records)
    }

    #[test]
    fn bounded_virtual_rehearsal_checks_boundary_without_a_writer() {
        let (result, calls, records) = exercise(|_, _| {}, None, false);
        result.unwrap();
        assert_eq!(calls, 181);
        assert_eq!(
            records
                .iter()
                .filter(|r| r["event"] == "NO-WRITE-MARKER")
                .count(),
            1
        );
        assert_eq!(
            records.last().unwrap()["event"],
            "completed-no-write-rehearsal"
        );
        for record in &records {
            assert_eq!(record["write_attempted"], false);
            assert_eq!(record["execution_enabled"], false);
            assert!(
                record["source"]
                    .as_str()
                    .unwrap()
                    .starts_with("static-simulation")
            );
        }
        let gate = records
            .iter()
            .find(|r| r["event"] == "boundary-preflight-no-write")
            .unwrap();
        assert_eq!(gate["data"]["execution_enabled"], false);
        assert_eq!(
            gate["data"]["checks"][5]["status"],
            "CPU/GPU-modelled-in-simulation-only"
        );
        assert_eq!(
            gate["data"]["full_speed_observation"]["source"],
            "static-simulation"
        );
        let post = records
            .iter()
            .find(|r| r["sample_index"] == 61 && r["event"] == "post-no-write-marker-sample")
            .unwrap();
        assert_eq!(post["data"]["FullSpeed_current"], false);
        assert!(post["data"]["readings"]["full_speed"].is_null());
    }

    #[test]
    fn baseline_gradual_cooling_is_allowed() {
        let temperatures = [36.875, 36.25, 35.625, 35.0, 34.375, 33.75];
        let (result, calls, _) = exercise(
            |index, frame| {
                frame.cpu.value = Some(temperatures[index.min(temperatures.len() - 1)]);
                frame.gpu.value = Some(32.0);
            },
            None,
            false,
        );
        result.unwrap();
        assert_eq!(calls, 181);
    }

    #[test]
    fn baseline_warming_over_three_from_prior_minimum_is_rejected() {
        for cpu in [true, false] {
            let temperatures = if cpu {
                [43.5, 40.0, 43.0, 43.125]
            } else {
                [34.0, 30.0, 33.0, 33.125]
            };
            let (result, calls, records) = exercise(
                |index, frame| {
                    let value = Some(temperatures[index]);
                    if cpu {
                        frame.cpu.value = value;
                    } else {
                        frame.gpu.value = value;
                    }
                },
                None,
                false,
            );
            assert!(
                result
                    .unwrap_err()
                    .contains("rise exceeds 3 C from a prior minimum")
            );
            // Cooling and exactly +3 C pass; +3.125 C aborts without retry.
            assert_eq!(calls, 4);
            assert!(!records.iter().any(|r| r["event"] == "NO-WRITE-MARKER"));
        }
    }

    #[test]
    fn baseline_absolute_temperature_limits_remain_enforced() {
        let (result, calls, _) = exercise(
            |_, frame| {
                frame.cpu.value = Some(55.0);
                frame.gpu.value = Some(50.0);
            },
            None,
            false,
        );
        result.unwrap();
        assert_eq!(calls, 181);
        for cpu in [true, false] {
            let (result, calls, records) = exercise(
                |_, frame| {
                    if cpu {
                        frame.cpu.value = Some(55.125);
                    } else {
                        frame.gpu.value = Some(50.125);
                    }
                },
                None,
                false,
            );
            assert!(
                result
                    .unwrap_err()
                    .contains("temperature outside proposed trial envelope")
            );
            assert_eq!(calls, 1);
            assert!(!records.iter().any(|r| r["event"] == "NO-WRITE-MARKER"));
        }
    }

    #[test]
    fn missing_or_hot_gpu_original_quirk_lost_ac_and_changed_sources_stop_before_boundary() {
        for scenario in 0..6 {
            let (result, calls, records) = exercise(
                |index, frame| {
                    if index == 1 {
                        match scenario {
                            0 => frame.gpu.value = None,
                            1 => frame.gpu.value = Some(60.0),
                            2 => frame.fans[0].validation = legion_core::FanValidation::Unvalidated,
                            3 => frame.ac_online = false,
                            4 => frame.source_fingerprint = "different GPU/module".into(),
                            _ => frame.profile.state = legion_core::TelemetryState::Unvalidated,
                        }
                    }
                },
                None,
                false,
            );
            assert!(result.is_err());
            assert_eq!(calls, 2);
            assert!(!records.iter().any(|r| r["event"] == "NO-WRITE-MARKER"));
        }
    }

    #[test]
    fn unknown_enabled_or_stale_full_speed_never_passes_boundary() {
        for scenario in 0..3 {
            let (result, calls, records) = exercise(
                |index, frame| {
                    if index == 60 {
                        if scenario == 0 {
                            frame.full_speed = None;
                        } else {
                            let mut raw = [0; 20];
                            raw[0] = 1;
                            raw[2] = 12;
                            raw[8] = 8;
                            raw[12] = if scenario == 1 { 1 } else { 9 };
                            frame.full_speed = Some(
                                crate::fan_full_speed::FullSpeedObservation::simulation(&raw),
                            );
                        }
                    }
                },
                None,
                false,
            );
            assert!(result.is_err());
            assert_eq!(calls, 61);
            assert!(!records.iter().any(|r| r["event"] == "NO-WRITE-MARKER"));
        }
        assert!(exercise(|_, _| {}, Some(60), false).0.is_err());
    }

    #[test]
    fn post_marker_anomaly_journal_failure_and_slow_read_do_not_retry() {
        let (result, calls, records) = exercise(
            |index, frame| {
                if index == 61 {
                    frame.cpu.value = Some(75.0)
                }
            },
            None,
            false,
        );
        assert!(result.unwrap_err().contains("post-marker-anomaly"));
        assert_eq!(calls, 62);
        assert_eq!(
            records
                .iter()
                .filter(|r| r["event"] == "NO-WRITE-MARKER")
                .count(),
            1
        );
        assert_eq!(exercise(|_, _| {}, None, true).1, 0);
        let (result, calls, _) = exercise(|_, _| {}, Some(0), false);
        assert!(result.is_err());
        assert_eq!(calls, 1);
    }

    #[test]
    fn nvidia_requires_one_identified_freshly_queried_numeric_gpu() {
        let good = b"00000000:01:00.0, NVIDIA GeForce RTX 4060 Laptop GPU, 34\n";
        assert_eq!(
            parse_nvidia_temperature(good).unwrap(),
            ("00000000:01:00.0".into(), 34.0)
        );
        for bad in [
            "",
            "00000000:01:00.0, NVIDIA GeForce RTX 4060 Laptop GPU, N/A\n",
            "00000000:01:00.0, NVIDIA GeForce RTX 4060 Laptop GPU, NaN\n",
            "00000000:01:00.0, other GPU, 34\n",
            "::::::::::::, NVIDIA GeForce RTX 4060 Laptop GPU, 34\n",
            "00000000:01:00.0, NVIDIA GeForce RTX 4060 Laptop GPU, 34\nextra",
        ] {
            assert!(parse_nvidia_temperature(bad.as_bytes()).is_err());
        }
        assert!(parse_nvidia_temperature(&[b' '; 513]).is_err());
    }
}
