//! Native, read-only status dashboard.
//!
//! This binary owns no hardware reader. It moves the existing
//! `StatusReader` into a worker thread and only presents its snapshots.

use eframe::egui;
use legion_core::{FanInterpretation, FanValidation, TelemetryState};
use legion_linux::{
    HwmonFanReading,
    status::{LinuxStatus, StatusReader},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

const REFRESH_INTERVAL: Duration = Duration::from_secs(2);

enum SnapshotMessage {
    Status(LinuxStatus),
    Error(String),
}

struct StatusWorker {
    receiver: Receiver<SnapshotMessage>,
    stop: Arc<AtomicBool>,
}

impl StatusWorker {
    fn start() -> Self {
        let (sender, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        thread::spawn(move || {
            let reader = match StatusReader::open() {
                Ok(reader) => reader,
                Err(error) => {
                    let _ = sender.send(SnapshotMessage::Error(error));
                    return;
                }
            };

            while !worker_stop.load(Ordering::Acquire) {
                let started = Instant::now();
                let message = match reader.read_once() {
                    Ok(status) => SnapshotMessage::Status(status),
                    Err(error) => SnapshotMessage::Error(error),
                };
                if sender.send(message).is_err() {
                    return;
                }

                let remaining = REFRESH_INTERVAL.saturating_sub(started.elapsed());
                let until = Instant::now() + remaining;
                while Instant::now() < until {
                    if worker_stop.load(Ordering::Acquire) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
        });
        Self { receiver, stop }
    }

    fn receive_latest(&self) -> Option<SnapshotMessage> {
        let mut latest = None;
        while let Ok(message) = self.receiver.try_recv() {
            latest = Some(message);
        }
        latest
    }
}

impl Drop for StatusWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

struct LegionApp {
    worker: StatusWorker,
    status: Option<LinuxStatus>,
    error: Option<String>,
}

impl LegionApp {
    fn new() -> Self {
        Self {
            worker: StatusWorker::start(),
            status: None,
            error: None,
        }
    }

    fn update_from_worker(&mut self) {
        if let Some(message) = self.worker.receive_latest() {
            match message {
                SnapshotMessage::Status(status) => {
                    self.status = Some(status);
                    self.error = None;
                }
                SnapshotMessage::Error(error) => self.error = Some(error),
            }
        }
    }
}

impl eframe::App for LegionApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_from_worker();
        ctx.request_repaint_after(Duration::from_millis(100));

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("LegionLinuxToolkit");
            ui.colored_label(egui::Color32::from_rgb(230, 180, 70), "Solo lectura");
            ui.label("Actualización aproximada: cada 2 segundos. No modifica perfiles, ventiladores ni firmware.");
            ui.separator();

            if let Some(status) = &self.status {
                status_panel(ui, status);
            } else {
                ui.label("Esperando la primera instantánea de estado…");
            }
            if let Some(error) = &self.error {
                ui.separator();
                ui.colored_label(egui::Color32::from_rgb(220, 90, 90), format!("Error de adquisición: {error}"));
                ui.label("No se reutilizan valores anteriores como si fueran actuales.");
            }
        });
    }
}

fn status_panel(ui: &mut egui::Ui, status: &LinuxStatus) {
    egui::Grid::new("status-grid")
        .num_columns(2)
        .spacing([16.0, 8.0])
        .show(ui, |ui| {
            ui.label("CPU Tctl:");
            ui.label(temperature_text(&status.cpu_tctl));
            ui.end_row();
            ui.label("GPU NVIDIA:");
            ui.label(temperature_text(&status.gpu_temperature));
            ui.end_row();
            ui.label("Alimentación:");
            ui.label(match &status.ac_online {
                Ok(true) => "AC".to_owned(),
                Ok(false) => "Batería".to_owned(),
                Err(error) => format!("No disponible: {error}"),
            });
            ui.end_row();
            ui.label("Perfil:");
            ui.label(match &status.platform_profile {
                Ok(profile) => profile
                    .normalized
                    .active
                    .as_ref()
                    .map(|active| active.name.clone())
                    .unwrap_or_else(|| "No disponible".to_owned()),
                Err(error) => format!("No disponible: {error}"),
            });
            ui.end_row();
        });

    ui.separator();
    ui.heading("Ventiladores (RPM reportadas)");
    match &status.fans {
        Ok(fans) if fans.is_empty() => {
            ui.label("No hay entradas RPM disponibles.");
        }
        Ok(fans) => {
            for fan in fans {
                ui.label(fan_text(fan));
            }
        }
        Err(error) => {
            ui.colored_label(
                egui::Color32::from_rgb(220, 90, 90),
                format!("RPM no disponibles: {error}"),
            );
        }
    }

    ui.separator();
    ui.heading("Advertencias");
    for warning in &status.warnings {
        ui.label(format!("• {warning}"));
    }
}

fn temperature_text(value: &Result<legion_core::TelemetryValue<f64>, String>) -> String {
    match value {
        Ok(reading) if reading.state == TelemetryState::Available => reading
            .value
            .map(|celsius| format!("{celsius:.1} °C"))
            .unwrap_or_else(|| "No disponible".to_owned()),
        Ok(reading) => format!("No disponible ({:?})", reading.state),
        Err(error) => format!("No disponible: {error}"),
    }
}

fn fan_text(fan: &HwmonFanReading) -> String {
    let rpm = fan
        .rpm
        .value
        .map(|rpm| format!("{rpm} RPM"))
        .unwrap_or_else(|| "No disponible".to_owned());
    let reliability = match fan.validation {
        FanValidation::Validated => "validada",
        _ => "no validada",
    };
    let interpretation = match fan.interpretation {
        FanInterpretation::NonConclusive => "no concluyente",
        _ => "sin interpretación concluyente",
    };
    format!(
        "{} / fan{}_input: {rpm} — fuente: {:?}; fiabilidad {reliability}; {interpretation}",
        fan.driver_name, fan.channel, fan.module_provenance
    )
}

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "LegionLinuxToolkit",
        eframe::NativeOptions::default(),
        Box::new(|_creation_context| Ok(Box::new(LegionApp::new()))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_core::{
        ConfidenceLevel, TelemetryArtifact, TelemetryProvenance, TelemetryUnit, TelemetryValue,
        ValueClassification,
    };
    use legion_linux::{HwmonDriverClassification, HwmonModuleProvenance};

    fn rpm(value: Option<u32>) -> TelemetryValue<u32> {
        TelemetryValue {
            value,
            unit: TelemetryUnit::Rpm,
            classification: ValueClassification::InstantaneousMeasurement,
            state: TelemetryState::Available,
            provenance: TelemetryProvenance {
                artifact: TelemetryArtifact::StaticSimulation,
                source: "fixture".into(),
                confidence: ConfidenceLevel::Low,
            },
        }
    }

    #[test]
    fn original_yogafan_zero_remains_non_conclusive_in_the_view() {
        let fan = HwmonFanReading {
            driver_name: "yogafan".into(),
            device_id: "fixture".into(),
            channel: 1,
            source_classification: HwmonDriverClassification::Yogafan83ExCandidateUnverified,
            module_provenance: HwmonModuleProvenance::OriginalYogafan {
                module_name: "yogafan".into(),
            },
            rpm: rpm(Some(0)),
            validation: FanValidation::Unvalidated,
            interpretation: FanInterpretation::NonConclusive,
        };
        let text = fan_text(&fan);
        assert!(text.contains("0 RPM"));
        assert!(text.contains("no validada"));
        assert!(text.contains("no concluyente"));
    }

    #[test]
    fn unavailable_temperature_is_never_rendered_as_a_number() {
        assert_eq!(
            temperature_text(&Err("fixture missing".into())),
            "No disponible: fixture missing"
        );
    }
}
