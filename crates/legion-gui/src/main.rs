//! Native read-only dashboard and offline curve preview.
//!
//! Hardware status is obtained solely through the existing `StatusReader`.
//! Curve documents reuse the shared offline LLT V2 model and expose no write
//! transport, control action, or hardware endpoint.

use eframe::egui;
use legion_core::{FanInterpretation, FanValidation, TelemetryState};
use legion_fan::{OfflineCurveFile, OfflineCurvePreview, preview_83ex_fnt0};
use legion_linux::{
    HwmonDriverClassification, HwmonFanReading, HwmonModuleProvenance,
    status::{LinuxStatus, StatusReader},
};
use std::{
    path::Path,
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
                let until = Instant::now() + REFRESH_INTERVAL.saturating_sub(started.elapsed());
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Dashboard,
    Curves,
}

struct CurveEditor {
    indices: [u16; 10],
    path: String,
    notice: Option<String>,
}

impl CurveEditor {
    fn new() -> Self {
        Self {
            indices: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            path: "mi-curva.json".into(),
            notice: None,
        }
    }

    fn document(&self) -> OfflineCurveFile {
        OfflineCurveFile::new(self.indices)
    }

    fn load(&mut self) {
        let result = (|| {
            let path =
                std::path::absolute(Path::new(self.path.trim())).map_err(|e| e.to_string())?;
            let bytes =
                legion_linux::read_offline_regular_file(&path, 4096).map_err(|e| e.to_string())?;
            let file = OfflineCurveFile::from_json(&bytes)?;
            self.indices = *file.indices();
            Ok::<_, String>(format!("Curva offline cargada desde {}", path.display()))
        })();
        self.notice = Some(match result {
            Ok(message) => message,
            Err(error) => format!("No se pudo abrir la curva: {error}"),
        });
    }

    fn save(&mut self) {
        let result = (|| {
            let document = self.document();
            document.validate()?;
            let path =
                std::path::absolute(Path::new(self.path.trim())).map_err(|e| e.to_string())?;
            legion_linux::save_offline_curve_json(&path, &document.to_pretty_json()?)?;
            Ok::<_, String>(format!("Curva offline guardada en {}", path.display()))
        })();
        self.notice = Some(match result {
            Ok(message) => message,
            Err(error) => format!("No se pudo guardar la curva: {error}"),
        });
    }
}

struct LegionApp {
    worker: StatusWorker,
    status: Option<LinuxStatus>,
    error: Option<String>,
    tab: Tab,
    curves: CurveEditor,
}

impl LegionApp {
    fn new() -> Self {
        Self {
            worker: StatusWorker::start(),
            status: None,
            error: None,
            tab: Tab::Dashboard,
            curves: CurveEditor::new(),
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
            ui.horizontal(|ui| {
                ui.colored_label(egui::Color32::from_rgb(230, 180, 70), "Solo lectura");
                ui.separator();
                ui.strong("Control de hardware: BLOQUEADO");
            });
            ui.label("No cambia perfiles, ventiladores ni firmware.");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Dashboard, "Dashboard");
                ui.selectable_value(&mut self.tab, Tab::Curves, "Curvas");
            });
            ui.separator();
            match self.tab {
                Tab::Dashboard => dashboard(ui, self.status.as_ref(), self.error.as_deref()),
                Tab::Curves => curves(ui, &mut self.curves),
            }
        });
    }
}

fn card(ui: &mut egui::Ui, title: &str, value: impl AsRef<str>) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(170.0);
        ui.small(title);
        ui.heading(value.as_ref());
    });
}

fn dashboard(ui: &mut egui::Ui, status: Option<&LinuxStatus>, error: Option<&str>) {
    if let Some(status) = status {
        ui.horizontal_wrapped(|ui| {
            card(ui, "CPU Tctl", temperature_text(&status.cpu_tctl));
            card(ui, "GPU NVIDIA", temperature_text(&status.gpu_temperature));
            card(ui, "Alimentación", ac_text(&status.ac_online));
            card(ui, "Perfil activo", profile_text(&status.platform_profile));
        });
        ui.add_space(12.0);
        ui.heading("Ventiladores");
        match &status.fans {
            Ok(fans) if fans.is_empty() => {
                ui.label("No hay entradas RPM disponibles.");
            }
            Ok(fans) => {
                for fan in fans {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.strong(format!("{} — {}", friendly_source(fan), rpm_text(fan)));
                        ui.small(friendly_reliability(fan));
                        ui.collapsing("Detalles técnicos", |ui| {
                            ui.label(format!(
                                "Canal: fan{}_input (sin asignación CPU/GPU)",
                                fan.channel
                            ));
                            ui.label(format!("Procedencia: {:?}", fan.module_provenance));
                            ui.label(format!("Clasificación: {:?}", fan.source_classification));
                        });
                    });
                }
            }
            Err(error) => {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 90, 90),
                    format!("RPM no disponibles: {error}"),
                );
            }
        }
        ui.collapsing("Advertencias y límites", |ui| {
            for warning in &status.warnings {
                ui.label(format!("• {warning}"));
            }
        });
    } else {
        ui.label("Esperando la primera instantánea de estado…");
    }
    if let Some(error) = error {
        ui.separator();
        ui.colored_label(
            egui::Color32::from_rgb(220, 90, 90),
            format!("Error de adquisición: {error}"),
        );
        ui.label("No se reutilizan valores anteriores como si fueran actuales.");
    }
}

fn curves(ui: &mut egui::Ui, editor: &mut CurveEditor) {
    ui.heading("Curva personalizada offline");
    ui.label("LLT GodMode V2 / FNT0. Es una previsualización; no representa el estado actual del firmware.");
    ui.horizontal(|ui| {
        ui.label("Archivo JSON:");
        ui.text_edit_singleline(&mut editor.path);
        if ui.button("Abrir").clicked() {
            editor.load();
        }
        if ui.button("Guardar como nuevo").clicked() {
            editor.save();
        }
    });
    if let Some(notice) = &editor.notice {
        ui.label(notice);
    }

    let preview = editor
        .document()
        .validate()
        .and_then(|proposal| preview_83ex_fnt0(&proposal));
    egui::Grid::new("curve-points")
        .striped(true)
        .show(ui, |ui| {
            ui.strong("Punto");
            ui.strong("Índice LLT");
            ui.strong("RPM nominal FNT0");
            ui.end_row();
            for index in 0..10 {
                ui.label((index + 1).to_string());
                ui.add(egui::DragValue::new(&mut editor.indices[index]).range(1..=10));
                if let Ok(preview) = &preview {
                    ui.label(format!("{} RPM", preview.points[index].nominal_rpm));
                } else {
                    ui.label("—");
                }
                ui.end_row();
            }
        });

    match editor
        .document()
        .validate()
        .and_then(|proposal| preview_83ex_fnt0(&proposal))
    {
        Ok(preview) => {
            ui.label("PREVISUALIZACIÓN FNT0 — RPM nominales, no mediciones reales.");
            curve_plot(ui, &preview);
            ui.collapsing("Detalles de catálogo recuperado", |ui| {
                for point in &preview.points {
                    ui.label(format!(
                        "Punto {}: FNT0 S4/S5 {} / {}; CPU raw {:?}; GPU raw {:?}",
                        point.point,
                        point.fnt0_sensor4,
                        point.fnt0_sensor5,
                        point.cpu_cl_ct_raw,
                        point.gpu_gl_gt_raw
                    ));
                }
            });
            let payload = editor
                .document()
                .validate()
                .expect("preview was validated")
                .payload();
            ui.collapsing(
                "SIMULACIÓN — NO SE ENVIARÁ AL HARDWARE (payload de 64 bytes)",
                |ui| {
                    ui.monospace(
                        payload
                            .iter()
                            .map(|byte| format!("{byte:02x}"))
                            .collect::<Vec<_>>()
                            .join(" "),
                    );
                },
            );
        }
        Err(error) => {
            ui.colored_label(
                egui::Color32::from_rgb(220, 90, 90),
                format!("Curva inválida: {error}"),
            );
            ui.label("No está disponible para exportación.");
        }
    }
}

fn curve_plot(ui: &mut egui::Ui, preview: &OfflineCurvePreview) {
    let desired = egui::vec2(ui.available_width().min(580.0), 190.0);
    let (response, painter) = ui.allocate_painter(desired, egui::Sense::hover());
    let rect = response.rect.shrink2(egui::vec2(30.0, 20.0));
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0_f32, egui::Color32::GRAY),
        egui::StrokeKind::Inside,
    );
    let points: Vec<_> = preview
        .points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let x = rect.left() + rect.width() * (index as f32 / 9.0);
            let y = rect.bottom() - rect.height() * (point.nominal_rpm as f32 / 5_000.0);
            egui::pos2(x, y)
        })
        .collect();
    painter.add(egui::Shape::line(
        points.clone(),
        egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(80, 170, 240)),
    ));
    for (index, point) in points.iter().enumerate() {
        painter.circle_filled(*point, 3.5, egui::Color32::from_rgb(80, 170, 240));
        painter.text(
            egui::pos2(point.x - 3.0, rect.bottom() + 4.0),
            egui::Align2::LEFT_TOP,
            (index + 1).to_string(),
            egui::FontId::proportional(11.0),
            egui::Color32::GRAY,
        );
    }
    painter.text(
        rect.left_top(),
        egui::Align2::LEFT_BOTTOM,
        "5000 RPM",
        egui::FontId::proportional(11.0),
        egui::Color32::GRAY,
    );
    painter.text(
        rect.left_bottom(),
        egui::Align2::LEFT_TOP,
        "0 RPM",
        egui::FontId::proportional(11.0),
        egui::Color32::GRAY,
    );
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

fn ac_text(value: &Result<bool, String>) -> String {
    match value {
        Ok(true) => "AC".into(),
        Ok(false) => "Batería".into(),
        Err(error) => format!("No disponible: {error}"),
    }
}

fn profile_text(value: &Result<legion_core::PlatformProfileReading, String>) -> String {
    match value {
        Ok(profile) => profile
            .normalized
            .active
            .as_ref()
            .map(|active| active.name.clone())
            .unwrap_or_else(|| "No disponible".to_owned()),
        Err(error) => format!("No disponible: {error}"),
    }
}

fn rpm_text(fan: &HwmonFanReading) -> String {
    fan.rpm
        .value
        .map(|rpm| format!("{rpm} RPM"))
        .unwrap_or_else(|| "No disponible".to_owned())
}

fn friendly_source(fan: &HwmonFanReading) -> &'static str {
    match fan.source_classification {
        HwmonDriverClassification::Verified83ExExperimentalQuirk => {
            "yogafan_83ex (quirk experimental)"
        }
        HwmonDriverClassification::Yogafan83ExCandidateUnverified => "yogafan",
        HwmonDriverClassification::Validated83ExQuirkSimulation => "yogafan (simulación)",
        HwmonDriverClassification::GenericUnvalidated if fan.driver_name == "acpi_fan" => {
            "acpi_fan"
        }
        HwmonDriverClassification::GenericUnvalidated => "hwmon",
    }
}

fn friendly_reliability(fan: &HwmonFanReading) -> &'static str {
    match (&fan.module_provenance, fan.validation, fan.interpretation) {
        (HwmonModuleProvenance::OriginalYogafan { .. }, _, _) => {
            "yogafan original: 0 RPM no demuestra ventiladores detenidos en 83EX."
        }
        (HwmonModuleProvenance::StandardAcpiFan { .. }, _, _) => {
            "acpi_fan: no demuestra RPM independientes de ambos ventiladores."
        }
        (_, FanValidation::Validated, _) => {
            "Lectura validada para su fuente; sin asignación CPU/GPU."
        }
        (_, _, FanInterpretation::NonConclusive) => {
            "Lectura no concluyente; no se infiere que el ventilador esté detenido."
        }
        _ => "Lectura no validada; sin asignación CPU/GPU.",
    }
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
    fn original_yogafan_zero_stays_non_conclusive_and_friendly() {
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
        assert_eq!(friendly_source(&fan), "yogafan");
        assert!(friendly_reliability(&fan).contains("no demuestra"));
    }

    #[test]
    fn invalid_curves_cannot_be_exported_through_the_shared_validator() {
        let editor = CurveEditor {
            indices: [1; 10],
            path: "fixture.json".into(),
            notice: None,
        };
        assert!(editor.document().validate().is_err());
    }

    #[test]
    fn default_curve_previews_existing_fnt0_nominal_levels() {
        let proposal = CurveEditor::new().document().validate().unwrap();
        let preview = preview_83ex_fnt0(&proposal).unwrap();
        assert_eq!(preview.points[0].nominal_rpm, 0);
        assert_eq!(preview.points[9].nominal_rpm, 5000);
    }
}
