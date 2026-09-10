//! Rocket VPN helper service.
//!
//! Runs as LocalSystem so the GUI can bring up the sing-box TUN tunnel without a
//! UAC prompt. The GUI talks to it over a local named pipe whose ACL only lets
//! interactive users + SYSTEM + Administrators connect.

#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
mod svc {
    use serde::{Deserialize, Serialize};
    use std::ffi::OsStr;
    use std::path::PathBuf;
    use std::process::{Child, Command};
    use std::sync::{Mutex, MutexGuard};
    use std::time::Duration;

    /// Lock that shrugs off poisoning — a panic in one request handler must not
    /// wedge every future request.
    fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
        m.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub const SERVICE_NAME: &str = "RocketVpnSvc";
    pub const DISPLAY_NAME: &str = "Rocket VPN Helper";
    pub const PIPE_NAME: &str = r"\\.\pipe\rocket-vpn-svc";
    // SYSTEM + Admins: full; interactive users: read/write. Protected (no inherit).
    const PIPE_SDDL: &str = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)";

    pub fn data_dir() -> PathBuf {
        let base = std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
        let d = base.join("Rocket");
        let _ = std::fs::create_dir_all(&d);
        d
    }

    fn singbox_path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("sing-box.exe")))
            .unwrap_or_else(|| PathBuf::from("sing-box.exe"))
    }

    // ------------------------- child (sing-box) -------------------------

    static CHILD: Mutex<Option<Child>> = Mutex::new(None);

    pub fn stop_core() {
        if let Some(mut c) = lock(&CHILD).take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    fn core_running() -> bool {
        let mut guard = lock(&CHILD);
        match guard.as_mut() {
            Some(c) => match c.try_wait() {
                Ok(Some(_)) => {
                    *guard = None;
                    false
                }
                _ => true,
            },
            None => false,
        }
    }

    /// `keep_awg`: when sing-box is being layered on top of an AmneziaWG uplink
    /// we just brought up (its "proxy" outbound binds to that interface), don't
    /// tear the uplink back down; every other caller wants a clean slate.
    fn start_core(config: &str, keep_awg: bool) -> Result<(), String> {
        stop_core();
        if !keep_awg {
            awg_stop();
        }
        let dir = data_dir();
        let cfg = dir.join("config.json");
        std::fs::write(&cfg, config).map_err(|e| format!("write config: {e}"))?;

        let sb = singbox_path();
        if !sb.exists() {
            return Err(format!("sing-box not found at {}", sb.display()));
        }
        let log = std::fs::File::create(dir.join("core.log"))
            .map_err(|e| format!("open log: {e}"))?;
        let log_err = log.try_clone().map_err(|e| e.to_string())?;

        let mut cmd = Command::new(&sb);
        cmd.arg("run").arg("-c").arg(&cfg).arg("-D").arg(&dir)
            .stdout(log)
            .stderr(log_err);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let child = cmd.spawn().map_err(|e| format!("spawn sing-box: {e}"))?;
        *lock(&CHILD) = Some(child);

        // give it a moment; if it died immediately, surface the log tail
        std::thread::sleep(Duration::from_millis(600));
        if !core_running() {
            let tail = std::fs::read_to_string(dir.join("core.log")).unwrap_or_default();
            let tail: String = tail.lines().rev().take(6).collect::<Vec<_>>().join(" | ");
            return Err(format!("sing-box exited: {tail}"));
        }
        Ok(())
    }

    // ------------------------- amneziawg -------------------------

    const AWG_IFACE: &str = "RocketAWG";

    struct AwgState {
        child: Child,
        endpoint_ip: String,
    }
    static AWG: Mutex<Option<AwgState>> = Mutex::new(None);

    pub fn awg_running() -> bool {
        let mut g = lock(&AWG);
        match g.as_mut() {
            Some(s) => match s.child.try_wait() {
                Ok(Some(_)) => { *g = None; false }
                _ => true,
            },
            None => false,
        }
    }

    pub fn awg_stop() {
        if let Some(mut s) = lock(&AWG).take() {
            let _ = s.child.kill();
            let _ = s.child.wait();
            if !s.endpoint_ip.is_empty() {
                let _ = run("route", &["delete", &s.endpoint_ip]);
            }
            let _ = run("ipconfig", &["/flushdns"]);
        }
    }

    fn run(prog: &str, args: &[&str]) -> Result<String, String> {
        use std::os::windows::process::CommandExt;
        let out = Command::new(prog)
            .args(args)
            .creation_flags(0x0800_0000)
            .output()
            .map_err(|e| format!("{prog}: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).to_string())
        } else {
            Err(format!(
                "{prog} {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }

    fn b64_to_hex(b64: &str) -> Result<String, String> {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|_| "bad base64 key".to_string())?;
        Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }

    fn resolve(host: &str) -> Option<String> {
        use std::net::ToSocketAddrs;
        if host.parse::<std::net::IpAddr>().is_ok() {
            return Some(host.to_string());
        }
        (host, 0u16).to_socket_addrs().ok()?.find_map(|s| {
            let ip = s.ip();
            if ip.is_ipv4() { Some(ip.to_string()) } else { None }
        })
    }

    fn default_gateway() -> Option<String> {
        let out = run(
            "powershell",
            &["-NoProfile", "-Command",
              "(Get-NetRoute -DestinationPrefix '0.0.0.0/0' -EA SilentlyContinue | Where-Object {$_.NextHop -ne '0.0.0.0'} | Sort-Object RouteMetric | Select-Object -First 1 -ExpandProperty NextHop)"],
        ).ok()?;
        let s = out.trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    }

    fn prefix_to_mask(p: u8) -> String {
        let m: u32 = if p == 0 { 0 } else { !0u32 << (32 - p as u32) };
        format!("{}.{}.{}.{}", (m >> 24) & 255, (m >> 16) & 255, (m >> 8) & 255, m & 255)
    }

    struct Wg {
        priv_hex: String,
        peer_hex: String,
        psk_hex: Option<String>,
        endpoint_host: String,
        endpoint_port: u16,
        addresses: Vec<(String, u8)>, // ipv4 only
        #[allow(dead_code)] // DNS is sing-box's job now; kept for reference/debugging
        dns: Vec<String>,
        mtu: u32,
        keepalive: u32,
        allowed: Vec<String>,
        junk: Vec<(String, String)>, // (jc,"4") ...
    }

    fn parse_wg(text: &str) -> Result<Wg, String> {
        let mut sect = "";
        let mut i: std::collections::HashMap<String, String> = Default::default();
        let mut p: std::collections::HashMap<String, String> = Default::default();
        for line in text.lines() {
            let l = line.trim();
            if l.is_empty() || l.starts_with('#') || l.starts_with(';') {
                continue;
            }
            if l.starts_with('[') {
                sect = if l.eq_ignore_ascii_case("[interface]") { "i" }
                    else if l.eq_ignore_ascii_case("[peer]") { "p" } else { "" };
                continue;
            }
            if let Some((k, v)) = l.split_once('=') {
                let (k, v) = (k.trim().to_lowercase(), v.trim().to_string());
                match sect { "i" => { i.insert(k, v); } "p" => { p.insert(k, v); } _ => {} }
            }
        }
        let get = |m: &std::collections::HashMap<String, String>, k: &str| m.get(k).cloned();
        let ep = get(&p, "endpoint").ok_or("wg: no Endpoint")?;
        let (h, port) = ep.rsplit_once(':').ok_or("wg: bad Endpoint")?;
        let addresses = get(&i, "address")
            .unwrap_or_default()
            .split(',')
            .filter_map(|a| {
                let a = a.trim();
                let (ip, pfx) = a.split_once('/').unwrap_or((a, "32"));
                if ip.parse::<std::net::Ipv4Addr>().is_ok() {
                    Some((ip.to_string(), pfx.parse().unwrap_or(32)))
                } else {
                    None
                }
            })
            .collect();
        let junk = ["jc", "jmin", "jmax", "s1", "s2", "s3", "s4", "h1", "h2", "h3", "h4",
                    "i1", "i2", "i3", "i4", "i5"]
            .iter()
            .filter_map(|k| get(&i, k).map(|v| (k.to_string(), v)))
            .collect();
        Ok(Wg {
            priv_hex: b64_to_hex(&get(&i, "privatekey").ok_or("wg: no PrivateKey")?)?,
            peer_hex: b64_to_hex(&get(&p, "publickey").ok_or("wg: no Peer PublicKey")?)?,
            psk_hex: match get(&p, "presharedkey") {
                Some(k) if !k.is_empty() => Some(b64_to_hex(&k)?),
                _ => None,
            },
            endpoint_host: h.trim_matches(['[', ']']).to_string(),
            endpoint_port: port.parse().map_err(|_| "wg: bad port")?,
            addresses,
            dns: get(&i, "dns").map(|d| d.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_default(),
            mtu: get(&i, "mtu").and_then(|m| m.parse().ok()).unwrap_or(0),
            keepalive: get(&p, "persistentkeepalive").and_then(|m| m.parse().ok()).unwrap_or(25),
            allowed: get(&p, "allowedips")
                .unwrap_or_else(|| "0.0.0.0/0".into())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            junk,
        })
    }

    fn awg_uapi(msg: &str) -> Result<(), String> {
        use std::io::{Read, Write};
        let pipe = format!(r"\\.\pipe\ProtectedPrefix\Administrators\AmneziaWG\{AWG_IFACE}");
        // the go UAPI listener re-arms one pipe instance per accept, so a fresh
        // open can momentarily race with ERROR_PIPE_BUSY (231) / not-found (2)
        let mut f = {
            let mut last = String::new();
            let mut opened = None;
            for _ in 0..80 {
                match std::fs::OpenOptions::new().read(true).write(true).open(&pipe) {
                    Ok(h) => { opened = Some(h); break; }
                    Err(e) => {
                        last = e.to_string();
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            }
            opened.ok_or_else(|| format!("open awg uapi: {last}"))?
        };
        f.write_all(msg.as_bytes()).map_err(|e| e.to_string())?;
        f.flush().ok();
        let mut resp = String::new();
        let mut b = [0u8; 256];
        while let Ok(n) = f.read(&mut b) {
            if n == 0 { break; }
            resp.push_str(&String::from_utf8_lossy(&b[..n]));
            if resp.contains("errno=") { break; }
        }
        if resp.contains("errno=0") {
            Ok(())
        } else {
            Err(format!("awg uapi rejected config: {}", resp.trim()))
        }
    }

    fn awg_uapi_get() -> Result<String, String> {
        use std::io::{Read, Write};
        let pipe = format!(r"\\.\pipe\ProtectedPrefix\Administrators\AmneziaWG\{AWG_IFACE}");
        let mut f = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&pipe)
            .map_err(|e| format!("open awg uapi (get): {e}"))?;
        f.write_all(b"get=1\n\n").map_err(|e| e.to_string())?;
        f.flush().ok();
        let mut resp = String::new();
        let mut b = [0u8; 1024];
        while let Ok(n) = f.read(&mut b) {
            if n == 0 { break; }
            resp.push_str(&String::from_utf8_lossy(&b[..n]));
            if resp.contains("\n\n") || resp.contains("errno=") { break; }
        }
        Ok(resp)
    }

    /// true once the peer has completed a handshake
    fn awg_handshaked() -> bool {
        awg_uapi_get()
            .ok()
            .map(|s| {
                s.lines().any(|l| {
                    l.strip_prefix("last_handshake_time_sec=")
                        .and_then(|v| v.trim().parse::<u64>().ok())
                        .map(|n| n > 0)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    }

    /// Bring AmneziaWG up as a plain uplink interface — addressed, handshaked,
    /// with a *non-hijacking* default route (real metric loses to the physical
    /// NIC) — then start sing-box on top of it, with its "proxy" outbound bound
    /// to this interface. sing-box's own TUN does all the capturing/routing, so
    /// the RU-bypass / zapret / download-bypass switches now apply to AmneziaWG
    /// exactly like any other protocol; only the uplink itself is AWG-specific.
    fn connect_awg_routed(conf: &str, singbox_config: &str) -> Result<(), String> {
        awg_stop();
        stop_core();
        if let Err(e) = start_awg_uplink(conf) {
            awg_stop();
            return Err(e);
        }
        if let Err(e) = start_core(singbox_config, true) {
            awg_stop();
            return Err(e);
        }
        Ok(())
    }

    fn start_awg_uplink(conf: &str) -> Result<(), String> {
        let cfg = parse_wg(conf)?;
        let ip = resolve(&cfg.endpoint_host).ok_or("can't resolve AWG endpoint")?;
        let dir = data_dir();
        let _ = std::fs::write(dir.join("awg.conf"), conf);

        // route the AWG server's own UDP outside the tunnel
        if let Some(gw) = default_gateway() {
            let _ = run("route", &["delete", &ip]);
            let _ = run("route", &["add", &ip, "mask", "255.255.255.255", &gw, "metric", "1"]);
        }

        let exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("amneziawg.exe")))
            .filter(|p| p.exists())
            .ok_or("amneziawg.exe not found")?;
        let log = std::fs::File::create(dir.join("awg.log")).ok();
        use std::os::windows::process::CommandExt;
        let child = Command::new(&exe)
            .arg(AWG_IFACE)
            .stdout(std::process::Stdio::null())
            .stderr(log.map(std::process::Stdio::from).unwrap_or(std::process::Stdio::null()))
            .env("LOG_LEVEL", "error")
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|e| format!("spawn amneziawg: {e}"))?;
        *lock(&AWG) = Some(AwgState { child, endpoint_ip: ip.clone() });

        // let it create the adapter / arm the UAPI pipe; bail early if it died
        // (elevation / driver / adapter failure). awg_uapi retries the open.
        for _ in 0..8 {
            std::thread::sleep(Duration::from_millis(150));
            if !awg_running() {
                let tail = std::fs::read_to_string(dir.join("awg.log")).unwrap_or_default();
                return Err(format!(
                    "amneziawg exited on startup: {}",
                    tail.lines().rev().take(5).collect::<Vec<_>>().join(" | ")
                ));
            }
        }

        // push the config (awg_uapi retries past the listener's re-arm race)
        let mut set = String::from("set=1\n");
        set += &format!("private_key={}\n", cfg.priv_hex);
        for (k, v) in &cfg.junk {
            set += &format!("{k}={v}\n");
        }
        set += "replace_peers=true\n";
        set += &format!("public_key={}\n", cfg.peer_hex);
        if let Some(psk) = &cfg.psk_hex {
            set += &format!("preshared_key={psk}\n");
        }
        set += &format!("endpoint={ip}:{}\n", cfg.endpoint_port);
        set += &format!("persistent_keepalive_interval={}\n", cfg.keepalive);
        set += "replace_allowed_ips=true\n";
        for a in &cfg.allowed {
            set += &format!("allowed_ip={a}\n");
        }
        set += "\n";
        if let Err(e) = awg_uapi(&set) {
            awg_stop();
            return Err(e);
        }

        // interface addressing / routing (amneziawg-go leaves this to us)
        std::thread::sleep(Duration::from_millis(400));
        for (idx, (a, pfx)) in cfg.addresses.iter().enumerate() {
            let mask = prefix_to_mask(*pfx);
            let verb = if idx == 0 { "set" } else { "add" };
            run("netsh", &["interface", "ipv4", verb, "address",
                           &format!("name={AWG_IFACE}"), "static", a, &mask])
                .map_err(|e| { awg_stop(); e })?;
        }
        let mtu = if cfg.mtu == 0 { 1420 } else { cfg.mtu };
        let _ = run("netsh", &["interface", "ipv4", "set", "subinterface",
                               AWG_IFACE, &format!("mtu={mtu}"), "store=active"]);
        // DNS is sing-box's job now (it owns the TUN) — this interface doesn't
        // need its own resolver.

        // non-default routes are harmless to add now
        for a in &cfg.allowed {
            if a != "0.0.0.0/0" && a.contains('.') {
                let _ = run("netsh", &["interface", "ipv4", "add", "route", a, AWG_IFACE, "metric=5"]);
            }
        }

        // wait for the handshake before this interface is reachable at all —
        // a bad key/endpoint should fail cleanly, not leave a half-up uplink
        let wants_default = cfg.allowed.iter().any(|a| a == "0.0.0.0/0");
        let mut ok = false;
        for _ in 0..40 {
            if awg_handshaked() { ok = true; break; }
            if !awg_running() { break; }
            std::thread::sleep(Duration::from_millis(400));
        }
        if !ok {
            let tail = std::fs::read_to_string(dir.join("awg.log")).unwrap_or_default();
            return Err(format!(
                "amneziawg: no handshake (check keys / endpoint / obfuscation params){}",
                {
                    let t = tail.lines().rev().take(4).collect::<Vec<_>>().join(" | ");
                    if t.is_empty() { String::new() } else { format!(": {t}") }
                }
            ));
        }

        if wants_default {
            // a PLAIN default route, not the classic /1+/1 split: split routes
            // always win by longest-prefix-match regardless of metric, which
            // would hijack all system traffic. A same-prefix-length route at a
            // worse metric than the real NIC's stays out of the way for normal
            // sockets, while sing-box's bind_interface can still reach through it.
            let _ = run("netsh", &["interface", "ipv4", "add", "route", "0.0.0.0/0", AWG_IFACE, "metric=100"]);
        }
        Ok(())
    }

    // ------------------------- pipe protocol -------------------------

    #[derive(Deserialize)]
    #[serde(tag = "cmd", rename_all = "lowercase")]
    enum Req {
        Ping,
        Status,
        Connect { config: String },
        #[serde(rename = "connect_awg")]
        ConnectAwg { conf: String, singbox_config: String },
        Disconnect,
    }

    #[derive(Serialize, Default)]
    struct Resp {
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        running: Option<bool>,
    }

    fn handle(line: &str) -> Resp {
        match serde_json::from_str::<Req>(line) {
            Ok(Req::Ping) => Resp { ok: true, ..Default::default() },
            Ok(Req::Status) => Resp {
                ok: true,
                running: Some(core_running() || awg_running()),
                ..Default::default()
            },
            Ok(Req::Connect { config }) => {
                awg_stop(); // in case we're switching from an AmneziaWG server
                match start_core(&config, false) {
                    Ok(()) => Resp { ok: true, ..Default::default() },
                    Err(e) => Resp { ok: false, error: Some(e), ..Default::default() },
                }
            }
            Ok(Req::ConnectAwg { conf, singbox_config }) => match connect_awg_routed(&conf, &singbox_config) {
                Ok(()) => Resp { ok: true, ..Default::default() },
                Err(e) => Resp { ok: false, error: Some(e), ..Default::default() },
            },
            Ok(Req::Disconnect) => {
                stop_core();
                awg_stop();
                Resp { ok: true, ..Default::default() }
            }
            Err(e) => Resp { ok: false, error: Some(format!("bad request: {e}")), ..Default::default() },
        }
    }

    // ------------------------- named pipe server -------------------------

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::Storage::FileSystem::{FlushFileBuffers, ReadFile, WriteFile};
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe,
    };

    const PIPE_ACCESS_DUPLEX: u32 = 0x0000_0003;
    const PIPE_TYPE_BYTE: u32 = 0x0000_0000;
    const PIPE_READMODE_BYTE: u32 = 0x0000_0000;
    const PIPE_WAIT: u32 = 0x0000_0000;
    const PIPE_UNLIMITED_INSTANCES: u32 = 255;

    fn wide(s: &str) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    struct SendHandle(HANDLE);
    unsafe impl Send for SendHandle {}

    /// Serve the pipe until `should_stop` returns true.
    ///
    /// One instance is always listening while others are busy, and each
    /// connection is handled on its own thread — so a slow `connect` (which can
    /// take many seconds bringing a tunnel up) never blocks `ping` / `status`.
    pub fn serve(should_stop: impl Fn() -> bool) {
        let name = wide(PIPE_NAME);
        let sddl = wide(PIPE_SDDL);

        unsafe {
            let mut psd: *mut core::ffi::c_void = std::ptr::null_mut();
            let ok = ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1, // SDDL_REVISION_1
                &mut psd,
                std::ptr::null_mut(),
            );
            let mut sa = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: if ok != 0 { psd } else { std::ptr::null_mut() },
                bInheritHandle: 0,
            };

            while !should_stop() {
                let pipe = CreateNamedPipeW(
                    name.as_ptr(),
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    PIPE_UNLIMITED_INSTANCES,
                    64 * 1024, // out buffer
                    64 * 1024, // in buffer
                    0,
                    &mut sa,
                );
                if pipe == INVALID_HANDLE_VALUE {
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }

                let connected = ConnectNamedPipe(pipe, std::ptr::null_mut());
                if connected == 0 {
                    // ERROR_PIPE_CONNECTED (535) also means a client is there
                    let err = windows_sys::Win32::Foundation::GetLastError();
                    if err != 535 {
                        CloseHandle(pipe);
                        continue;
                    }
                }

                // hand the connected instance to a worker and loop back to
                // create the next listening instance immediately
                let h = SendHandle(pipe);
                let _ = std::thread::Builder::new()
                    .name("pipe-conn".into())
                    .spawn(move || serve_conn(h));
            }
        }
    }

    fn serve_conn(h: SendHandle) {
        let pipe = h.0;
        serve_reqresp(pipe);
        unsafe {
            FlushFileBuffers(pipe);
            DisconnectNamedPipe(pipe);
            CloseHandle(pipe);
        }
    }

    fn serve_reqresp(pipe: HANDLE) {
        let req = unsafe { read_msg(pipe) };
        let resp = match req {
            Some(l) => {
                let line = l.trim().to_string();
                std::panic::catch_unwind(|| handle(&line)).unwrap_or_else(|_| Resp {
                    ok: false,
                    error: Some("internal error (handler panicked)".into()),
                    ..Default::default()
                })
            }
            None => Resp { ok: false, error: Some("empty request".into()), ..Default::default() },
        };
        let mut body = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".into());
        body.push('\n');
        unsafe { write_all(pipe, body.as_bytes()) };
    }

    unsafe fn read_msg(pipe: HANDLE) -> Option<String> {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let mut read = 0u32;
            let ok = ReadFile(
                pipe,
                chunk.as_mut_ptr() as *mut _,
                chunk.len() as u32,
                &mut read,
                std::ptr::null_mut(),
            );
            if ok == 0 || read == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..read as usize]);
            if buf.contains(&b'\n') || buf.len() > 512 * 1024 {
                break;
            }
        }
        if buf.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&buf).to_string())
        }
    }

    unsafe fn write_all(pipe: HANDLE, mut data: &[u8]) {
        while !data.is_empty() {
            let mut written = 0u32;
            let ok = WriteFile(
                pipe,
                data.as_ptr() as *const _,
                data.len() as u32,
                &mut written,
                std::ptr::null_mut(),
            );
            if ok == 0 || written == 0 {
                break;
            }
            data = &data[written as usize..];
        }
    }

    // ------------------------- SCM integration -------------------------

    use windows_service::service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    windows_service::define_windows_service!(ffi_service_main, service_main);

    fn service_main(_args: Vec<std::ffi::OsString>) {
        let (tx, rx) = std::sync::mpsc::channel::<()>();

        let handler = move |control| -> ServiceControlHandlerResult {
            match control {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    stop_core();
                    awg_stop();
                    let _ = tx.send(());
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };
        let status_handle = match service_control_handler::register(SERVICE_NAME, handler) {
            Ok(h) => h,
            Err(_) => return,
        };

        let running = |state, accept| ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accept,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        };
        let _ = status_handle.set_service_status(running(
            ServiceState::Running,
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        ));

        // pipe server on its own thread; main thread waits for the stop signal
        let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sf = stop_flag.clone();
        let worker = std::thread::spawn(move || {
            serve(move || sf.load(std::sync::atomic::Ordering::Relaxed))
        });

        let _ = rx.recv();
        stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        // nudge the blocking ConnectNamedPipe by opening the pipe once
        let _ = std::fs::OpenOptions::new().read(true).write(true).open(PIPE_NAME);
        let _ = worker.join();

        let _ = status_handle.set_service_status(running(
            ServiceState::Stopped,
            ServiceControlAccept::empty(),
        ));
    }

    pub fn run_as_service() -> windows_service::Result<()> {
        windows_service::service_dispatcher::start(SERVICE_NAME, ffi_service_main)
    }

    pub fn install() -> windows_service::Result<()> {
        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
        )?;
        let exe = std::env::current_exe().unwrap();
        let info = ServiceInfo {
            name: SERVICE_NAME.into(),
            display_name: DISPLAY_NAME.into(),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: exe.clone(),
            launch_arguments: vec!["run".into()],
            dependencies: vec![],
            account_name: None, // LocalSystem
            account_password: None,
        };
        let svc = match manager.create_service(&info, ServiceAccess::START | ServiceAccess::CHANGE_CONFIG) {
            Ok(s) => s,
            Err(windows_service::Error::Winapi(e)) if e.raw_os_error() == Some(1073) => {
                // already exists — reopen and point it at this exe, in case an
                // earlier install left it aimed at a stale (e.g. build-output) path
                let s = manager.open_service(
                    SERVICE_NAME,
                    ServiceAccess::START | ServiceAccess::STOP | ServiceAccess::CHANGE_CONFIG,
                )?;
                let _ = s.stop();
                std::thread::sleep(Duration::from_millis(400));
                let _ = s.change_config(&info);
                s
            }
            Err(e) => return Err(e),
        };
        let _ = svc.set_description("Brings up the Rocket VPN tunnel with system privileges.");
        let _ = svc.start(&[] as &[&std::ffi::OsStr]);
        Ok(())
    }

    pub fn uninstall() -> windows_service::Result<()> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let svc = manager.open_service(
            SERVICE_NAME,
            ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
        )?;
        let _ = svc.stop();
        std::thread::sleep(Duration::from_millis(400));
        svc.delete()?;
        Ok(())
    }

    /// Run the pipe server in the foreground (for debugging, not as a service).
    pub fn console() {
        eprintln!("rocket-svc console mode — pipe {PIPE_NAME}");
        serve(|| false);
    }
}

#[cfg(windows)]
fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    match arg.as_str() {
        "install" => {
            match svc::install() {
                Ok(()) => println!("installed"),
                Err(e) => {
                    eprintln!("install failed: {e} (run as Administrator)");
                    std::process::exit(1);
                }
            }
        }
        "uninstall" => {
            match svc::uninstall() {
                Ok(()) => println!("uninstalled"),
                Err(e) => {
                    eprintln!("uninstall failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        "console" => svc::console(),
        "run" | "" => {
            if svc::run_as_service().is_err() {
                // not launched by the SCM — fall back to console
                svc::console();
            }
        }
        other => {
            eprintln!("unknown arg: {other}");
            std::process::exit(2);
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("rocket-svc is Windows-only");
}
