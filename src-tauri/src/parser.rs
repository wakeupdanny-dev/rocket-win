use crate::model::Server;
use anyhow::{anyhow, Result};
use base64::Engine;
use std::collections::HashMap;
use uuid::Uuid;

fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.trim().replace(['\n', '\r', ' '], "");
    let engines = [
        base64::engine::general_purpose::STANDARD,
        base64::engine::general_purpose::STANDARD_NO_PAD,
        base64::engine::general_purpose::URL_SAFE,
        base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ];
    for e in engines {
        if let Ok(v) = e.decode(s.as_bytes()) {
            return Some(v);
        }
    }
    None
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

/// The pieces of a `scheme://[userinfo@]host[:port][?query][#fragment]` link,
/// parsed by hand so we tolerate non-standard forms (e.g. a base64-wrapped
/// `userinfo@host:port` authority, which `url::Url` mis-reads as the host).
struct LinkParts {
    userinfo: String,
    host: String,
    port: u16,
    query: HashMap<String, String>,
    fragment: String,
}

fn looks_base64ish(s: &str) -> bool {
    s.len() >= 16
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '-' | '_' | '='))
        && !s.contains('.')
}

fn split_link(link: &str, scheme: &str, default_port: u16) -> Result<LinkParts> {
    let rest = link
        .strip_prefix(scheme)
        .and_then(|r| r.strip_prefix("://"))
        .ok_or_else(|| anyhow!("not a {scheme} link"))?;

    let (before_frag, fragment) = match rest.split_once('#') {
        Some((a, f)) => (
            a,
            percent_encoding::percent_decode_str(f)
                .decode_utf8_lossy()
                .to_string(),
        ),
        None => (rest, String::new()),
    };
    let (authority, query_str) = match before_frag.split_once('?') {
        Some((a, q)) => (a, q),
        None => (before_frag, ""),
    };

    let query: HashMap<String, String> = url::form_urlencoded::parse(query_str.as_bytes())
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    // If the authority has no '@' but base64-decodes to something with one,
    // unwrap it (some generators emit `scheme://base64(userinfo@host:port)`).
    let authority = if !authority.contains('@') && looks_base64ish(authority) {
        b64_decode(authority)
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .filter(|s| s.contains('@'))
            .unwrap_or_else(|| authority.to_string())
    } else {
        authority.to_string()
    };

    let (userinfo, hostport) = match authority.rsplit_once('@') {
        Some((u, h)) => (u.trim_matches(':').to_string(), h.to_string()),
        None => (String::new(), authority.clone()),
    };
    let hostport = hostport.trim_start_matches('/');
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (
            h.trim_matches(['[', ']']).to_string(),
            p.split(['/', '?']).next().unwrap_or("").parse().unwrap_or(default_port),
        ),
        None => (hostport.to_string(), default_port),
    };

    Ok(LinkParts {
        userinfo: percent_encoding::percent_decode_str(&userinfo)
            .decode_utf8_lossy()
            .to_string(),
        host,
        port,
        query,
        fragment,
    })
}

/// Parse a single share link into a Server.
pub fn parse_link(link: &str) -> Result<Server> {
    let link = link.trim();
    if let Some(rest) = link.strip_prefix("vmess://") {
        return parse_vmess(rest);
    }
    if link.starts_with("vless://") {
        return parse_vless(link);
    }
    if link.starts_with("trojan://") {
        return parse_trojan(link);
    }
    if link.starts_with("ss://") {
        return parse_ss(link);
    }
    if link.starts_with("socks5://") || link.starts_with("socks://") {
        return parse_socks_http(link);
    }
    if link.starts_with("hysteria2://") || link.starts_with("hy2://") {
        return parse_hysteria2(link);
    }
    if link.starts_with("hysteria://") || link.starts_with("hy://") {
        return parse_hysteria(link);
    }
    if link.starts_with("tuic://") {
        return parse_tuic(link);
    }
    // http(s):// only when it looks like a bare proxy endpoint, not a subscription URL
    for pfx in ["http://", "https://"] {
        if let Some(rest) = link.strip_prefix(pfx) {
            let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
            if !rest[authority.len()..].starts_with('/') && authority.contains(':') {
                return parse_socks_http(link);
            }
        }
    }
    Err(anyhow!("unsupported link scheme"))
}

fn parse_vmess(b64: &str) -> Result<Server> {
    let raw = b64_decode(b64).ok_or_else(|| anyhow!("bad vmess base64"))?;
    let v: serde_json::Value = serde_json::from_slice(&raw)?;
    let gs = |k: &str| -> String {
        match &v[k] {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => String::new(),
        }
    };
    let port: u16 = gs("port").parse().unwrap_or(443);
    let net = {
        let n = gs("net");
        if n.is_empty() { "tcp".to_string() } else { n }
    };
    let tls = gs("tls");
    let host = {
        let h = gs("host");
        if h.is_empty() { gs("sni") } else { h }
    };
    let name = {
        let n = gs("ps");
        if n.is_empty() { format!("{}:{}", gs("add"), port) } else { n }
    };
    Ok(Server {
        id: new_id(),
        name,
        kind: "vmess".into(),
        address: gs("add"),
        port,
        uuid: gs("id"),
        password: String::new(),
        method: String::new(),
        alter_id: gs("aid").parse().unwrap_or(0),
        network: net,
        path: gs("path"),
        host: host.clone(),
        service_name: gs("path").trim_start_matches('/').to_string(),
        security: if tls == "tls" { "tls".into() } else { String::new() },
        sni: {
            let s = gs("sni");
            if s.is_empty() { host } else { s }
        },
        alpn: gs("alpn"),
        fingerprint: gs("fp"),
        flow: String::new(),
        public_key: String::new(),
        short_id: String::new(),
        allow_insecure: gs("verify_cert") == "false",
        up_mbps: 0,
        down_mbps: 0,
        obfs: String::new(),
        obfs_password: String::new(),
        congestion: String::new(),
        private_key: String::new(),
        pre_shared_key: String::new(),
        local_address: vec![],
        reserved: vec![],
        raw_config: String::new(),
        from_sub: None,
        latency: None,
    })
}

fn parse_vless(link: &str) -> Result<Server> {
    let p = split_link(link, "vless", 443)?;
    let q = p.query;
    let host = p.host;
    let port = p.port;
    let name = if p.fragment.is_empty() {
        format!("{host}:{port}")
    } else {
        p.fragment
    };
    let net = q.get("type").cloned().unwrap_or_else(|| "tcp".into());
    let sec = q.get("security").cloned().unwrap_or_default();
    let sni = q
        .get("sni")
        .or_else(|| q.get("host"))
        .cloned()
        .unwrap_or_default();
    Ok(Server {
        id: new_id(),
        name,
        kind: "vless".into(),
        address: host,
        port,
        uuid: p.userinfo,
        password: String::new(),
        method: String::new(),
        alter_id: 0,
        network: net,
        path: q.get("path").cloned().unwrap_or_default(),
        host: q.get("host").cloned().unwrap_or_default(),
        service_name: q.get("serviceName").cloned().unwrap_or_default(),
        security: match sec.as_str() {
            "reality" => "reality".into(),
            "tls" | "xtls" => "tls".into(),
            _ => String::new(),
        },
        sni,
        alpn: q.get("alpn").cloned().unwrap_or_default(),
        fingerprint: q.get("fp").cloned().unwrap_or_default(),
        flow: q.get("flow").cloned().unwrap_or_default(),
        public_key: q.get("pbk").cloned().unwrap_or_default(),
        short_id: q.get("sid").cloned().unwrap_or_default(),
        allow_insecure: q.get("allowInsecure").map(|s| s == "1").unwrap_or(false),
        up_mbps: 0,
        down_mbps: 0,
        obfs: String::new(),
        obfs_password: String::new(),
        congestion: String::new(),
        private_key: String::new(),
        pre_shared_key: String::new(),
        local_address: vec![],
        reserved: vec![],
        raw_config: String::new(),
        from_sub: None,
        latency: None,
    })
}

fn parse_trojan(link: &str) -> Result<Server> {
    let p = split_link(link, "trojan", 443)?;
    let q = p.query;
    let host = p.host.clone();
    let port = p.port;
    let name = if p.fragment.is_empty() {
        format!("{host}:{port}")
    } else {
        p.fragment
    };
    Ok(Server {
        id: new_id(),
        name,
        kind: "trojan".into(),
        address: host.clone(),
        port,
        uuid: String::new(),
        password: p.userinfo,
        method: String::new(),
        alter_id: 0,
        network: q.get("type").cloned().unwrap_or_else(|| "tcp".into()),
        path: q.get("path").cloned().unwrap_or_default(),
        host: q.get("host").cloned().unwrap_or_default(),
        service_name: q.get("serviceName").cloned().unwrap_or_default(),
        security: "tls".into(),
        sni: q.get("sni").cloned().unwrap_or(host),
        alpn: q.get("alpn").cloned().unwrap_or_default(),
        fingerprint: q.get("fp").cloned().unwrap_or_default(),
        flow: String::new(),
        public_key: String::new(),
        short_id: String::new(),
        allow_insecure: q.get("allowInsecure").map(|s| s == "1").unwrap_or(false),
        up_mbps: 0,
        down_mbps: 0,
        obfs: String::new(),
        obfs_password: String::new(),
        congestion: String::new(),
        private_key: String::new(),
        pre_shared_key: String::new(),
        local_address: vec![],
        reserved: vec![],
        raw_config: String::new(),
        from_sub: None,
        latency: None,
    })
}

fn parse_ss(link: &str) -> Result<Server> {
    // ss://BASE64(method:pass)@host:port#name   OR   ss://BASE64(method:pass@host:port)#name
    let without = link.strip_prefix("ss://").unwrap();
    let (main, name) = match without.split_once('#') {
        Some((m, n)) => (
            m.to_string(),
            percent_encoding::percent_decode_str(n)
                .decode_utf8_lossy()
                .to_string(),
        ),
        None => (without.to_string(), String::new()),
    };
    let (method, password, host, port) = if let Some((userinfo, hostpart)) = main.split_once('@') {
        let decoded = b64_decode(userinfo).unwrap_or_else(|| userinfo.as_bytes().to_vec());
        let up = String::from_utf8_lossy(&decoded).to_string();
        let (m, p) = up.split_once(':').ok_or_else(|| anyhow!("bad ss userinfo"))?;
        let (h, pt) = hostpart
            .split_once(':')
            .ok_or_else(|| anyhow!("bad ss host"))?;
        (
            m.to_string(),
            p.to_string(),
            h.to_string(),
            pt.split(['/', '?']).next().unwrap_or("443").to_string(),
        )
    } else {
        let decoded = b64_decode(&main).ok_or_else(|| anyhow!("bad ss base64"))?;
        let s = String::from_utf8_lossy(&decoded).to_string();
        let (mp, hp) = s.split_once('@').ok_or_else(|| anyhow!("bad ss blob"))?;
        let (m, p) = mp.split_once(':').ok_or_else(|| anyhow!("bad ss method"))?;
        let (h, pt) = hp.split_once(':').ok_or_else(|| anyhow!("bad ss host"))?;
        (m.to_string(), p.to_string(), h.to_string(), pt.to_string())
    };
    let port: u16 = port.parse().unwrap_or(443);
    Ok(Server {
        id: new_id(),
        name: if name.is_empty() { format!("{host}:{port}") } else { name },
        kind: "shadowsocks".into(),
        address: host,
        port,
        uuid: String::new(),
        password,
        method,
        alter_id: 0,
        network: "tcp".into(),
        path: String::new(),
        host: String::new(),
        service_name: String::new(),
        security: String::new(),
        sni: String::new(),
        alpn: String::new(),
        fingerprint: String::new(),
        flow: String::new(),
        public_key: String::new(),
        short_id: String::new(),
        allow_insecure: false,
        up_mbps: 0,
        down_mbps: 0,
        obfs: String::new(),
        obfs_password: String::new(),
        congestion: String::new(),
        private_key: String::new(),
        pre_shared_key: String::new(),
        local_address: vec![],
        reserved: vec![],
        raw_config: String::new(),
        from_sub: None,
        latency: None,
    })
}

// ---- socks / http proxy, hysteria, hysteria2, tuic ----

fn qbool(q: &HashMap<String, String>, k: &str) -> bool {
    matches!(q.get(k).map(|s| s.as_str()), Some("1" | "true" | "yes"))
}

fn parse_socks_http(link: &str) -> Result<Server> {
    let (scheme, kind) = if link.starts_with("socks5://") {
        ("socks5", "socks")
    } else if link.starts_with("socks://") {
        ("socks", "socks")
    } else if link.starts_with("https://") {
        ("https", "http")
    } else {
        ("http", "http")
    };
    let p = split_link(link, scheme, if kind == "socks" { 1080 } else { 8080 })?;
    // userinfo may be user:pass or base64(user:pass)
    let raw = if p.userinfo.contains(':') {
        p.userinfo.clone()
    } else if let Some(d) = b64_decode(&p.userinfo) {
        String::from_utf8_lossy(&d).to_string()
    } else {
        p.userinfo.clone()
    };
    let (user, pass) = raw.split_once(':').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or_default();
    Ok(Server {
        id: new_id(),
        name: if p.fragment.is_empty() { format!("{}:{}", p.host, p.port) } else { p.fragment },
        kind: kind.into(),
        address: p.host,
        port: p.port,
        method: user, // username
        password: pass,
        network: "tcp".into(),
        security: if kind == "http" && scheme == "https" { "tls".into() } else { String::new() },
        ..Default::default()
    })
}

fn parse_hysteria2(link: &str) -> Result<Server> {
    let scheme = if link.starts_with("hysteria2://") { "hysteria2" } else { "hy2" };
    let p = split_link(link, scheme, 443)?;
    let q = &p.query;
    Ok(Server {
        id: new_id(),
        name: if p.fragment.is_empty() { format!("{}:{}", p.host, p.port) } else { p.fragment.clone() },
        kind: "hysteria2".into(),
        address: p.host.clone(),
        port: p.port,
        password: p.userinfo.clone(),
        security: "tls".into(),
        sni: q.get("sni").cloned().unwrap_or(p.host),
        alpn: q.get("alpn").cloned().unwrap_or_default(),
        allow_insecure: qbool(q, "insecure"),
        obfs: q.get("obfs").cloned().unwrap_or_default(),
        obfs_password: q.get("obfs-password").or_else(|| q.get("obfs_password")).cloned().unwrap_or_default(),
        network: "udp".into(),
        ..Default::default()
    })
}

fn parse_hysteria(link: &str) -> Result<Server> {
    let scheme = if link.starts_with("hysteria://") { "hysteria" } else { "hy" };
    let p = split_link(link, scheme, 443)?;
    let q = &p.query;
    Ok(Server {
        id: new_id(),
        name: if p.fragment.is_empty() { format!("{}:{}", p.host, p.port) } else { p.fragment.clone() },
        kind: "hysteria".into(),
        address: p.host.clone(),
        port: p.port,
        password: q.get("auth").or_else(|| q.get("auth_str")).cloned().unwrap_or_else(|| p.userinfo.clone()),
        security: "tls".into(),
        sni: q.get("peer").or_else(|| q.get("sni")).cloned().unwrap_or(p.host),
        alpn: q.get("alpn").cloned().unwrap_or_default(),
        allow_insecure: qbool(q, "insecure"),
        obfs: q.get("obfs").cloned().unwrap_or_default(),
        up_mbps: q.get("upmbps").or_else(|| q.get("up_mbps")).and_then(|s| s.parse().ok()).unwrap_or(50),
        down_mbps: q.get("downmbps").or_else(|| q.get("down_mbps")).and_then(|s| s.parse().ok()).unwrap_or(100),
        network: "udp".into(),
        ..Default::default()
    })
}

fn parse_tuic(link: &str) -> Result<Server> {
    let p = split_link(link, "tuic", 443)?;
    let q = &p.query;
    let (uuid, password) = p.userinfo.split_once(':').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((p.userinfo.clone(), String::new()));
    Ok(Server {
        id: new_id(),
        name: if p.fragment.is_empty() { format!("{}:{}", p.host, p.port) } else { p.fragment.clone() },
        kind: "tuic".into(),
        address: p.host.clone(),
        port: p.port,
        uuid,
        password,
        security: "tls".into(),
        sni: q.get("sni").cloned().unwrap_or(p.host),
        alpn: q.get("alpn").cloned().unwrap_or_else(|| "h3".into()),
        allow_insecure: qbool(q, "allow_insecure") || qbool(q, "insecure"),
        congestion: q.get("congestion_control").cloned().unwrap_or_else(|| "bbr".into()),
        network: "udp".into(),
        ..Default::default()
    })
}

/// Parse a WireGuard `wg-quick` .conf. Returns an error for AmneziaWG configs
/// (junk-packet params) since the bundled sing-box can't do the obfuscation.
fn parse_wg_conf(text: &str) -> Result<Server> {
    let mut section = "";
    let mut iface: HashMap<String, String> = HashMap::new();
    let mut peer: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            section = if line.eq_ignore_ascii_case("[interface]") { "i" } else if line.eq_ignore_ascii_case("[peer]") { "p" } else { "" };
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let (k, v) = (k.trim().to_lowercase(), v.trim().to_string());
            match section {
                "i" => { iface.insert(k, v); }
                "p" => { peer.insert(k, v); }
                _ => {}
            }
        }
    }

    // AmneziaWG junk-packet obfuscation params → handled by amneziawg.exe, not sing-box
    let is_awg = ["jc", "jmin", "jmax", "s1", "s2", "s3", "s4", "h1", "h2", "h3", "h4"]
        .iter()
        .any(|k| iface.contains_key(*k));

    let priv_key = iface.get("privatekey").cloned().ok_or_else(|| anyhow!("wg: no PrivateKey"))?;
    let pub_key = peer.get("publickey").cloned().ok_or_else(|| anyhow!("wg: no Peer PublicKey"))?;
    let endpoint = peer.get("endpoint").cloned().ok_or_else(|| anyhow!("wg: no Endpoint"))?;
    let (host, port) = endpoint
        .rsplit_once(':')
        .map(|(h, p)| (h.trim_matches(['[', ']']).to_string(), p.parse().unwrap_or(51820)))
        .unwrap_or((endpoint.clone(), 51820));

    let local = iface
        .get("address")
        .map(|a| a.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>())
        .unwrap_or_default();

    Ok(Server {
        id: new_id(),
        name: if is_awg { format!("AmneziaWG {host}") } else { format!("WireGuard {host}") },
        kind: if is_awg { "amneziawg".into() } else { "wireguard".into() },
        address: host,
        port,
        private_key: priv_key,
        public_key: pub_key,
        pre_shared_key: peer.get("presharedkey").cloned().unwrap_or_default(),
        local_address: local,
        raw_config: if is_awg { text.trim().to_string() } else { String::new() },
        network: "udp".into(),
        ..Default::default()
    })
}

/// Parse a blob of text: many links, optionally base64-wrapped (subscription body).
pub fn parse_many(text: &str) -> Vec<Server> {
    let candidates: Vec<String> = {
        let trimmed = text.trim();
        let looks_like_link = trimmed.contains("://");
        if looks_like_link {
            trimmed.lines().map(|l| l.to_string()).collect()
        } else if let Some(bytes) = b64_decode(trimmed) {
            String::from_utf8_lossy(&bytes)
                .lines()
                .map(|l| l.to_string())
                .collect()
        } else {
            trimmed.lines().map(|l| l.to_string()).collect()
        }
    };

    let mut out = Vec::new();
    for line in candidates {
        let line = line.trim();
        if line.is_empty() || !line.contains("://") {
            continue;
        }
        if let Ok(s) = parse_link(line) {
            out.push(s);
        }
    }
    out
}

/// Parse arbitrary imported text: a Xray/v2ray/sing-box JSON config, or a blob of
/// share links / a base64 subscription.
pub fn parse_any(text: &str) -> Vec<Server> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let servers = parse_config_json(&v);
            if !servers.is_empty() {
                return servers;
            }
        }
    }
    // WireGuard / AmneziaWG .conf
    let lower = trimmed.to_lowercase();
    if lower.contains("[interface]") && lower.contains("[peer]") {
        return parse_wg_conf(trimmed).map(|s| vec![s]).unwrap_or_default();
    }
    parse_many(trimmed)
}

/// Like `parse_any` but surfaces the error (used by file import so the UI can
/// explain e.g. an unsupported AmneziaWG config).
pub fn parse_any_result(text: &str) -> Result<Vec<Server>> {
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();
    if lower.contains("[interface]") && lower.contains("[peer]") {
        return parse_wg_conf(trimmed).map(|s| vec![s]);
    }
    let v = parse_any(trimmed);
    if v.is_empty() {
        Err(anyhow!("no_servers_parsed"))
    } else {
        Ok(v)
    }
}

/// Extract proxy servers from a Xray / v2ray-core `outbounds` config (also handles
/// a sing-box `outbounds` config).
fn parse_config_json(v: &serde_json::Value) -> Vec<Server> {
    let outbounds = v
        .get("outbounds")
        .and_then(|o| o.as_array())
        .cloned()
        .or_else(|| v.as_array().cloned())
        .unwrap_or_default();

    let mut out = Vec::new();
    for ob in &outbounds {
        // Xray uses "protocol", sing-box uses "type"
        let proto = ob
            .get("protocol")
            .or_else(|| ob.get("type"))
            .and_then(|p| p.as_str())
            .unwrap_or("");
        match proto {
            "vless" | "vmess" | "trojan" | "shadowsocks" => {}
            _ => continue,
        }
        if let Some(s) = xray_outbound(ob, proto) {
            out.push(s);
        }
    }
    out
}

fn gstr(v: &serde_json::Value, path: &[&str]) -> String {
    let mut cur = v;
    for p in path {
        match cur.get(p) {
            Some(next) => cur = next,
            None => return String::new(),
        }
    }
    match cur {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

fn xray_outbound(ob: &serde_json::Value, proto: &str) -> Option<Server> {
    let stream = ob.get("streamSettings").cloned().unwrap_or_default();
    let network = {
        let n = gstr(&stream, &["network"]);
        if n.is_empty() { "tcp".into() } else { n }
    };
    let security = gstr(&stream, &["security"]);

    // endpoint + credentials differ by protocol
    let settings = ob.get("settings").cloned().unwrap_or_default();
    let (address, port, uuid, password, method, alter_id, flow) = match proto {
        "vless" | "vmess" => {
            let vnext = settings.get("vnext").and_then(|x| x.as_array())?.first()?.clone();
            let user = vnext.get("users").and_then(|x| x.as_array())?.first()?.clone();
            (
                gstr(&vnext, &["address"]),
                gstr(&vnext, &["port"]).parse().unwrap_or(443),
                gstr(&user, &["id"]),
                String::new(),
                String::new(),
                gstr(&user, &["alterId"]).parse().unwrap_or(0),
                gstr(&user, &["flow"]),
            )
        }
        _ => {
            let srv = settings.get("servers").and_then(|x| x.as_array())?.first()?.clone();
            (
                gstr(&srv, &["address"]),
                gstr(&srv, &["port"]).parse().unwrap_or(443),
                String::new(),
                gstr(&srv, &["password"]),
                gstr(&srv, &["method"]),
                0,
                String::new(),
            )
        }
    };
    if address.is_empty() {
        return None;
    }

    let tls_key = if security == "reality" { "realitySettings" } else { "tlsSettings" };
    let sni = {
        let s = gstr(&stream, &[tls_key, "serverName"]);
        if !s.is_empty() { s } else { gstr(&stream, &["wsSettings", "headers", "Host"]) }
    };

    let (ws_path, ws_host) = (
        gstr(&stream, &["wsSettings", "path"]),
        gstr(&stream, &["wsSettings", "headers", "Host"]),
    );
    let grpc_name = gstr(&stream, &["grpcSettings", "serviceName"]);

    let name = {
        let tag = ob.get("tag").and_then(|t| t.as_str()).unwrap_or("");
        if !tag.is_empty() && tag != "proxy" && tag != "out" {
            tag.to_string()
        } else {
            format!("{address}:{port}")
        }
    };

    Some(Server {
        id: new_id(),
        name,
        kind: proto.to_string(),
        address,
        port,
        uuid,
        password,
        method,
        alter_id,
        network: network.clone(),
        path: if network == "ws" { ws_path } else { String::new() },
        host: ws_host,
        service_name: grpc_name,
        security: match security.as_str() {
            "reality" => "reality".into(),
            "tls" | "xtls" => "tls".into(),
            _ if proto == "trojan" => "tls".into(),
            _ => String::new(),
        },
        sni,
        alpn: String::new(),
        fingerprint: {
            let f = gstr(&stream, &[tls_key, "fingerprint"]);
            if f.is_empty() { "chrome".into() } else { f }
        },
        flow,
        public_key: gstr(&stream, &["realitySettings", "publicKey"]),
        short_id: gstr(&stream, &["realitySettings", "shortId"]),
        allow_insecure: gstr(&stream, &[tls_key, "allowInsecure"]) == "true",
        up_mbps: 0,
        down_mbps: 0,
        obfs: String::new(),
        obfs_password: String::new(),
        congestion: String::new(),
        private_key: String::new(),
        pre_shared_key: String::new(),
        local_address: vec![],
        reserved: vec![],
        raw_config: String::new(),
        from_sub: None,
        latency: None,
    })
}

// ------------------------- export (Server -> share formats) -------------------------

fn enc(s: &str) -> String {
    const SET: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
        .add(b' ')
        .add(b'"')
        .add(b'#')
        .add(b'%')
        .add(b'&')
        .add(b'+')
        .add(b'/')
        .add(b'<')
        .add(b'>')
        .add(b'?')
        .add(b'=')
        .add(b'`')
        .add(b'{')
        .add(b'}');
    percent_encoding::utf8_percent_encode(s, SET).to_string()
}

/// Build a share link (`vless://` / `vmess://` / `trojan://` / `ss://`).
pub fn to_link(s: &Server) -> String {
    let name = enc(&s.name);
    match s.kind.as_str() {
        "vmess" => {
            let j = serde_json::json!({
                "v": "2",
                "ps": s.name,
                "add": s.address,
                "port": s.port.to_string(),
                "id": s.uuid,
                "aid": s.alter_id.to_string(),
                "scy": "auto",
                "net": s.network,
                "type": "none",
                "host": s.host,
                "path": s.path,
                "tls": if s.security.is_empty() { "" } else { "tls" },
                "sni": s.sni,
                "alpn": s.alpn,
                "fp": s.fingerprint,
            });
            format!(
                "vmess://{}",
                base64::engine::general_purpose::STANDARD.encode(j.to_string())
            )
        }
        "shadowsocks" => {
            let userinfo = base64::engine::general_purpose::STANDARD
                .encode(format!("{}:{}", s.method, s.password));
            format!("ss://{}@{}:{}#{}", userinfo, s.address, s.port, name)
        }
        "socks" | "http" => {
            let cred = if s.method.is_empty() {
                String::new()
            } else {
                format!("{}:{}@", enc(&s.method), enc(&s.password))
            };
            let scheme = if s.kind == "socks" { "socks5" } else { "http" };
            format!("{scheme}://{cred}{}:{}#{name}", s.address, s.port)
        }
        "hysteria2" => {
            let mut q = vec![];
            if !s.sni.is_empty() {
                q.push(format!("sni={}", enc(&s.sni)));
            }
            if s.allow_insecure {
                q.push("insecure=1".into());
            }
            if !s.obfs.is_empty() {
                q.push(format!("obfs={}", s.obfs));
                q.push(format!("obfs-password={}", enc(&s.obfs_password)));
            }
            let qs = if q.is_empty() { String::new() } else { format!("?{}", q.join("&")) };
            format!("hysteria2://{}@{}:{}{qs}#{name}", enc(&s.password), s.address, s.port)
        }
        "hysteria" => {
            let mut q = vec![format!("auth={}", enc(&s.password))];
            if !s.sni.is_empty() {
                q.push(format!("peer={}", enc(&s.sni)));
            }
            if s.allow_insecure {
                q.push("insecure=1".into());
            }
            if s.up_mbps > 0 {
                q.push(format!("upmbps={}", s.up_mbps));
            }
            if s.down_mbps > 0 {
                q.push(format!("downmbps={}", s.down_mbps));
            }
            if !s.obfs.is_empty() {
                q.push(format!("obfs={}", enc(&s.obfs)));
            }
            format!("hysteria://{}:{}?{}#{name}", s.address, s.port, q.join("&"))
        }
        "tuic" => {
            let mut q = vec![];
            if !s.sni.is_empty() {
                q.push(format!("sni={}", enc(&s.sni)));
            }
            if !s.alpn.is_empty() {
                q.push(format!("alpn={}", enc(&s.alpn)));
            }
            if !s.congestion.is_empty() {
                q.push(format!("congestion_control={}", s.congestion));
            }
            let qs = if q.is_empty() { String::new() } else { format!("?{}", q.join("&")) };
            format!("tuic://{}:{}@{}:{}{qs}#{name}", s.uuid, enc(&s.password), s.address, s.port)
        }
        "amneziawg" if !s.raw_config.is_empty() => s.raw_config.clone(),
        "wireguard" | "amneziawg" => {
            // no universal link — emit the wg-quick .conf instead
            let mut c = String::from("[Interface]\n");
            c.push_str(&format!("PrivateKey = {}\n", s.private_key));
            if !s.local_address.is_empty() {
                c.push_str(&format!("Address = {}\n", s.local_address.join(", ")));
            }
            c.push_str("\n[Peer]\n");
            c.push_str(&format!("PublicKey = {}\n", s.public_key));
            if !s.pre_shared_key.is_empty() {
                c.push_str(&format!("PresharedKey = {}\n", s.pre_shared_key));
            }
            c.push_str(&format!("Endpoint = {}:{}\n", s.address, s.port));
            c.push_str("AllowedIPs = 0.0.0.0/0, ::/0\n");
            c
        }
        "trojan" => {
            let mut q = vec![format!("type={}", s.network)];
            if !s.sni.is_empty() {
                q.push(format!("sni={}", enc(&s.sni)));
            }
            if !s.fingerprint.is_empty() {
                q.push(format!("fp={}", s.fingerprint));
            }
            if !s.path.is_empty() {
                q.push(format!("path={}", enc(&s.path)));
            }
            if !s.host.is_empty() {
                q.push(format!("host={}", enc(&s.host)));
            }
            format!(
                "trojan://{}@{}:{}?{}#{}",
                enc(&s.password),
                s.address,
                s.port,
                q.join("&"),
                name
            )
        }
        _ => {
            // vless
            let mut q = vec![
                format!("type={}", s.network),
                "encryption=none".to_string(),
            ];
            let security = match s.security.as_str() {
                "reality" => "reality",
                "tls" => "tls",
                _ => "none",
            };
            q.push(format!("security={security}"));
            if !s.sni.is_empty() {
                q.push(format!("sni={}", enc(&s.sni)));
            }
            if !s.fingerprint.is_empty() {
                q.push(format!("fp={}", s.fingerprint));
            }
            if !s.flow.is_empty() {
                q.push(format!("flow={}", s.flow));
            }
            if !s.public_key.is_empty() {
                q.push(format!("pbk={}", s.public_key));
            }
            if !s.short_id.is_empty() {
                q.push(format!("sid={}", s.short_id));
            }
            if !s.path.is_empty() {
                q.push(format!("path={}", enc(&s.path)));
            }
            if !s.host.is_empty() {
                q.push(format!("host={}", enc(&s.host)));
            }
            if !s.service_name.is_empty() {
                q.push(format!("serviceName={}", enc(&s.service_name)));
            }
            if !s.alpn.is_empty() {
                q.push(format!("alpn={}", enc(&s.alpn)));
            }
            format!(
                "vless://{}@{}:{}?{}#{}",
                s.uuid,
                s.address,
                s.port,
                q.join("&"),
                name
            )
        }
    }
}

/// Build a shareable single-server config. Xray format for vless/vmess/trojan/ss
/// (widely importable); sing-box format for the rest.
pub fn to_xray_json(s: &Server) -> serde_json::Value {
    if s.kind == "wireguard" {
        return serde_json::json!({ "endpoints": [crate::singbox::wireguard_endpoint(s)] });
    }
    if !matches!(s.kind.as_str(), "vless" | "vmess" | "trojan" | "shadowsocks") {
        return serde_json::json!({ "outbounds": [crate::singbox::outbound_json(s)] });
    }
    let mut stream = serde_json::json!({ "network": if s.network.is_empty() { "tcp" } else { &s.network } });
    match s.security.as_str() {
        "reality" => {
            stream["security"] = "reality".into();
            stream["realitySettings"] = serde_json::json!({
                "serverName": s.sni,
                "fingerprint": if s.fingerprint.is_empty() { "chrome" } else { &s.fingerprint },
                "publicKey": s.public_key,
                "shortId": s.short_id,
                "spiderX": "",
            });
        }
        "tls" => {
            stream["security"] = "tls".into();
            stream["tlsSettings"] = serde_json::json!({
                "serverName": s.sni,
                "fingerprint": if s.fingerprint.is_empty() { "chrome" } else { &s.fingerprint },
                "allowInsecure": s.allow_insecure,
            });
        }
        _ if s.kind == "trojan" => {
            stream["security"] = "tls".into();
            stream["tlsSettings"] = serde_json::json!({ "serverName": s.sni });
        }
        _ => {}
    }
    if s.network == "ws" {
        stream["wsSettings"] = serde_json::json!({
            "path": if s.path.is_empty() { "/" } else { &s.path },
            "headers": { "Host": s.host },
        });
    } else if s.network == "grpc" {
        stream["grpcSettings"] = serde_json::json!({ "serviceName": s.service_name });
    }

    let settings = match s.kind.as_str() {
        "vmess" | "vless" => serde_json::json!({
            "vnext": [{
                "address": s.address,
                "port": s.port,
                "users": [{
                    "id": s.uuid,
                    "encryption": if s.kind == "vless" { "none" } else { "auto" },
                    "flow": s.flow,
                    "alterId": s.alter_id,
                }],
            }],
        }),
        _ => serde_json::json!({
            "servers": [{
                "address": s.address,
                "port": s.port,
                "password": s.password,
                "method": s.method,
            }],
        }),
    };

    serde_json::json!({
        "outbounds": [{
            "tag": s.name,
            "protocol": s.kind,
            "settings": settings,
            "streamSettings": stream,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PBK: &str = "TqjRyZrS2UGkibrm1xuT-5vw-obXvdMNaKyIYqUAej0";

    #[test]
    fn vless_reality_standard() {
        let link = format!(
            "vless://afa63015-c6c7-40b1-b543-6a424e1a43ea@89.125.37.108:443?type=tcp&security=reality&pbk={PBK}&sni=www.googletagmanager.com&sid=1d2649c3bed72c88&fp=chrome&flow=xtls-rprx-vision#FI"
        );
        let s = parse_link(&link).unwrap();
        assert_eq!(s.kind, "vless");
        assert_eq!(s.address, "89.125.37.108");
        assert_eq!(s.port, 443);
        assert_eq!(s.uuid, "afa63015-c6c7-40b1-b543-6a424e1a43ea");
        assert_eq!(s.security, "reality");
        assert_eq!(s.public_key, PBK);
        assert_eq!(s.flow, "xtls-rprx-vision");
        assert_eq!(s.sni, "www.googletagmanager.com");
        assert_eq!(s.name, "FI");
    }

    #[test]
    fn xray_json_vless_reality() {
        let json = format!(
            r#"{{"outbounds":[{{"protocol":"vless","tag":"MyServer","settings":{{"vnext":[{{"address":"89.125.37.108","port":443,"users":[{{"encryption":"none","flow":"xtls-rprx-vision","id":"afa63015-c6c7-40b1-b543-6a424e1a43ea"}}]}}]}},"streamSettings":{{"network":"tcp","security":"reality","realitySettings":{{"fingerprint":"chrome","publicKey":"{PBK}","serverName":"www.googletagmanager.com","shortId":"1d2649c3bed72c88"}}}}}},{{"protocol":"freedom","tag":"direct"}}]}}"#
        );
        let servers = parse_any(&json);
        assert_eq!(servers.len(), 1);
        let s = &servers[0];
        assert_eq!(s.kind, "vless");
        assert_eq!(s.address, "89.125.37.108");
        assert_eq!(s.uuid, "afa63015-c6c7-40b1-b543-6a424e1a43ea");
        assert_eq!(s.security, "reality");
        assert_eq!(s.public_key, PBK);
        assert_eq!(s.short_id, "1d2649c3bed72c88");
        assert_eq!(s.flow, "xtls-rprx-vision");
        assert_eq!(s.name, "MyServer");
    }

    #[test]
    fn xray_json_vmess_ws() {
        let json = r#"{"outbounds":[{"protocol":"vmess","settings":{"vnext":[{"address":"a.example.com","port":443,"users":[{"id":"b831381d-6324-4d53-ad4f-8cda48b30811","alterId":0}]}]},"streamSettings":{"network":"ws","security":"tls","tlsSettings":{"serverName":"a.example.com"},"wsSettings":{"path":"/vm","headers":{"Host":"a.example.com"}}}}]}"#;
        let servers = parse_any(json);
        assert_eq!(servers.len(), 1);
        let s = &servers[0];
        assert_eq!(s.kind, "vmess");
        assert_eq!(s.network, "ws");
        assert_eq!(s.path, "/vm");
        assert_eq!(s.host, "a.example.com");
        assert_eq!(s.security, "tls");
    }

    #[test]
    fn vless_base64_wrapped_authority() {
        // some generators emit vless://base64(":uuid@host:port")?params
        let inner = ":6a550bd2-905c-4b28-a49e-197d71823ebf@89.125.37.108:443";
        let b64 = base64::engine::general_purpose::STANDARD.encode(inner);
        let link = format!("vless://{b64}?security=reality&pbk={PBK}&sid=1d2649c3bed72c88&type=tcp&flow=xtls-rprx-vision#B64");
        let s = parse_link(&link).unwrap();
        assert_eq!(s.address, "89.125.37.108");
        assert_eq!(s.port, 443);
        assert_eq!(s.uuid, "6a550bd2-905c-4b28-a49e-197d71823ebf");
        assert_eq!(s.security, "reality");
        assert_eq!(s.public_key, PBK);
    }

    #[test]
    fn vmess_json_link() {
        let json = r#"{"v":"2","ps":"tokyo","add":"jp.example.com","port":"443","id":"b831381d-6324-4d53-ad4f-8cda48b30811","aid":"0","net":"ws","path":"/ray","host":"jp.example.com","tls":"tls"}"#;
        let b64 = base64::engine::general_purpose::STANDARD.encode(json);
        let s = parse_link(&format!("vmess://{b64}")).unwrap();
        assert_eq!(s.kind, "vmess");
        assert_eq!(s.address, "jp.example.com");
        assert_eq!(s.network, "ws");
        assert_eq!(s.path, "/ray");
        assert_eq!(s.security, "tls");
        assert_eq!(s.name, "tokyo");
    }

    #[test]
    fn ss_sip002() {
        let s = parse_link("ss://YWVzLTI1Ni1nY206cGFzcw==@1.2.3.4:8388#us").unwrap();
        assert_eq!(s.kind, "shadowsocks");
        assert_eq!(s.method, "aes-256-gcm");
        assert_eq!(s.password, "pass");
        assert_eq!(s.address, "1.2.3.4");
        assert_eq!(s.port, 8388);
    }

    #[test]
    fn parse_many_base64_subscription() {
        let body = "vless://afa63015-c6c7-40b1-b543-6a424e1a43ea@1.1.1.1:443?security=reality&pbk=x#a\ntrojan://pw@2.2.2.2:443#b";
        let b64 = base64::engine::general_purpose::STANDARD.encode(body);
        assert_eq!(parse_many(&b64).len(), 2);
    }

    #[test]
    fn socks_and_http() {
        let s = parse_link("socks5://u:p@1.2.3.4:1080#S").unwrap();
        assert_eq!(s.kind, "socks");
        assert_eq!(s.method, "u");
        assert_eq!(s.password, "p");
        assert_eq!(s.port, 1080);
        let h = parse_link("http://1.2.3.4:8080").unwrap();
        assert_eq!(h.kind, "http");
        assert_eq!(h.port, 8080);
    }

    #[test]
    fn hysteria2_link() {
        let s = parse_link("hysteria2://secret@a.com:443?sni=bing.com&insecure=1&obfs=salamander&obfs-password=xyz#HY2").unwrap();
        assert_eq!(s.kind, "hysteria2");
        assert_eq!(s.password, "secret");
        assert_eq!(s.sni, "bing.com");
        assert!(s.allow_insecure);
        assert_eq!(s.obfs, "salamander");
        assert_eq!(s.obfs_password, "xyz");
    }

    #[test]
    fn tuic_link() {
        let s = parse_link("tuic://b831381d-6324-4d53-ad4f-8cda48b30811:pw@a.com:443?sni=x.com&congestion_control=bbr&alpn=h3#T").unwrap();
        assert_eq!(s.kind, "tuic");
        assert_eq!(s.uuid, "b831381d-6324-4d53-ad4f-8cda48b30811");
        assert_eq!(s.password, "pw");
        assert_eq!(s.congestion, "bbr");
    }

    #[test]
    fn wireguard_conf() {
        let conf = "[Interface]\nPrivateKey = KEY1\nAddress = 10.0.0.2/32\n[Peer]\nPublicKey = KEY2\nEndpoint = 1.2.3.4:51820\nAllowedIPs = 0.0.0.0/0\n";
        let v = parse_any_result(conf).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, "wireguard");
        assert_eq!(v[0].private_key, "KEY1");
        assert_eq!(v[0].public_key, "KEY2");
        assert_eq!(v[0].port, 51820);
    }

    #[test]
    fn amneziawg_detected() {
        let conf = "[Interface]\nPrivateKey = K\nAddress = 10.0.0.2/32\nJc = 4\nJmin = 8\nS1 = 15\nH1 = 1\n[Peer]\nPublicKey = P\nEndpoint = 1.2.3.4:4500\nAllowedIPs = 0.0.0.0/0\n";
        let v = parse_any_result(conf).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, "amneziawg");
        assert!(v[0].raw_config.contains("Jc = 4"));
        // raw_config round-trips verbatim through to_link
        assert_eq!(to_link(&v[0]), conf.trim());
    }

    #[test]
    fn link_round_trips_new_protocols() {
        for link in [
            "socks5://u:p@1.2.3.4:1080#S",
            "hysteria2://pw@a.com:443?sni=b.com&obfs=salamander&obfs-password=x#H",
            "tuic://uuid-x:pw@a.com:443?sni=b.com&congestion_control=bbr#T",
        ] {
            let s = parse_link(link).unwrap();
            let s2 = parse_link(&to_link(&s)).unwrap();
            assert_eq!(s.address, s2.address, "{link}");
            assert_eq!(s.kind, s2.kind, "{link}");
        }
    }
}
