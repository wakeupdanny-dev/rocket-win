//! Client for the Rocket VPN helper service (a LocalSystem Windows service that
//! brings up the TUN tunnel without a UAC prompt). Talks over a local named pipe.
//! When the service isn't installed, callers fall back to elevating the GUI.

#![cfg(windows)]

use std::io::{Read, Write};
use std::time::{Duration, Instant};

const PIPE: &str = r"\\.\pipe\rocket-vpn-svc";

/// Runtime dir the service uses (readable by normal users).
pub fn data_dir() -> std::path::PathBuf {
    let base = std::env::var_os("ProgramData")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData"));
    base.join("Rocket")
}

const ERROR_PIPE_BUSY: i32 = 231;

fn request(json: &str) -> anyhow::Result<serde_json::Value> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut pipe = loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(PIPE) {
            Ok(p) => break p,
            // only wait when the single pipe instance is momentarily busy;
            // "not found" means the service isn't installed — fail fast
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(60));
            }
            Err(e) => return Err(e.into()),
        }
    };

    pipe.write_all(json.as_bytes())?;
    pipe.write_all(b"\n")?;
    pipe.flush()?;

    let mut buf = Vec::new();
    let mut b = [0u8; 512];
    loop {
        let n = pipe.read(&mut b)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&b[..n]);
        if buf.contains(&b'\n') {
            break;
        }
    }
    Ok(serde_json::from_slice(&buf)?)
}

/// Is the helper service reachable?
pub fn available() -> bool {
    request(r#"{"cmd":"ping"}"#)
        .map(|v| v["ok"].as_bool().unwrap_or(false))
        .unwrap_or(false)
}

/// Is the service currently running a tunnel?
pub fn is_running() -> bool {
    request(r#"{"cmd":"status"}"#)
        .map(|v| v["running"].as_bool().unwrap_or(false))
        .unwrap_or(false)
}

pub fn connect(config_json: &str) -> anyhow::Result<()> {
    let req = serde_json::json!({ "cmd": "connect", "config": config_json });
    let v = request(&req.to_string())?;
    if v["ok"].as_bool().unwrap_or(false) {
        Ok(())
    } else {
        anyhow::bail!(
            "singbox_start_failed: {}",
            v["error"].as_str().unwrap_or("service error")
        )
    }
}

pub fn connect_awg(conf: &str, singbox_config: &str) -> anyhow::Result<()> {
    let req = serde_json::json!({ "cmd": "connect_awg", "conf": conf, "singbox_config": singbox_config });
    let v = request(&req.to_string())?;
    if v["ok"].as_bool().unwrap_or(false) {
        Ok(())
    } else {
        anyhow::bail!(
            "amneziawg_start_failed: {}",
            v["error"].as_str().unwrap_or("service error")
        )
    }
}

pub fn disconnect() -> anyhow::Result<()> {
    let _ = request(r#"{"cmd":"disconnect"}"#)?;
    Ok(())
}
