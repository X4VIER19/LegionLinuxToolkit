//! Local-only curve documents and embedded catalogue preview. No transport.
use legion_fan::{OfflineCurveFile, preview_83ex_fnt0};
use std::path::Path;

fn load(path: &Path) -> Result<OfflineCurveFile, String> {
    let path = std::path::absolute(path).map_err(|e| e.to_string())?;
    let bytes = legion_linux::read_offline_regular_file(&path, 4096).map_err(|e| e.to_string())?;
    OfflineCurveFile::from_json(&bytes)
}

pub(super) fn run(args: &[String]) -> Result<String, String> {
    let mut curve = None;
    let mut save = None;
    let mut payload = false;
    let mut format = "text";
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok("Offline only: legion-cli curve (--indices 1,2,3,4,5,6,7,8,9,10 | --open FILE.json) [--save NEW.json] [--payload] [--format text|json]\nNo apply option, hardware access or overwrite. Catalogue values are nominal, not measurements.".into()),
            "--indices" => {
                if curve.is_some() { return Err("choose exactly one --indices or --open".into()); }
                let values = args.next().ok_or("--indices requires ten comma-separated integers")?
                    .split(',').map(|v| v.trim().parse::<u16>().map_err(|_| "invalid index".to_owned()))
                    .collect::<Result<Vec<_>, _>>()?;
                curve = Some(OfflineCurveFile::new(values.try_into().map_err(|_| "exactly ten indices required")?));
            }
            "--open" => {
                if curve.is_some() { return Err("choose exactly one --indices or --open".into()); }
                curve = Some(load(Path::new(args.next().ok_or("--open requires a file")?))?);
            }
            "--save" => {
                if save.is_some() { return Err("duplicate --save".into()); }
                save = Some(args.next().ok_or("--save requires a new file")?);
            }
            "--payload" => payload = true,
            "--format" => format = args.next().ok_or("--format requires text or json")?,
            other => return Err(format!("unsupported offline curve argument: {other}")),
        }
    }
    if !matches!(format, "text" | "json") {
        return Err("format must be text or json".into());
    }
    let curve = curve.ok_or("provide --indices or --open")?;
    let proposal = curve.validate()?;
    let preview = preview_83ex_fnt0(&proposal)?;
    let bytes = proposal.payload();
    if let Some(path) = save {
        let path = std::path::absolute(path).map_err(|e| e.to_string())?;
        legion_linux::save_offline_curve_json(&path, &curve.to_pretty_json()?)?;
    }
    if format == "json" {
        return serde_json::to_string_pretty(&serde_json::json!({
            "source":"offline-curve-simulation", "execution_enabled":false,
            "curve":curve, "catalogue_source":preview.source, "points":preview.points,
            "payload_simulation":if payload { Some(bytes.to_vec()) } else { None },
            "notice":"Nominal RPM only. Thresholds are recovered raw catalogue values, not measured temperatures or verified physical fan mapping. No write authorization."
        })).map_err(|e| e.to_string());
    }
    let mut text = format!(
        "OFFLINE / SIMULATION — LLT GodMode V2 — 83EX/NUCN15WW\n{}\nPoint Index NominalRPM FNT0-S4 FNT0-S5 CPU(CL/CT) GPU(GL/GT)\n",
        preview.source
    );
    for p in preview.points {
        text.push_str(&format!(
            "{:>5} {:>5} {:>10} {:>7} {:>7} {:>4}/{:<4} {:>4}/{:<4}\n",
            p.point,
            p.index,
            p.nominal_rpm,
            p.fnt0_sensor4,
            p.fnt0_sensor5,
            p.cpu_cl_ct_raw[0],
            p.cpu_cl_ct_raw[1],
            p.gpu_gl_gt_raw[0],
            p.gpu_gl_gt_raw[1]
        ));
    }
    text.push_str("RPM nominales, no medidas. Umbrales: valores crudos de catálogo; no calibración física ni mapeo de canales. Escritura deshabilitada.\n");
    if payload {
        text.push_str(&format!(
            "SIMULATED PAYLOAD (64 bytes): {}\n",
            bytes
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn invoke(args: &[&str]) -> Result<String, String> {
        run(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn preview_payload_and_threshold_positions_reuse_the_existing_model() {
        let output = invoke(&[
            "--indices",
            "2,2,3,4,5,6,7,8,9,10",
            "--payload",
            "--format",
            "json",
        ])
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value["execution_enabled"], false);
        assert_eq!(value["points"][0]["nominal_rpm"], 1700);
        assert_eq!(value["points"][0]["fnt0_sensor4"], 48);
        assert_eq!(
            value["points"][0]["cpu_cl_ct_raw"],
            serde_json::json!([0, 58])
        );
        let bytes = value["payload_simulation"].as_array().unwrap();
        assert_eq!(bytes.len(), 64);
        assert_eq!(
            &bytes[6..10],
            &serde_json::json!([2, 0, 2, 0]).as_array().unwrap()[..]
        );
        assert!(bytes[26..].iter().all(|v| v == 0));
    }

    #[test]
    fn invalid_inputs_versions_and_unknown_fields_are_rejected() {
        for indices in [
            "1,2",
            "0,2,3,4,5,6,7,8,9,10",
            "1,2,3,4,5,6,7,8,9,11",
            "2,1,3,4,5,6,7,8,9,10",
            "1,1,1,1,1,1,1,1,1,1",
            "a,2,3,4,5,6,7,8,9,10",
        ] {
            assert!(invoke(&["--indices", indices]).is_err());
        }
        assert!(invoke(&["--real"]).is_err());
        assert!(invoke(&["--open", "/sys/firmware/acpi/platform_profile"]).is_err());
        let example = include_str!("../../../examples/curves/83ex-default-v1.json");
        assert!(OfflineCurveFile::from_json(example.as_bytes()).is_ok());
        let wrong_version = example.replace("\"schema_version\": 1", "\"schema_version\": 2");
        assert!(OfflineCurveFile::from_json(wrong_version.as_bytes()).is_err());
        assert!(
            OfflineCurveFile::from_json(
                example
                    .replace("\"indices\":", "\"apply\": true, \"indices\":")
                    .as_bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn local_json_roundtrip_refuses_overwrite_symlinks_and_host_paths() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!(
                "curve-file-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir(&root).unwrap();
        let file = root.join("curve.json");
        let path = file.to_str().unwrap();
        invoke(&["--indices", "1,2,3,4,5,6,7,8,9,10", "--save", path]).unwrap();
        assert!(
            invoke(&["--open", path, "--payload"])
                .unwrap()
                .contains("SIMULATED PAYLOAD")
        );
        assert!(invoke(&["--open", path, "--save", path]).is_err());
        let link = root.join("link.json");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(load(&link).is_err());
        assert!(
            legion_linux::save_offline_curve_json(Path::new("/sys/curve.json"), b"{}").is_err()
        );
        assert!(
            legion_linux::save_offline_curve_json(Path::new("/dev/curve.json"), b"{}").is_err()
        );
        assert!(load(&root).is_err());
        std::fs::write(root.join("invalid.json"), b"{not-json").unwrap();
        assert!(load(&root.join("invalid.json")).is_err());
    }
}
