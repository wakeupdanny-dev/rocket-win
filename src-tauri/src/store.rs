use crate::model::Persisted;
use std::fs;
use std::path::{Path, PathBuf};

pub fn config_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("Rocket");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn data_file() -> PathBuf {
    config_dir().join("store.json")
}

pub fn runtime_config_file() -> PathBuf {
    config_dir().join("sing-box.json")
}

pub fn load() -> Persisted {
    let path = data_file();
    match fs::read_to_string(&path) {
        Ok(txt) => serde_json::from_str(&txt).unwrap_or_default(),
        Err(_) => Persisted::default(),
    }
}

pub fn save(p: &Persisted) -> anyhow::Result<()> {
    let path = data_file();
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(p)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn write_runtime_config(json: &str) -> anyhow::Result<PathBuf> {
    let path = runtime_config_file();
    fs::write(&path, json)?;
    Ok(path)
}

/// Resolve the bundled sing-box binary for both `tauri dev` and installed builds.
pub fn singbox_path(resource_dir: Option<&Path>) -> PathBuf {
    if let Some(rd) = resource_dir {
        let p = rd.join("sing-box.exe");
        if p.exists() {
            return p;
        }
    }
    // dev fallback: <crate>/binaries/sing-box.exe
    let dev = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join("sing-box.exe");
    if dev.exists() {
        return dev;
    }
    PathBuf::from("sing-box.exe")
}
