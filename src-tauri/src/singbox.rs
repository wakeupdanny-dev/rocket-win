use crate::model::{Server, Settings};
use serde_json::{json, Value};

const GEOSITE_RU: &str = "https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-category-ru.srs";
const GEOIP_RU: &str = "https://raw.githubusercontent.com/SagerNet/sing-geoip/rule-set/geoip-ru.srs";
const RU_DNS: &str = "77.88.8.8"; // Yandex — resolves .ru domains without going through the tunnel
// must match rocket-service's AWG_IFACE — the wintun adapter name amneziawg.exe is
// told to create, which sing-box's "proxy" outbound then binds to for AmneziaWG.
pub const AWG_IFACE_NAME: &str = "RocketAWG";

/// Store / launcher / torrent clients that should download at full local speed
/// instead of saturating the shared VPN exit. Matched by process name.
const DOWNLOAD_APPS: &[&str] = &[
    // game stores & launchers
    "steam.exe",
    "steamwebhelper.exe",
    "steamservice.exe",
    "EpicGamesLauncher.exe",
    "EpicWebHelper.exe",
    "Battle.net.exe",
    "Agent.exe", // Battle.net updater
    "GalaxyClient.exe",
    "GalaxyClientService.exe",
    "UbisoftConnect.exe",
    "upc.exe",
    "UplayWebCore.exe",
    "EADesktop.exe",
    "EABackgroundService.exe",
    "Origin.exe",
    "RockstarService.exe",
    "Launcher.exe", // Rockstar / others
    // torrent clients
    "qbittorrent.exe",
    "uTorrent.exe",
    "BitTorrent.exe",
    "transmission-qt.exe",
    "transmission-daemon.exe",
    "deluge.exe",
    "deluged.exe",
    "BitComet.exe",
    "tixati.exe",
];

/// Build a TLS block for a server, or `Value::Null` if plaintext.
fn tls_block(s: &Server) -> Value {
    let needs_tls = s.kind == "trojan" || s.security == "tls" || s.security == "reality";
    if !needs_tls {
        return Value::Null;
    }
    let sni = if !s.sni.is_empty() {
        s.sni.clone()
    } else if !s.host.is_empty() {
        s.host.clone()
    } else {
        s.address.clone()
    };
    let mut tls = json!({
        "enabled": true,
        "server_name": sni,
        "insecure": s.allow_insecure,
    });
    if !s.alpn.is_empty() {
        tls["alpn"] = json!(s.alpn.split(',').map(|x| x.trim()).collect::<Vec<_>>());
    }
    // QUIC-based protocols don't use uTLS
    let quic = matches!(s.kind.as_str(), "hysteria" | "hysteria2" | "tuic");
    if !quic {
        let fp = if s.fingerprint.is_empty() {
            "chrome".to_string()
        } else {
            s.fingerprint.clone()
        };
        tls["utls"] = json!({ "enabled": true, "fingerprint": fp });
    }
    if s.security == "reality" {
        tls["reality"] = json!({
            "enabled": true,
            "public_key": s.public_key,
            "short_id": s.short_id,
        });
    }
    tls
}

/// Build a transport block, or `Value::Null` for plain TCP.
fn transport_block(s: &Server) -> Value {
    match s.network.as_str() {
        "ws" => {
            let mut t = json!({ "type": "ws", "path": if s.path.is_empty() { "/" } else { &s.path } });
            if !s.host.is_empty() {
                t["headers"] = json!({ "Host": s.host });
            }
            t
        }
        "grpc" => {
            let name = if !s.service_name.is_empty() {
                s.service_name.clone()
            } else {
                s.path.trim_start_matches('/').to_string()
            };
            json!({ "type": "grpc", "service_name": name })
        }
        "http" | "h2" => json!({
            "type": "http",
            "host": if s.host.is_empty() { vec![] } else { vec![s.host.clone()] },
            "path": if s.path.is_empty() { "/".into() } else { s.path.clone() },
        }),
        "httpupgrade" => {
            let mut t = json!({ "type": "httpupgrade", "path": if s.path.is_empty() { "/" } else { &s.path } });
            if !s.host.is_empty() {
                t["host"] = json!(s.host);
            }
            t
        }
        _ => Value::Null,
    }
}

/// The proxy outbound (tag "proxy").
pub fn outbound_json(s: &Server) -> Value {
    let mut o = match s.kind.as_str() {
        "vless" => {
            let mut v = json!({
                "type": "vless",
                "uuid": s.uuid,
                "packet_encoding": "xudp",
            });
            if !s.flow.is_empty() {
                v["flow"] = json!(s.flow);
            }
            v
        }
        "vmess" => json!({
            "type": "vmess",
            "uuid": s.uuid,
            "security": "auto",
            "alter_id": s.alter_id,
        }),
        "trojan" => json!({ "type": "trojan", "password": s.password }),
        "shadowsocks" => json!({
            "type": "shadowsocks",
            "method": s.method,
            "password": s.password,
        }),
        "socks" => {
            let mut v = json!({ "type": "socks", "version": "5" });
            if !s.method.is_empty() {
                v["username"] = json!(s.method);
                v["password"] = json!(s.password);
            }
            v
        }
        "http" => {
            let mut v = json!({ "type": "http" });
            if !s.method.is_empty() {
                v["username"] = json!(s.method);
                v["password"] = json!(s.password);
            }
            v
        }
        "hysteria2" => {
            let mut v = json!({ "type": "hysteria2", "password": s.password });
            if s.up_mbps > 0 {
                v["up_mbps"] = json!(s.up_mbps);
            }
            if s.down_mbps > 0 {
                v["down_mbps"] = json!(s.down_mbps);
            }
            if !s.obfs.is_empty() {
                v["obfs"] = json!({ "type": s.obfs, "password": s.obfs_password });
            }
            v
        }
        "hysteria" => {
            let mut v = json!({
                "type": "hysteria",
                "auth_str": s.password,
                "up_mbps": if s.up_mbps == 0 { 50 } else { s.up_mbps },
                "down_mbps": if s.down_mbps == 0 { 100 } else { s.down_mbps },
            });
            if !s.obfs.is_empty() {
                v["obfs"] = json!(s.obfs);
            }
            v
        }
        "tuic" => json!({
            "type": "tuic",
            "uuid": s.uuid,
            "password": s.password,
            "congestion_control": if s.congestion.is_empty() { "bbr" } else { &s.congestion },
        }),
        other => json!({ "type": other }),
    };
    o["tag"] = json!("proxy");
    o["server"] = json!(s.address);
    o["server_port"] = json!(s.port);

    let tls = tls_block(s);
    if !tls.is_null() {
        o["tls"] = tls;
    }
    let tr = transport_block(s);
    if !tr.is_null() {
        o["transport"] = tr;
    }
    o
}

/// WireGuard is a sing-box *endpoint*, not an outbound. Returns the endpoint
/// object (tag "proxy").
pub fn wireguard_endpoint(s: &Server) -> Value {
    let addr = if s.local_address.is_empty() {
        vec!["172.16.0.2/32".to_string()]
    } else {
        s.local_address.clone()
    };
    let mut peer = json!({
        "address": s.address,
        "port": s.port,
        "public_key": s.public_key,
        "allowed_ips": ["0.0.0.0/0", "::/0"],
    });
    if !s.pre_shared_key.is_empty() {
        peer["pre_shared_key"] = json!(s.pre_shared_key);
    }
    json!({
        "type": "wireguard",
        "tag": "proxy",
        "system": false,
        "mtu": 1408,
        "address": addr,
        "private_key": s.private_key,
        "peers": [peer],
    })
}

fn dns_server_from_url(dns: &str) -> Value {
    // `detour: proxy` forces resolution through the tunnel (avoids DNS leaks).
    if let Some(rest) = dns.strip_prefix("https://") {
        let host = rest.split('/').next().unwrap_or("1.1.1.1");
        json!({ "type": "https", "tag": "remote-dns", "server": host, "detour": "proxy" })
    } else if let Some(rest) = dns.strip_prefix("tls://") {
        json!({ "type": "tls", "tag": "remote-dns", "server": rest, "detour": "proxy" })
    } else {
        json!({ "type": "udp", "tag": "remote-dns", "server": dns, "detour": "proxy" })
    }
}

/// Full config. `mode` is "global" (everything through the proxy) or
/// "bypass_ru" (Russian sites/IPs go direct, everything else through the proxy).
pub fn build_config(
    server: &Server,
    mode: &str,
    settings: &Settings,
    cache_path: &str,
    zapret_domains: &[String],
) -> Value {
    let listen_port = settings.listen_port;
    let clash_port = if settings.clash_api_port == 0 {
        9191
    } else {
        settings.clash_api_port
    };

    // TUN is the only capture mode; the local mixed inbound stays for apps that
    // want to point at it explicitly.
    let inbounds = vec![
        json!({
            "type": "mixed",
            "tag": "mixed-in",
            "listen": "127.0.0.1",
            "listen_port": listen_port,
        }),
        json!({
            "type": "tun",
            "tag": "tun-in",
            // IPv4 only — most proxy servers have no working IPv6 egress, and a
            // dual-stack TUN makes apps try (and hang on) AAAA addresses.
            "address": ["172.19.0.1/30"],
            "mtu": 1500,
            "auto_route": true,
            // strict_route breaks LAN + can wedge loopback on Windows; keep it off
            "strict_route": false,
            "stack": "mixed",
        }),
    ];

    let is_wg = server.kind == "wireguard";
    let is_awg = server.kind == "amneziawg";
    let outbounds = if is_wg {
        vec![json!({ "type": "direct", "tag": "direct" })]
    } else if is_awg {
        // amneziawg.exe already speaks the (obfuscated) WireGuard protocol on its
        // own adapter, brought up by the service as a non-default uplink — sing-box
        // just has to push "proxy" traffic out through that named interface. This
        // is what lets the routing rules below (RU bypass / zapret / downloads)
        // apply to AmneziaWG exactly like any other protocol.
        vec![
            json!({ "type": "direct", "tag": "proxy", "bind_interface": AWG_IFACE_NAME }),
            json!({ "type": "direct", "tag": "direct" }),
        ]
    } else {
        vec![
            outbound_json(server),
            json!({ "type": "direct", "tag": "direct" }),
        ]
    };
    let endpoints: Vec<Value> = if is_wg {
        vec![wireguard_endpoint(server)]
    } else {
        vec![]
    };

    // ---- route ----
    let mut route_rules = vec![
        json!({ "action": "sniff" }),
        json!({ "protocol": "dns", "action": "hijack-dns" }),
        json!({ "ip_is_private": true, "outbound": "direct" }),
    ];

    // store/launcher/torrent downloads bypass the tunnel for full local speed
    if settings.download_bypass {
        route_rules.push(json!({ "process_name": DOWNLOAD_APPS, "outbound": "direct" }));
    }

    // domains handled by an external DPI-bypass tool (zapret) must skip the
    // tunnel so the tool can work on the real connection.
    if !zapret_domains.is_empty() {
        route_rules.push(json!({ "domain_suffix": zapret_domains, "outbound": "direct" }));
    }

    let mut rule_sets: Vec<Value> = vec![];
    let bypass_ru = mode == "bypass_ru";

    if bypass_ru {
        // Russian domains + Russian IPs bypass the tunnel; everything else uses it.
        route_rules.push(json!({ "rule_set": ["geosite-ru", "geoip-ru"], "outbound": "direct" }));
        // no download_detour (deprecated in 1.14): the rule-set URLs are foreign
        // hosts, so they download through the tunnel via the default route anyway.
        rule_sets = vec![
            json!({ "type": "remote", "tag": "geosite-ru", "format": "binary", "url": GEOSITE_RU }),
            json!({ "type": "remote", "tag": "geoip-ru", "format": "binary", "url": GEOIP_RU }),
        ];
    }
    let final_outbound = "proxy";

    let dns_remote = dns_server_from_url(&settings.dns);
    let mut dns_rules: Vec<Value> = vec![];
    if bypass_ru {
        // resolve .ru names with a Russian resolver so geoip-ru sees the real IPs
        dns_rules.push(json!({ "rule_set": ["geosite-ru"], "server": "ru-dns" }));
    }
    let mut config = json!({
        "log": { "level": "info", "timestamp": true },
        "dns": {
            "servers": [
                dns_remote,
                { "type": "udp", "tag": "ru-dns", "server": RU_DNS }
            ],
            "rules": dns_rules,
            "final": "remote-dns",
            "strategy": "ipv4_only"
        },
        "inbounds": inbounds,
        "outbounds": outbounds,
        "route": {
            "rules": route_rules,
            "rule_set": rule_sets,
            "final": final_outbound,
            "auto_detect_interface": true,
            "default_domain_resolver": "ru-dns"
        },
        "experimental": {
            "clash_api": { "external_controller": format!("127.0.0.1:{clash_port}") },
            "cache_file": { "enabled": true, "path": cache_path }
        }
    });

    if !endpoints.is_empty() {
        config["endpoints"] = json!(endpoints);
    }
    config
}

/// A minimal config used purely to measure latency of one server (URLTest).
#[allow(dead_code)]
pub fn latency_probe_config(server: &Server, socks_port: u16) -> Value {
    json!({
        "log": { "level": "error" },
        "inbounds": [{
            "type": "mixed", "tag": "probe-in",
            "listen": "127.0.0.1", "listen_port": socks_port
        }],
        "outbounds": [ outbound_json(server), { "type": "direct", "tag": "direct" } ],
        "route": { "final": "proxy" }
    })
}
