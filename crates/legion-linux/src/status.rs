//! Explicit single Linux status snapshot, no monitor, writer or module loader.
use crate::{ApprovedRealReadActivation, PlatformProfileQuery, SysfsPlatformProfileProvider};
use legion_core::{
    ConfidenceLevel, FanValidation, PlatformProfileReading, TelemetryArtifact, TelemetryProvenance,
    TelemetryState, TelemetryUnit, TelemetryValue, ValueClassification,
};
use serde::Serialize;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Serialize)]
pub struct LinuxStatus {
    pub source: &'static str,
    pub observed_unix_ms: u128,
    pub acquisition_ms: u128,
    pub cpu_tctl: Result<TelemetryValue<f64>, String>,
    pub gpu_temperature: Result<TelemetryValue<f64>, String>,
    pub ac_online: Result<bool, String>,
    pub ac_source: &'static str,
    pub platform_profile: Result<PlatformProfileReading, String>,
    pub fans: Result<Vec<crate::HwmonFanReading>, String>,
    pub warnings: Vec<String>,
}

fn temperature(celsius: f64, source: String) -> TelemetryValue<f64> {
    TelemetryValue {
        value: Some(celsius),
        unit: TelemetryUnit::Celsius,
        classification: ValueClassification::InstantaneousMeasurement,
        state: TelemetryState::Available,
        provenance: TelemetryProvenance {
            artifact: TelemetryArtifact::LinuxReadOnly,
            source,
            confidence: ConfidenceLevel::Low,
        },
    }
}

fn fan_warnings(fans: &Result<Vec<crate::HwmonFanReading>, String>) -> Vec<String> {
    let mut warnings = vec!["Instantánea no atómica; no sondeo ni reintentos. NVIDIA no informa la edad física de su sensor.".into()];
    if let Ok(fans) = fans {
        if fans.iter().any(|f| f.driver_name == "acpi_fan") {
            warnings.push("acpi_fan no representa necesariamente ambos ventiladores; canales sin asignación CPU/GPU.".into());
        }
        if fans
            .iter()
            .any(|f| f.driver_name == "yogafan" && f.validation != FanValidation::Validated)
        {
            warnings.push("yogafan original/no verificado no es fiable en 83EX: su 0 RPM NO demuestra ventiladores detenidos.".into());
        }
    }
    warnings
}

/// Calling this function explicitly requests exactly one status acquisition.
/// DMI is observed only to enforce the existing 83EX/NUCN15WW gate; never
/// emits RealReadAuthorization. No paths/configuration/confirmation bypass.
pub fn read_once() -> Result<LinuxStatus, String> {
    static USED: AtomicBool = AtomicBool::new(false);
    USED.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "one status snapshot per process; no retry")?;
    let started = Instant::now();
    let identity = crate::approved_one_shot_dmi_identity_read("I-UNDERSTAND-READ-ONLY-DMI")
        .map_err(|e| format!("status DMI gate: {e}"))?;
    if !crate::hwmon_real::is_exact_target_identity(&identity.identity) {
        return Err(
            "status supports only observed Lenovo 83EX / NUCN15WW; no hardware authorization"
                .into(),
        );
    }
    let root = crate::open_fixed_sysfs_root().map_err(|e| format!("status sysfs: {e:?}"))?;
    let source = crate::hwmon_real::ConfinedHwmonSource::from_linux_root(root);
    let cpu_tctl = source
        .read_status_cpu_once()
        .map(|(c, _, name)| temperature(c, name));
    let ac_online = source.read_ac_once().map(|(online, _)| online);
    let fans = source.read_once(true, None).map_err(|e| e.to_string());
    let platform_profile =
        SysfsPlatformProfileProvider::new(ApprovedRealReadActivation::ApprovedOneShot)
            .map_err(|e| format!("profile open: {e:?}"))
            .and_then(|provider| provider.read_once().map_err(|e| e.to_string()))
            .map(|reading| reading.profile().clone());
    let gpu_temperature = crate::nvidia::nvidia_once()
        .and_then(|bytes| crate::nvidia::parse_nvidia_temperature(&bytes))
        .map(|(pci, c)| {
            temperature(
                c,
                format!("NVIDIA:{pci}:temperature.gpu; current-query; hardware-cache-age-unknown"),
            )
        });
    let warnings = fan_warnings(&fans);
    Ok(LinuxStatus {
        source: "linux-read-only-single-status",
        acquisition_ms: started.elapsed().as_millis(),
        observed_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis(),
        cpu_tctl,
        gpu_temperature,
        ac_online,
        ac_source: "ACAD/type=Mains; ACAD/online",
        platform_profile,
        fans,
        warnings,
    })
}

impl LinuxStatus {
    pub fn to_text(&self) -> String {
        let temp = |reading: &Result<TelemetryValue<f64>, String>| match reading {
            Ok(v) => match v.value {
                Some(c) => format!("{c:.3} °C [{}]", v.provenance.source),
                None => "no disponible (dato ausente)".into(),
            },
            Err(error) => format!("no disponible: {error}"),
        };
        let mut text = format!(
            "Legion 83EX — STATUS solo lectura ({})\nCPU Tctl: {}\nGPU: {}\nAC [{}]: {}\n",
            self.source,
            temp(&self.cpu_tctl),
            temp(&self.gpu_temperature),
            self.ac_source,
            match &self.ac_online {
                Ok(true) => "conectada".into(),
                Ok(false) => "desconectada".into(),
                Err(e) => format!("no disponible: {e}"),
            }
        );
        match &self.platform_profile {
            Ok(p) => text.push_str(&format!(
                "Perfil: {} ({:?})\n",
                p.normalized
                    .active
                    .as_ref()
                    .map(|p| p.name.as_str())
                    .unwrap_or("ausente"),
                p.state
            )),
            Err(e) => text.push_str(&format!("Perfil no disponible: {e}\n")),
        }
        match &self.fans {
            Ok(fans) if fans.is_empty() => text.push_str("RPM: sin entradas disponibles\n"),
            Ok(fans) => {
                for f in fans {
                    text.push_str(&format!(
                        "{} {}/fan{}_input: {} RPM reportadas; {:?}, {:?}; módulo {:?}\n",
                        f.driver_name,
                        f.device_id,
                        f.channel,
                        f.rpm
                            .value
                            .map(|v| v.to_string())
                            .unwrap_or("ausente".into()),
                        f.validation,
                        f.interpretation,
                        f.module_provenance
                    ));
                }
            }
            Err(e) => text.push_str(&format!("RPM no disponibles: {e}\n")),
        }
        for warning in &self.warnings {
            text.push_str(&format!("Aviso: {warning}\n"));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_zero_and_generic_acpi_remain_explicitly_non_conclusive() {
        use crate::hwmon::{HwmonReadingOrigin, build_reading};
        use crate::{HwmonDriverClassification as Class, HwmonModuleProvenance as Module};
        let fan = |driver, rpm, classification| {
            build_reading(
                driver,
                "fixture".into(),
                1,
                rpm,
                classification,
                Module::StaticSimulation,
                HwmonReadingOrigin::StaticSimulation {
                    root_label: "status-fixture",
                },
            )
        };
        let fans = Ok(vec![
            fan("yogafan".into(), 0, Class::Yogafan83ExCandidateUnverified),
            fan("acpi_fan".into(), 1500, Class::GenericUnvalidated),
        ]);
        let snapshot = LinuxStatus {
            source: "static-simulation",
            observed_unix_ms: 0,
            acquisition_ms: 0,
            cpu_tctl: Err("fixture permission denied".into()),
            gpu_temperature: Err("fixture NVIDIA N/A".into()),
            ac_online: Ok(false),
            ac_source: "fixture AC",
            platform_profile: Err("fixture absent".into()),
            warnings: fan_warnings(&fans),
            fans,
        };
        let text = snapshot.to_text();
        assert!(text.contains("NO demuestra ventiladores detenidos"));
        assert!(text.contains("no representa necesariamente ambos"));
        assert!(text.contains("acpi_fan"));
        assert!(text.contains("desconectada"));
        let json = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(json["fans"]["Ok"][0]["rpm"]["value"], 0);
        assert_eq!(json["fans"]["Ok"][0]["validation"], "unvalidated");
        assert!(json["cpu_tctl"]["Err"].is_string());
    }
}
