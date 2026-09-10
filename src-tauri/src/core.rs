//! sing-box process lifecycle + traffic polling.

use crate::model::Traffic;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

const LOG_CAP: usize = 800;

#[derive(Default)]
pub struct Core {
    child: Mutex<Option<Child>>,
    logs: Mutex<VecDeque<String>>,
    traffic: Mutex<Traffic>,
    clash_port: Mutex<u16>,
    stop_flag: Mutex<Arc<std::sync::atomic::AtomicBool>>,
}

impl Core {
    pub fn new() -> Self {
        Core::default()
    }

    pub fn is_running(&self) -> bool {
        self.child.lock().is_some()
    }

    pub fn logs_snapshot(&self) -> Vec<String> {
        self.logs.lock().iter().cloned().collect()
    }

    pub fn traffic_snapshot(&self) -> Traffic {
        self.traffic.lock().clone()
    }

    fn push_log(&self, line: String) {
        let mut l = self.logs.lock();
        if l.len() >= LOG_CAP {
            l.pop_front();
        }
        l.push_back(line);
    }

    pub async fn start(
        self: &Arc<Self>,
        app: AppHandle,
        singbox: &Path,
        config_path: &Path,
        working_dir: &Path,
        clash_port: u16,
    ) -> anyhow::Result<()> {
        self.stop().await;

        let mut cmd = Command::new(singbox);
        cmd.arg("run")
            .arg("-c")
            .arg(config_path)
            .arg("-D")
            .arg(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

        let mut child = cmd.spawn()?;
        *self.clash_port.lock() = clash_port;

        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        *self.stop_flag.lock() = stop.clone();

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        *self.child.lock() = Some(child);

        // log readers
        if let Some(out) = stdout {
            let this = self.clone();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(out).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    this.push_log(line.clone());
                    let _ = app.emit("log-line", line);
                }
            });
        }
        if let Some(err) = stderr {
            let this = self.clone();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    this.push_log(line.clone());
                    let _ = app.emit("log-line", line);
                }
            });
        }

        // traffic poller
        {
            let this = self.clone();
            let app = app.clone();
            let stop = stop.clone();
            tauri::async_runtime::spawn(async move {
                this.poll_traffic(app, stop).await;
            });
        }

        Ok(())
    }

    /// Best-effort synchronous kill for app-shutdown cleanup.
    pub fn kill_sync(&self) {
        self.stop_flag
            .lock()
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(mut c) = self.child.lock().take() {
            let _ = c.start_kill();
        }
    }

    pub async fn stop(&self) {
        self.stop_flag
            .lock()
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let child = self.child.lock().take();
        if let Some(mut c) = child {
            let _ = c.start_kill();
            let _ = c.wait().await;
        }
        *self.traffic.lock() = Traffic::default();
    }

    async fn poll_traffic(
        &self,
        app: AppHandle,
        stop: Arc<std::sync::atomic::AtomicBool>,
    ) {
        let port = *self.clash_port.lock();
        let url = format!("http://127.0.0.1:{port}/connections");
        let client = reqwest::Client::new();
        let (mut last_up, mut last_down) = (0u64, 0u64);
        let mut first = true;

        loop {
            if stop.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            let Ok(resp) = client.get(&url).send().await else {
                continue;
            };
            let Ok(v) = resp.json::<serde_json::Value>().await else {
                continue;
            };
            let up_total = v["uploadTotal"].as_u64().unwrap_or(0);
            let down_total = v["downloadTotal"].as_u64().unwrap_or(0);
            let (up_rate, down_rate) = if first {
                first = false;
                (0, 0)
            } else {
                (
                    up_total.saturating_sub(last_up),
                    down_total.saturating_sub(last_down),
                )
            };
            last_up = up_total;
            last_down = down_total;

            let t = Traffic {
                up: up_rate,
                down: down_rate,
                up_total,
                down_total,
            };
            *self.traffic.lock() = t.clone();
            let _ = app.emit("traffic", t);
        }
    }
}
