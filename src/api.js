// Thin wrapper around Tauri commands with a browser fallback so `vite` dev
// (outside the Tauri shell) still renders with mock data.

const inTauri = typeof window !== "undefined" && !!window.__TAURI_INTERNALS__;

let _invoke = async () => {
  throw new Error("not in tauri");
};
let _listen = async () => () => {};

if (inTauri) {
  const core = await import("@tauri-apps/api/core");
  const event = await import("@tauri-apps/api/event");
  _invoke = core.invoke;
  _listen = event.listen;
}

// ---- mock state for browser dev ----
const mock = {
  state: { connected: false, selected: "m1", mode: "bypass_ru", listen_port: 1089, elevated: true },
  servers: [
    { id: "m3", name: "🇺🇸 Los Angeles", type: "vless", address: "us1.example.com", port: 8443, latency: 240, from_sub: null },
    { id: "m1", name: "🇯🇵 Tokyo 01", type: "vless", address: "jp1.example.com", port: 443, latency: 78, from_sub: "sub.example.com" },
    { id: "m2", name: "🇸🇬 Singapore", type: "vmess", address: "sg1.example.com", port: 443, latency: 132, from_sub: "sub.example.com" },
  ],
  traffic: { up: 0, down: 0, up_total: 0, down_total: 0 },
  settings: {
    autostart: false, autoconnect: false, lang: "", listen_port: 1089, dns: "https://1.1.1.1/dns-query", sub_update_hours: 12, zapret_dir: "", zapret_mode: false, fallback: false, download_bypass: false,
    subscriptions: [
      { name: "sub.example.com", url: "https://sub.example.com/abc", upload: 0, download: 246_000_000_000, total: 500_000_000_000, expire: 1787000000, updated: 1789000000, count: 2 },
    ],
  },
};

async function call(cmd, args) {
  if (inTauri) return _invoke(cmd, args);
  // browser fallback
  switch (cmd) {
    case "get_state": return { ...mock.state };
    case "list_servers": return mock.servers.map((s) => ({ ...s }));
    case "get_settings": return { ...mock.settings };
    case "get_traffic": return { ...mock.traffic };
    case "get_logs": return ["[mock] running in browser — start via `npm run tauri dev`"];
    case "select_server": mock.state.selected = args.id; return null;
    case "set_mode": mock.state.mode = args.mode; return null;
    case "detect_zapret": return mock.settings.zapret_dir || "";
    case "pick_zapret_dir": mock.settings.zapret_dir = "C:\\Users\\me\\Desktop\\zapret"; return mock.settings.zapret_dir;
    case "export_link": return "vless://mock-uuid@example.com:443?type=tcp&security=reality#Mock";
    case "export_json": return JSON.stringify({ outbounds: [{ protocol: "vless", tag: "Mock" }] }, null, 2);
    case "export_qr": return `<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 10 10"><rect width="10" height="10" fill="#fff"/><rect x="1" y="1" width="3" height="3"/><rect x="6" y="1" width="3" height="3"/><rect x="1" y="6" width="3" height="3"/></svg>`;
    case "save_config": return null;
    case "connect": mock.state.connected = true; return null;
    case "disconnect": mock.state.connected = false; return null;
    case "delete_server": mock.servers = mock.servers.filter((s) => s.id !== args.id); return null;
    case "test_latency": return Math.floor(40 + Math.random() * 300);
    case "os_language": return (navigator.language || "en").toLowerCase().startsWith("ru") ? "ru" : "en";
    case "app_version": return "0.1.1";
    case "check_update": return { current: "0.1.1", latest: "0.1.1", available: false, url: "https://github.com/wakeupdanny-dev/rocket-win/releases/latest", download_url: null };
    case "open_url": return null;
    case "add_servers_from_text": return 1;
    case "import_from_file": return 1;
    case "add_subscription": return 3;
    case "update_subscriptions": return mock.servers.length;
    case "set_settings": Object.assign(mock.settings, args.settings); return null;
    default: return null;
  }
}

export const api = {
  getState: () => call("get_state"),
  listServers: () => call("list_servers"),
  selectServer: (id) => call("select_server", { id }),
  deleteServer: (id) => call("delete_server", { id }),
  setMode: (mode) => call("set_mode", { mode }),
  detectZapret: () => call("detect_zapret"),
  pickZapretDir: () => call("pick_zapret_dir"),
  exportLink: (id) => call("export_link", { id }),
  exportJson: (id) => call("export_json", { id }),
  exportQr: (id) => call("export_qr", { id }),
  saveConfig: (id, format) => call("save_config", { id, format }),
  connect: () => call("connect"),
  disconnect: () => call("disconnect"),
  testLatency: (id) => call("test_latency", { id }),
  addServersFromText: (text) => call("add_servers_from_text", { text }),
  importFromFile: () => call("import_from_file"),
  addSubscription: (name, url) => call("add_subscription", { name, url }),
  updateSubscriptions: () => call("update_subscriptions"),
  getTraffic: () => call("get_traffic"),
  getSettings: () => call("get_settings"),
  osLanguage: () => call("os_language"),
  appVersion: () => call("app_version"),
  checkUpdate: () => call("check_update"),
  openUrl: (url) => call("open_url", { url }),
  setSettings: (settings) => call("set_settings", { settings }),
  getLogs: () => call("get_logs"),
  on: (evt, cb) => (inTauri ? _listen(evt, (e) => cb(e.payload)) : Promise.resolve(() => {})),
};
