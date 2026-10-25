//! Shared, one-query NVIDIA reader and strict parser. Never loads modules.
#[cfg(any(feature = "linux-status", feature = "experimental-fan-trial-monitor"))]
use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(any(feature = "linux-status", feature = "experimental-fan-trial-monitor"))]
/// Fixed executable/arguments, cleared environment, bounded child lifetime and
/// output. No shell, PATH lookup, fallback, installation or automatic retry.
pub(crate) fn nvidia_once() -> Result<Vec<u8>, String> {
    // Metadata checks only. Never load NVIDIA or a FullSpeed module.
    use std::os::unix::fs::FileTypeExt;
    if !std::fs::symlink_metadata("/dev/nvidiactl").is_ok_and(|m| m.file_type().is_char_device()) {
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
