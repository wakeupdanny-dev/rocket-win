mod core;
mod elevate;
pub mod model;
pub mod parser;
#[cfg(windows)]
mod service;
pub mod singbox;
mod store;

use crate::core::Core;
use crate::model::*;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

type TrayItem = tauri::menu::MenuItem<tauri::Wry>;

struct TrayHandles {
    toggle: TrayItem,
    show: TrayItem,
    quit: TrayItem,
}

pub struct AppCtx {
    data: Mutex<Persisted>,
    core: Arc<Core>,
    connected: Mutex<bool>,
    tray: Mutex<Option<TrayHandles>>,
    quitting: std::sync::atomic::AtomicBool,
}

impl AppCtx {
    fn dto(&self) -> AppStateDto {
        let d = self.data.lock();
        AppStateDto {
            connected: *self.connected.lock(),
            selected: d.selected.clone(),
            mode: d.mode.clone(),
            listen_port: d.settings.listen_port,
            elevated: elevate::is_elevated(),
        }
    }
    fn persist(&self) {
        let d = self.data.lock();
        let _ = store::save(&d);
    }
}

/// Tiny translation table for the few Rust-side (tray) strings.
fn tr(lang: &str, key: &str) -> &'static str {
    let ru = lang == "ru";
    match key {
        "connect" => if ru { "Подключить" } else { "Connect" },
        "disconnect" => if ru { "Отключить" } else { "Disconnect" },
        "open" => if ru { "Открыть" } else { "Open" },
        "quit" => if ru { "Выход" } else { "Quit" },
        _ => "",
    }
}

fn ui_lang(app: &AppHandle) -> String {
    let l = app.state::<AppCtx>().data.lock().settings.lang.clone();
    if l.is_empty() { os_language() } else { l }
}

fn emit_state(app: &AppHandle) {
    let ctx = app.state::<AppCtx>();
    let dto = ctx.dto();
    let lang = ui_lang(app);
    if let Some(h) = ctx.tray.lock().as_ref() {
        let _ = h.toggle.set_text(tr(&lang, if dto.connected { "disconnect" } else { "connect" }));
    }
    let _ = app.emit("state-changed", dto);
}

/// Re-label the tray menu after a language change.
fn retranslate_tray(app: &AppHandle) {
    let lang = ui_lang(app);
    let connected = *app.state::<AppCtx>().connected.lock();
    if let Some(h) = app.state::<AppCtx>().tray.lock().as_ref() {
        let _ = h.toggle.set_text(tr(&lang, if connected { "disconnect" } else { "connect" }));
        let _ = h.show.set_text(tr(&lang, "open"));
        let _ = h.quit.set_text(tr(&lang, "quit"));
    }
}

// ------------------------- commands -------------------------

#[tauri::command]
fn get_state(ctx: State<AppCtx>) -> AppStateDto {
    ctx.dto()
}

#[tauri::command]
fn list_servers(ctx: State<AppCtx>) -> Vec<Server> {
    ctx.data.lock().servers.clone()
}

fn find_server(app: &AppHandle, id: &str) -> Result<Server, String> {
    app.state::<AppCtx>()
        .data
        .lock()
        .servers
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .ok_or_else(|| "server_not_found".to_string())
}

#[tauri::command]
fn export_link(id: String, app: AppHandle) -> Result<String, String> {
    Ok(parser::to_link(&find_server(&app, &id)?))
}

#[tauri::command]
fn export_json(id: String, app: AppHandle) -> Result<String, String> {
    let s = find_server(&app, &id)?;
    if matches!(s.kind.as_str(), "wireguard" | "amneziawg") {
        // there is no JSON form — hand back the wg-quick .conf
        return Ok(parser::to_link(&s));
    }
    let v = parser::to_xray_json(&s);
    serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
}

#[tauri::command]
fn export_qr(id: String, app: AppHandle) -> Result<String, String> {
    let link = parser::to_link(&find_server(&app, &id)?);
    // low error-correction = max capacity (~2953 bytes) — AmneziaWG confs with a
    // long I1-I5 obfuscation chain can run past the default level's ~2331 bytes
    let code = qrcode::QrCode::with_error_correction_level(link.as_bytes(), qrcode::EcLevel::L)
        .map_err(|_| "qr_too_long".to_string())?;
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(240, 240)
        .quiet_zone(true)
        .dark_color(qrcode::render::svg::Color("#000000"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build();
    // drop the XML prolog so it can be injected into HTML
    Ok(svg.split_once("?>").map(|(_, r)| r).unwrap_or(&svg).trim().to_string())
}

#[tauri::command]
async fn save_config(id: String, format: String, app: AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    {
        use tauri_plugin_dialog::DialogExt;
        let server = find_server(&app, &id)?;
        let safe: String = server
            .name
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let is_wg = matches!(server.kind.as_str(), "wireguard" | "amneziawg");
        let (content, ext) = if is_wg {
            (parser::to_link(&server), "conf")
        } else if format == "link" {
            (parser::to_link(&server), "txt")
        } else {
            (
                serde_json::to_string_pretty(&parser::to_xray_json(&server))
                    .map_err(|e| e.to_string())?,
                "json",
            )
        };
        let file = app
            .dialog()
            .file()
            .set_file_name(format!("{safe}.{ext}"))
            .add_filter(ext.to_uppercase(), &[ext])
            .blocking_save_file();
        let Some(fp) = file else {
            return Err("cancelled".into());
        };
        let path = fp.into_path().map_err(|e| e.to_string())?;
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (id, format, app);
        Err("cancelled".into())
    }
}

#[tauri::command]
fn get_settings(ctx: State<AppCtx>) -> Settings {
    ctx.data.lock().settings.clone()
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[derive(serde::Serialize)]
struct UpdateInfo {
    current: String,
    latest: String,
    available: bool,
    url: String,
    download_url: Option<String>,
}

const UPDATE_REPO: &str = "wakeupdanny-dev/rocket-win";

/// Pull `Rocket_X.Y.Z_x64-setup.exe` out of a release asset's file name —
/// tauri's NSIS output always follows `{productName}_{version}_x64-setup.exe`.
fn version_from_asset_name(name: &str) -> Option<String> {
    let rest = name.strip_prefix("Rocket_")?;
    let ver = rest.split('_').next()?;
    (ver.contains('.') && ver.chars().all(|c| c.is_ascii_digit() || c == '.'))
        .then(|| ver.to_string())
}

fn parse_semver(s: &str) -> (u32, u32, u32) {
    let mut it = s.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

#[tauri::command]
async fn check_update() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let client = reqwest::Client::builder()
        .user_agent("rocket-vpn-client") // required by the GitHub API, else 403
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let resp: serde_json::Value = client
        .get(format!("https://api.github.com/repos/{UPDATE_REPO}/releases/latest"))
        .send()
        .await
        .map_err(|_| "update_check_failed".to_string())?
        .json()
        .await
        .map_err(|_| "update_check_failed".to_string())?;

    let url = resp["html_url"].as_str().unwrap_or_default().to_string();
    let mut latest = current.clone();
    let mut download_url = None;
    if let Some(assets) = resp["assets"].as_array() {
        for a in assets {
            if let Some(v) = a["name"].as_str().and_then(version_from_asset_name) {
                latest = v;
                download_url = a["browser_download_url"].as_str().map(String::from);
                break;
            }
        }
    }
    let available = parse_semver(&latest) > parse_semver(&current);
    Ok(UpdateInfo { current, latest, available, url, download_url })
}

#[tauri::command]
fn open_url(url: String, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_shell::ShellExt;
    app.shell().open(url, None).map_err(|e| e.to_string())
}

/// The OS UI language collapsed to what we ship: "ru" for a Russian Windows,
/// "en" for anything else. Used to pick the default when the user hasn't chosen.
#[tauri::command]
fn os_language() -> String {
    #[cfg(windows)]
    {
        // LANGID low 10 bits = primary language; 0x19 = LANG_RUSSIAN
        let langid = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
        if (langid & 0x3ff) == 0x19 {
            return "ru".into();
        }
    }
    "en".into()
}

#[tauri::command]
async fn select_server(id: String, app: AppHandle) -> Result<(), String> {
    let (changed, connected) = {
        let ctx = app.state::<AppCtx>();
        let mut d = ctx.data.lock();
        let changed = d.selected.as_deref() != Some(id.as_str());
        d.selected = Some(id);
        let _ = store::save(&d);
        (changed, *ctx.connected.lock() || ctx.core.is_running())
    };

    // if a tunnel is up and the user picked a different server, switch to it
    if changed && connected {
        reconnect(app.clone()).await?;
    }
    emit_state(&app);
    Ok(())
}

#[tauri::command]
fn delete_server(id: String, ctx: State<AppCtx>) {
    let mut d = ctx.data.lock();
    d.servers.retain(|s| s.id != id);
    if d.selected.as_deref() == Some(id.as_str()) {
        d.selected = d.servers.first().map(|s| s.id.clone());
    }
    drop(d);
    ctx.persist();
}

#[tauri::command]
async fn set_mode(mode: String, app: AppHandle) -> Result<(), String> {
    {
        let ctx = app.state::<AppCtx>();
        ctx.data.lock().mode = mode;
        ctx.persist();
    }
    // hot-reload if connected
    let running = app.state::<AppCtx>().core.is_running();
    if running {
        reconnect(app.clone()).await?;
    }
    emit_state(&app);
    Ok(())
}

fn add_parsed(ctx: &AppCtx, parsed: Vec<model::Server>) -> Result<usize, String> {
    if parsed.is_empty() {
        return Err("no_servers_parsed".into());
    }
    let n = parsed.len();
    let mut d = ctx.data.lock();
    for s in parsed {
        d.servers.push(s);
    }
    if d.selected.is_none() {
        d.selected = d.servers.first().map(|s| s.id.clone());
    }
    drop(d);
    ctx.persist();
    Ok(n)
}

#[tauri::command]
fn add_servers_from_text(text: String, ctx: State<AppCtx>) -> Result<usize, String> {
    add_parsed(ctx.inner(), parser::parse_any(&text))
}

/// Open a native file picker and import servers from the chosen file:
/// a Xray/v2ray/sing-box JSON config, a list of share links, or a base64 blob.
#[tauri::command]
async fn import_from_file(app: AppHandle) -> Result<usize, String> {
    use tauri_plugin_dialog::DialogExt;

    let picked = app
        .dialog()
        .file()
        .add_filter("Config or links", &["json", "txt", "conf", "yaml", "yml", "dat"])
        .add_filter("All files", &["*"])
        .blocking_pick_file();

    let Some(fp) = picked else {
        return Err("cancelled".into());
    };
    let path = fp.into_path().map_err(|e| e.to_string())?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("file_read_failed: {e}"))?;

    let parsed = parser::parse_any_result(&text).map_err(|e| e.to_string())?;
    let ctx = app.state::<AppCtx>();
    add_parsed(ctx.inner(), parsed)
}

fn sub_name_from_url(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|rest| rest.split(['/', '?']).next())
        .filter(|h| !h.is_empty())
        .unwrap_or("Subscription")
        .to_string()
}

#[tauri::command]
async fn add_subscription(name: String, url: String, app: AppHandle) -> Result<usize, String> {
    let name = if name.trim().is_empty() {
        sub_name_from_url(&url)
    } else {
        name
    };
    let (body, info) = fetch_subscription(&url).await.map_err(|e| e.to_string())?;
    let mut parsed = parser::parse_any(&body);
    if parsed.is_empty() {
        return Err("sub_empty".into());
    }
    for s in &mut parsed {
        s.from_sub = Some(name.clone());
    }
    let n = parsed.len();
    {
        let ctx = app.state::<AppCtx>();
        let mut d = ctx.data.lock();
        d.servers.retain(|s| s.from_sub.as_deref() != Some(name.as_str()));
        d.servers.extend(parsed);
        let entry = Subscription {
            name: name.clone(),
            url: url.clone(),
            upload: info.upload,
            download: info.download,
            total: info.total,
            expire: info.expire,
            updated: now_secs(),
            count: n,
        };
        if let Some(existing) = d.settings.subscriptions.iter_mut().find(|x| x.url == url) {
            *existing = entry;
        } else {
            d.settings.subscriptions.push(entry);
        }
        if d.selected.is_none() {
            d.selected = d.servers.first().map(|s| s.id.clone());
        }
        drop(d);
        ctx.persist();
    }
    Ok(n)
}

#[tauri::command]
async fn update_subscriptions(app: AppHandle) -> Result<usize, String> {
    Ok(refresh_subs(&app, false).await)
}

/// Refresh subscriptions. `only_stale` limits work to ones past their
/// `sub_update_hours` window (used by the background auto-updater).
async fn refresh_subs(app: &AppHandle, only_stale: bool) -> usize {
    let (subs, hours) = {
        let ctx = app.state::<AppCtx>();
        let d = ctx.data.lock();
        (d.settings.subscriptions.clone(), d.settings.sub_update_hours)
    };
    let now = now_secs();
    let mut total = 0usize;
    let mut changed = false;

    for sub in subs {
        if only_stale {
            if hours == 0 {
                break;
            }
            if now.saturating_sub(sub.updated) < hours * 3600 {
                continue;
            }
        }
        let Ok((body, info)) = fetch_subscription(&sub.url).await else {
            continue;
        };
        let mut parsed = parser::parse_any(&body);
        if parsed.is_empty() {
            continue;
        }
        for s in &mut parsed {
            s.from_sub = Some(sub.name.clone());
        }
        let n = parsed.len();
        total += n;
        changed = true;

        let ctx = app.state::<AppCtx>();
        let mut d = ctx.data.lock();
        d.servers
            .retain(|s| s.from_sub.as_deref() != Some(sub.name.as_str()));
        d.servers.extend(parsed);
        if let Some(e) = d.settings.subscriptions.iter_mut().find(|x| x.url == sub.url) {
            e.upload = info.upload;
            e.download = info.download;
            e.total = info.total;
            e.expire = info.expire;
            e.updated = now;
            e.count = n;
        }
    }

    if changed {
        app.state::<AppCtx>().persist();
        emit_state(app);
    }
    total
}

/// Background loop: check every 30 min, refresh any subscription older than the
/// configured window.
async fn auto_update_loop(app: AppHandle) {
    tokio::time::sleep(Duration::from_secs(8)).await;
    loop {
        refresh_subs(&app, true).await;
        tokio::time::sleep(Duration::from_secs(30 * 60)).await;
    }
}

/// While connected with `fallback` on, poll for real connectivity; after two
/// consecutive misses, reconnect (which runs the failover search).
async fn fallback_watchdog(app: AppHandle) {
    let mut misses = 0u8;
    loop {
        tokio::time::sleep(Duration::from_secs(45)).await;

        let (connected, fallback) = {
            let ctx = app.state::<AppCtx>();
            let connected = *ctx.connected.lock();
            let fallback = ctx.data.lock().settings.fallback;
            (connected, fallback)
        };
        if !connected || !fallback {
            misses = 0;
            continue;
        }

        if probe_through_tunnel().await.is_some() {
            misses = 0;
            continue;
        }
        misses += 1;
        if misses >= 2 {
            misses = 0;
            let _ = do_connect(app.clone()).await;
            emit_state(&app);
        }
    }
}

#[tauri::command]
async fn set_settings(settings: serde_json::Value, app: AppHandle) -> Result<(), String> {
    if settings.get("__open_config").is_some() {
        let _ = open_path(&store::config_dir());
        return Ok(());
    }
    let autostart_before;
    {
        let ctx = app.state::<AppCtx>();
        let mut d = ctx.data.lock();
        autostart_before = d.settings.autostart;
        let mut cur = serde_json::to_value(&d.settings).map_err(|e| e.to_string())?;
        if let (Some(obj), Some(patch)) = (cur.as_object_mut(), settings.as_object()) {
            for (k, v) in patch {
                obj.insert(k.clone(), v.clone());
            }
        }
        d.settings = serde_json::from_value(cur).map_err(|e| e.to_string())?;
        let _ = store::save(&d);
    }

    if settings.get("lang").is_some() {
        retranslate_tray(&app);
    }
    if settings.get("autostart").is_some() {
        apply_autostart(&app, autostart_before);
    }

    // a routing-affecting change → rebuild the tunnel if it's up
    let routing_keys = ["zapret_mode", "zapret_dir", "dns", "download_bypass"];
    if routing_keys.iter().any(|k| settings.get(k).is_some()) {
        let running = {
            #[cfg(windows)]
            {
                *app.state::<AppCtx>().connected.lock() && service::is_running()
                    || app.state::<AppCtx>().core.is_running()
            }
            #[cfg(not(windows))]
            {
                app.state::<AppCtx>().core.is_running()
            }
        };
        if running {
            reconnect(app.clone()).await?;
        }
    }

    // turning fallback on while connected → check now, fail over if it's dead
    if settings.get("fallback").and_then(|v| v.as_bool()) == Some(true)
        && *app.state::<AppCtx>().connected.lock()
    {
        let h = app.clone();
        tauri::async_runtime::spawn(async move {
            if probe_through_tunnel().await.is_none() {
                let _ = do_connect(h.clone()).await;
                emit_state(&h);
            }
        });
    }

    emit_state(&app);
    Ok(())
}

/// Register / unregister the app in the Windows startup list.
fn apply_autostart(app: &AppHandle, enable: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let mgr = app.autolaunch();
    let _ = if enable { mgr.enable() } else { mgr.disable() };
}

#[tauri::command]
fn get_logs(ctx: State<AppCtx>) -> Vec<String> {
    let mem = ctx.core.logs_snapshot();
    if !mem.is_empty() {
        return mem;
    }
    // service mode: sing-box logs to ProgramData
    #[cfg(windows)]
    if let Ok(txt) = std::fs::read_to_string(service::data_dir().join("core.log")) {
        return txt
            .lines()
            .rev()
            .take(400)
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
    }
    mem
}

#[tauri::command]
fn get_traffic(ctx: State<AppCtx>) -> Traffic {
    ctx.core.traffic_snapshot()
}

#[tauri::command]
async fn test_latency(id: String, ctx: State<'_, AppCtx>) -> Result<i64, String> {
    let (target, connected, is_selected, clash_port) = {
        let d = ctx.data.lock();
        let t = d
            .servers
            .iter()
            .find(|s| s.id == id)
            .map(|s| (s.address.clone(), s.port, s.kind.clone()));
        let is_selected = d.selected.as_deref() == Some(id.as_str());
        let cp = if d.settings.clash_api_port == 0 { 9191 } else { d.settings.clash_api_port };
        (t, *ctx.connected.lock(), is_selected, cp)
    };
    let Some((addr, port, kind)) = target else {
        return Err("server_not_found".into());
    };

    // A server that ISN'T holding the tunnel can't be measured honestly while
    // connected: its traffic (TCP or ICMP) goes through the active tunnel
    // instead of a real path to it, which reads back as a bogus near-0ms.
    if connected && !is_selected {
        return Err("cant_measure_connected".into());
    }
    let is_active = connected && is_selected;
    let is_wg = matches!(kind.as_str(), "wireguard" | "amneziawg");

    // -1 = no answer.
    let ms = if is_active {
        // the running sing-box outbound (AmneziaWG included — its "proxy" outbound
        // is now a real sing-box direct/bind_interface outbound too) — ask the
        // core for a proxied URL-test; a raw TCP connect is swallowed by TUN (~1 ms)
        clash_delay(clash_port).await.map(|m| m as i64).unwrap_or(-1)
    } else if is_wg {
        // UDP endpoint — ICMP the server box (its /32 is routed around the
        // tunnel, so this is the real distance even while connected)
        tokio::task::spawn_blocking(move || measure_icmp(&addr))
            .await
            .map_err(|e| e.to_string())?
    } else {
        tokio::task::spawn_blocking(move || measure_tcp(&addr, port))
            .await
            .map_err(|e| e.to_string())?
    };

    ctx.data
        .lock()
        .servers
        .iter_mut()
        .filter(|s| s.id == id)
        .for_each(|s| s.latency = Some(ms));
    ctx.persist();
    Ok(ms)
}

/// Real latency of the running sing-box outbound, via the clash API's URL-test.
async fn clash_delay(port: u16) -> Option<u32> {
    let url = format!(
        "http://127.0.0.1:{port}/proxies/proxy/delay?timeout=3000&url=http://cp.cloudflare.com/generate_204"
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()?;
    let v: serde_json::Value = client.get(&url).send().await.ok()?.json().await.ok()?;
    v.get("delay").and_then(|d| d.as_u64()).map(|d| d as u32)
}

/// A real round-trip through whatever the current default route is (the live
/// tunnel, when one is up). `None` = no internet.
async fn probe_through_tunnel() -> Option<u32> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
        .ok()?;
    let start = std::time::Instant::now();
    let r = client
        .get("http://cp.cloudflare.com/generate_204")
        .send()
        .await
        .ok()?;
    (r.status().as_u16() == 204 || r.status().is_success())
        .then(|| start.elapsed().as_millis() as u32)
}

/// Poll for real connectivity until it appears or `timeout` runs out. A fresh
/// tunnel (AmneziaWG especially) can take several seconds before routes/DNS
/// actually carry traffic, so a single early probe would misjudge it as dead.
async fn wait_for_real_connectivity(timeout: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        if probe_through_tunnel().await.is_some() {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
    }
}

#[tauri::command]
async fn connect(app: AppHandle) -> Result<(), String> {
    do_connect(app.clone()).await.map_err(|e| e.to_string())?;
    emit_state(&app);
    Ok(())
}

#[tauri::command]
async fn disconnect(app: AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    if service::available() {
        let _ = service::disconnect();
    }
    let ctx = app.state::<AppCtx>();
    ctx.core.stop().await;
    *ctx.connected.lock() = false;
    emit_state(&app);
    Ok(())
}

// ------------------------- helpers -------------------------

async fn reconnect(app: AppHandle) -> Result<(), String> {
    do_connect(app).await.map_err(|e| e.to_string())
}

/// Connect to the selected server; if `fallback` is on and the tunnel comes up
/// but carries no traffic, switch to the next-best server automatically.
async fn do_connect(app: AppHandle) -> anyhow::Result<()> {
    connect_once(app.clone()).await?;

    let (fallback, original) = {
        let ctx = app.state::<AppCtx>();
        let d = ctx.data.lock();
        (d.settings.fallback, d.selected.clone())
    };
    if !fallback {
        return Ok(());
    }

    // give routes/DNS time to settle, then check for real connectivity. AmneziaWG
    // in particular can take 10+ seconds after the handshake before traffic
    // actually flows (route/DNS propagation) — a single quick probe was rejecting
    // perfectly good AWG candidates as "dead" during failover.
    if wait_for_real_connectivity(Duration::from_secs(15)).await {
        return Ok(());
    }

    // failover: try the other servers, lowest known latency first
    let candidates: Vec<String> = {
        let ctx = app.state::<AppCtx>();
        let d = ctx.data.lock();
        let mut v: Vec<(String, i64)> = d
            .servers
            .iter()
            .filter(|s| Some(&s.id) != original.as_ref())
            .map(|s| {
                let l = match s.latency {
                    Some(n) if n >= 0 => n,
                    _ => i64::MAX,
                };
                (s.id.clone(), l)
            })
            .collect();
        v.sort_by_key(|(_, l)| *l);
        v.into_iter().map(|(id, _)| id).take(4).collect()
    };

    for cand in candidates {
        {
            let ctx = app.state::<AppCtx>();
            let mut d = ctx.data.lock();
            d.selected = Some(cand.clone());
            let _ = store::save(&d);
        }
        emit_state(&app);
        if connect_once(app.clone()).await.is_ok() && wait_for_real_connectivity(Duration::from_secs(15)).await {
            emit_state(&app);
            return Ok(());
        }
    }

    // nothing worked — go back to the user's original pick
    {
        let ctx = app.state::<AppCtx>();
        let mut d = ctx.data.lock();
        d.selected = original;
        let _ = store::save(&d);
    }
    let _ = connect_once(app.clone()).await;
    emit_state(&app);
    Ok(())
}

async fn connect_once(app: AppHandle) -> anyhow::Result<()> {
    let (server, mode, settings) = {
        let ctx = app.state::<AppCtx>();
        let d = ctx.data.lock();
        let sel = d
            .selected
            .as_ref()
            .and_then(|id| d.servers.iter().find(|s| &s.id == id))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no_server_selected"))?;
        (sel, d.mode.clone(), d.settings.clone())
    };

    let clash_port = if settings.clash_api_port == 0 {
        9191
    } else {
        settings.clash_api_port
    };

    let zapret = if settings.zapret_mode && !settings.zapret_dir.is_empty() {
        read_zapret_domains(&settings.zapret_dir)
    } else {
        vec![]
    };

    // AmneziaWG: the helper drives amneziawg.exe as a non-default uplink over
    // its UAPI pipe, then layers a normal sing-box config on top whose "proxy"
    // outbound is bound to that interface — so routing (RU bypass / zapret /
    // download-bypass) applies exactly like it does for every other protocol.
    if server.kind == "amneziawg" {
        #[cfg(windows)]
        {
            if !service::available() {
                anyhow::bail!("service_required");
            }
            if server.raw_config.is_empty() {
                anyhow::bail!("amneziawg_no_config");
            }
            let cache = service::data_dir().join("cache.db");
            let config = singbox::build_config(&server, &mode, &settings, &cache.to_string_lossy(), &zapret);
            let config_json = serde_json::to_string(&config)?;
            service::connect_awg(&server.raw_config, &config_json)?;
            if !wait_ready(clash_port).await {
                let _ = service::disconnect();
                anyhow::bail!("singbox_start_failed: timeout");
            }
            *app.state::<AppCtx>().connected.lock() = true;
            return Ok(());
        }
        #[cfg(not(windows))]
        anyhow::bail!("amneziawg_unsupported");
    }

    // 1. Preferred path: the helper service brings up the tunnel with no UAC.
    #[cfg(windows)]
    if service::available() {
        let cache = service::data_dir().join("cache.db");
        let config = singbox::build_config(&server, &mode, &settings, &cache.to_string_lossy(), &zapret);
        let config_json = serde_json::to_string(&config)?;
        service::connect(&config_json)?;
        if !wait_ready(clash_port).await {
            let _ = service::disconnect();
            anyhow::bail!("singbox_start_failed: timeout");
        }
        *app.state::<AppCtx>().connected.lock() = true;
        return Ok(());
    }

    // 2. No service → run sing-box ourselves; TUN needs Administrator, so
    //    relaunch through UAC and resume the connection.
    #[cfg(windows)]
    if !elevate::is_elevated() {
        if elevate::relaunch_elevated(&["--autoconnect"]) {
            app.state::<AppCtx>().core.kill_sync();
            app.exit(0);
            anyhow::bail!("relaunching");
        }
        anyhow::bail!("admin_required");
    }

    let workdir = store::config_dir();
    let cache_path = workdir.join("cache.db");
    let config = singbox::build_config(
        &server,
        &mode,
        &settings,
        &cache_path.to_string_lossy(),
        &zapret,
    );
    let config_path =
        store::write_runtime_config(&serde_json::to_string_pretty(&config)?)?;

    let resource_dir = app.path().resource_dir().ok();
    let singbox = store::singbox_path(resource_dir.as_deref());
    if !singbox.exists() {
        anyhow::bail!("singbox_missing: {}", singbox.display());
    }

    let ctx = app.state::<AppCtx>();
    ctx.core
        .start(app.clone(), &singbox, &config_path, &workdir, clash_port)
        .await?;

    // wait for the clash api to answer, or fail with the tail of the log
    let ok = wait_ready(clash_port).await;
    if !ok {
        let tail = ctx.core.logs_snapshot();
        ctx.core.stop().await;
        let tail = tail
            .iter()
            .rev()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join(" | ");
        anyhow::bail!("singbox_start_failed: {tail}");
    }

    *ctx.connected.lock() = true;
    Ok(())
}

async fn wait_ready(clash_port: u16) -> bool {
    let url = format!("http://127.0.0.1:{clash_port}/version");
    let client = reqwest::Client::new();
    for _ in 0..40 {
        if let Ok(r) = client.get(&url).send().await {
            if r.status().is_success() {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    false
}

fn measure_tcp(addr: &str, port: u16) -> i64 {
    use std::net::{TcpStream, ToSocketAddrs};
    let start = std::time::Instant::now();
    let iter = match (addr, port).to_socket_addrs() {
        Ok(i) => i,
        Err(_) => return -1,
    };
    for sa in iter {
        if TcpStream::connect_timeout(&sa, Duration::from_secs(3)).is_ok() {
            return start.elapsed().as_millis() as i64;
        }
    }
    -1
}

/// ICMP echo to a host — the only pre-connect reachability signal for a
/// WireGuard/AmneziaWG server (its endpoint is UDP, so no TCP probe). Returns
/// round-trip ms, 0 for sub-millisecond, or -1 if unreachable / ICMP filtered.
#[cfg(windows)]
fn measure_icmp(host: &str) -> i64 {
    use std::net::{IpAddr, ToSocketAddrs};
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY,
    };

    let v4 = (host, 0u16)
        .to_socket_addrs()
        .ok()
        .and_then(|it| {
            it.filter_map(|s| match s.ip() {
                IpAddr::V4(v4) => Some(v4),
                _ => None,
            })
            .next()
        });
    let Some(v4) = v4 else { return -1 };

    unsafe {
        let h = IcmpCreateFile();
        if h == INVALID_HANDLE_VALUE {
            return -1;
        }
        let dest = u32::from_ne_bytes(v4.octets()); // in_addr (network order) on LE
        let payload = [0u8; 32];
        let reply_len = std::mem::size_of::<ICMP_ECHO_REPLY>() + payload.len() + 8;
        let mut reply = vec![0u8; reply_len];
        let n = IcmpSendEcho(
            h,
            dest,
            payload.as_ptr() as *const std::ffi::c_void,
            payload.len() as u16,
            std::ptr::null_mut(),
            reply.as_mut_ptr() as *mut std::ffi::c_void,
            reply_len as u32,
            2000,
        );
        IcmpCloseHandle(h);
        if n == 0 {
            return -1;
        }
        let r = &*(reply.as_ptr() as *const ICMP_ECHO_REPLY);
        if r.Status != 0 {
            return -1;
        }
        r.RoundTripTime as i64
    }
}

#[cfg(not(windows))]
fn measure_icmp(_host: &str) -> i64 {
    -1
}

#[cfg(all(test, windows))]
mod icmp_tests {
    #[test]
    fn pings_a_live_host() {
        let live = super::measure_icmp("1.1.1.1");
        assert!((0..2000).contains(&live), "1.1.1.1 should answer ICMP, got {live}");
        // an address with no route out — expect a timeout, not a bogus success
        assert_eq!(super::measure_icmp("240.0.0.1"), -1);
    }
}

#[derive(Default)]
struct SubUserinfo {
    upload: u64,
    download: u64,
    total: u64,
    expire: u64,
}

fn parse_userinfo(h: &str) -> SubUserinfo {
    let mut u = SubUserinfo::default();
    for part in h.split(';') {
        let part = part.trim();
        let Some((k, v)) = part.split_once('=') else { continue };
        let n = v.trim().parse::<u64>().unwrap_or(0);
        match k.trim() {
            "upload" => u.upload = n,
            "download" => u.download = n,
            "total" => u.total = n,
            "expire" => u.expire = n,
            _ => {}
        }
    }
    u
}

async fn fetch_subscription(url: &str) -> anyhow::Result<(String, SubUserinfo)> {
    let client = reqwest::Client::builder()
        .user_agent("ClashforWindows/0.20.39") // some panels only serve links to known UAs
        .timeout(Duration::from_secs(20))
        .build()?;
    let resp = client.get(url).send().await?.error_for_status()?;
    let info = resp
        .headers()
        .get("subscription-userinfo")
        .and_then(|v| v.to_str().ok())
        .map(parse_userinfo)
        .unwrap_or_default();
    let body = resp.text().await?;
    Ok((body, info))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ------------------------- zapret integration -------------------------

/// A folder is a zapret install if it has `lists/list-general.txt`.
fn is_zapret_dir(p: &std::path::Path) -> bool {
    p.join("lists").join("list-general.txt").is_file()
}

/// Look for a zapret folder on the Desktop / in common spots.
fn detect_zapret_dir() -> Option<String> {
    let mut roots: Vec<std::path::PathBuf> = vec![];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Desktop"));
        roots.push(home.join("Downloads"));
        roots.push(home);
    }
    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if !path.is_dir() {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            if (name.contains("zapret") || name.contains("запрет")) && is_zapret_dir(&path) {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }
    None
}

/// Parse zapret hostlist files into sing-box `domain_suffix` entries.
fn read_zapret_domains(dir: &str) -> Vec<String> {
    let lists = std::path::Path::new(dir).join("lists");
    let files = [
        "list-general.txt",
        "list-google.txt",
        "list-general-user.txt",
    ];
    let mut set = std::collections::BTreeSet::new();
    for f in files {
        let Ok(txt) = std::fs::read_to_string(lists.join(f)) else { continue };
        for line in txt.lines() {
            let line = line.trim().trim_start_matches('^');
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            // skip the placeholder entry zapret ships
            if line.eq_ignore_ascii_case("domain.example.abc") {
                continue;
            }
            if line.contains(' ') || !line.contains('.') {
                continue;
            }
            set.insert(line.to_ascii_lowercase());
        }
    }
    set.into_iter().collect()
}

#[tauri::command]
fn detect_zapret(app: AppHandle) -> String {
    let ctx = app.state::<AppCtx>();
    let cur = ctx.data.lock().settings.zapret_dir.clone();
    if !cur.is_empty() && is_zapret_dir(std::path::Path::new(&cur)) {
        return cur;
    }
    match detect_zapret_dir() {
        Some(dir) => {
            let mut d = ctx.data.lock();
            d.settings.zapret_dir = dir.clone();
            let _ = store::save(&d);
            dir
        }
        None => String::new(),
    }
}

#[tauri::command]
async fn pick_zapret_dir(app: AppHandle) -> Result<String, String> {
    #[cfg(windows)]
    {
        use tauri_plugin_dialog::DialogExt;
        let picked = app.dialog().file().blocking_pick_folder();
        let Some(fp) = picked else {
            return Err("cancelled".into());
        };
        let path = fp.into_path().map_err(|e| e.to_string())?;
        if !is_zapret_dir(&path) {
            return Err("not_a_zapret_dir".into());
        }
        let dir = path.to_string_lossy().to_string();
        let ctx = app.state::<AppCtx>();
        {
            let mut d = ctx.data.lock();
            d.settings.zapret_dir = dir.clone();
            let _ = store::save(&d);
        }
        Ok(dir)
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("cancelled".into())
    }
}

fn open_path(p: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer").arg(p).spawn()?;
    }
    #[cfg(not(windows))]
    {
        let _ = p;
    }
    Ok(())
}

fn reveal_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn quit_app(app: &AppHandle) {
    app.state::<AppCtx>()
        .quitting
        .store(true, std::sync::atomic::Ordering::Relaxed);
    // quitting the app tears the tunnel down (closing to tray does not)
    #[cfg(windows)]
    {
        let _ = service::disconnect();
    }
    app.state::<AppCtx>().core.kill_sync();
    app.exit(0);
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let lang = ui_lang(app);
    let toggle = MenuItem::with_id(app, "toggle", tr(&lang, "connect"), true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", tr(&lang, "open"), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", tr(&lang, "quit"), true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&toggle, &show, &PredefinedMenuItem::separator(app)?, &quit],
    )?;

    *app.state::<AppCtx>().tray.lock() = Some(TrayHandles {
        toggle: toggle.clone(),
        show: show.clone(),
        quit: quit.clone(),
    });

    let _tray = TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Rocket")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let app = app.clone();
            match event.id.as_ref() {
                "show" => reveal_window(&app),
                "quit" => quit_app(&app),
                "toggle" => {
                    tauri::async_runtime::spawn(async move {
                        let connected = *app.state::<AppCtx>().connected.lock();
                        let _ = if connected {
                            disconnect(app.clone()).await
                        } else {
                            connect(app.clone()).await
                        };
                    });
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    let shown = w.is_visible().unwrap_or(false)
                        && !w.is_minimized().unwrap_or(false);
                    if shown {
                        let _ = w.hide();
                    } else {
                        reveal_window(app);
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let persisted = store::load();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(
            // remember only the window SIZE, never the position — the app always
            // opens centred (see window.center in tauri.conf.json)
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .manage(AppCtx {
            data: Mutex::new(persisted),
            core: Arc::new(Core::new()),
            connected: Mutex::new(false),
            tray: Mutex::new(None),
            quitting: std::sync::atomic::AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            list_servers,
            get_settings,
            os_language,
            app_version,
            check_update,
            open_url,
            select_server,
            delete_server,
            set_mode,
            detect_zapret,
            pick_zapret_dir,
            export_link,
            export_json,
            export_qr,
            save_config,
            add_servers_from_text,
            import_from_file,
            add_subscription,
            update_subscriptions,
            set_settings,
            get_logs,
            get_traffic,
            test_latency,
            connect,
            disconnect,
        ])
        .on_window_event(|window, event| {
            use std::sync::atomic::Ordering;
            let ctx = window.state::<AppCtx>();
            match event {
                // the X button hides to the tray; minimize stays a normal minimize
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    if !ctx.quitting.load(Ordering::Relaxed) {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
                tauri::WindowEvent::Destroyed => {
                    ctx.core.kill_sync();
                }
                _ => {}
            }
        })
        .setup(|app| {
            build_tray(app.handle())?;
            let handle = app.handle().clone();
            let ctx = app.state::<AppCtx>();

            let want_autostart = ctx.data.lock().settings.autostart;
            apply_autostart(&handle, want_autostart);

            // if the helper service already has a tunnel up, show it as connected
            #[cfg(windows)]
            if service::available() && service::is_running() {
                *ctx.connected.lock() = true;
            }

            // launched by the OS startup entry → go straight to the tray
            if std::env::args().any(|a| a == "--hidden") {
                if let Some(w) = handle.get_webview_window("main") {
                    let _ = w.hide();
                }
            }

            // `--autoconnect` is passed by an elevation relaunch that wants to
            // resume the connection.
            // only the elevation relaunch resumes a connection on startup
            let resume = std::env::args().any(|a| a == "--autoconnect")
                && ctx.data.lock().selected.is_some();
            if resume {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = do_connect(h.clone()).await;
                    emit_state(&h);
                });
            }
            // periodic subscription auto-update
            tauri::async_runtime::spawn(auto_update_loop(handle.clone()));
            // watchdog: fail over to another server if the tunnel goes dead
            tauri::async_runtime::spawn(fallback_watchdog(handle.clone()));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
