use serde::{Deserialize, Serialize};

/// A single proxy endpoint parsed from a share link or subscription.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Server {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String, // vless | vmess | trojan | shadowsocks
    pub address: String,
    pub port: u16,

    // credentials
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub method: String, // shadowsocks cipher
    #[serde(default)]
    pub alter_id: u16, // vmess

    // transport
    #[serde(default = "default_network")]
    pub network: String, // tcp | ws | grpc | http
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub host: String, // ws Host header / http host
    #[serde(default)]
    pub service_name: String, // grpc

    // tls / reality
    #[serde(default)]
    pub security: String, // "", "tls", "reality"
    #[serde(default)]
    pub sni: String,
    #[serde(default)]
    pub alpn: String,
    #[serde(default)]
    pub fingerprint: String, // utls fp
    #[serde(default)]
    pub flow: String, // vless flow
    #[serde(default)]
    pub public_key: String, // reality pbk
    #[serde(default)]
    pub short_id: String, // reality sid
    #[serde(default)]
    pub allow_insecure: bool,

    // hysteria / hysteria2 / tuic
    #[serde(default)]
    pub up_mbps: u32,
    #[serde(default)]
    pub down_mbps: u32,
    #[serde(default)]
    pub obfs: String, // hy2: "salamander"; hy1: obfs string
    #[serde(default)]
    pub obfs_password: String,
    #[serde(default)]
    pub congestion: String, // tuic congestion_control (bbr / cubic / new_reno)

    // wireguard / amneziawg
    #[serde(default)]
    pub private_key: String,
    #[serde(default)]
    pub pre_shared_key: String,
    #[serde(default)]
    pub local_address: Vec<String>, // Interface Address(es)
    #[serde(default)]
    pub reserved: Vec<u8>,
    #[serde(default)]
    pub raw_config: String, // amneziawg: the whole wg-quick .conf (has junk params)

    // bookkeeping
    #[serde(default)]
    pub from_sub: Option<String>,
    #[serde(default)]
    pub latency: Option<i64>, // ms, -1 = timeout
}

fn default_network() -> String {
    "tcp".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub name: String,
    pub url: String,
    // parsed from the `Subscription-Userinfo` response header, bytes
    #[serde(default)]
    pub upload: u64,
    #[serde(default)]
    pub download: u64,
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub expire: u64, // unix seconds, 0 = none
    #[serde(default)]
    pub updated: u64, // unix seconds of last successful fetch
    #[serde(default)]
    pub count: usize, // servers from this subscription
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRule {
    pub kind: String,   // DOMAIN-SUFFIX, IP-CIDR, GEOIP, GEOSITE, ...
    pub value: String,
    pub action: String, // PROXY | DIRECT | REJECT
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "def_port")]
    pub listen_port: u16,
    #[serde(default)]
    pub clash_api_port: u16,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub autoconnect: bool,
    #[serde(default)]
    pub lang: String, // "", "en", "ru"  ("" = follow OS)
    #[serde(default = "def_dns")]
    pub dns: String,
    #[serde(default = "def_sub_hours")]
    pub sub_update_hours: u64, // auto-update subscriptions every N hours; 0 = off
    #[serde(default)]
    pub zapret_dir: String, // path to a zapret folder (has lists/list-*.txt)
    #[serde(default)]
    pub zapret_mode: bool, // route zapret-handled domains direct (bypass the tunnel)
    #[serde(default)]
    pub fallback: bool, // if the tunnel has no internet, switch to another server (lowest latency first)
    #[serde(default)]
    pub download_bypass: bool, // Steam/Epic/torrent clients download outside the tunnel
    #[serde(default)]
    pub subscriptions: Vec<Subscription>,
    // kept only so old store.json files still deserialize
    #[serde(default, skip_serializing)]
    pub custom_rules: Vec<CustomRule>,
    #[serde(default, skip_serializing)]
    pub system_proxy: Option<bool>,
    #[serde(default, skip_serializing)]
    pub tun: Option<bool>,
}

fn def_port() -> u16 {
    1089
}
fn def_dns() -> String {
    "https://1.1.1.1/dns-query".into()
}
fn def_sub_hours() -> u64 {
    12
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            listen_port: 1089,
            clash_api_port: 9191,
            autostart: false,
            autoconnect: false,
            lang: String::new(),
            dns: def_dns(),
            sub_update_hours: def_sub_hours(),
            zapret_dir: String::new(),
            zapret_mode: false,
            fallback: false,
            download_bypass: false,
            subscriptions: vec![],
            custom_rules: vec![],
            system_proxy: None,
            tun: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AppStateDto {
    pub connected: bool,
    pub selected: Option<String>,
    pub mode: String, // global | bypass_ru
    pub listen_port: u16,
    pub elevated: bool, // process has Administrator rights
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Traffic {
    pub up: u64,
    pub down: u64,
    pub up_total: u64,
    pub down_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persisted {
    #[serde(default)]
    pub servers: Vec<Server>,
    #[serde(default)]
    pub selected: Option<String>,
    #[serde(default = "def_mode")]
    pub mode: String,
    #[serde(default)]
    pub settings: Settings,
}

fn def_mode() -> String {
    "bypass_ru".into()
}

impl Default for Persisted {
    fn default() -> Self {
        Persisted {
            servers: vec![],
            selected: None,
            mode: "bypass_ru".into(),
            settings: Settings::default(),
        }
    }
}
