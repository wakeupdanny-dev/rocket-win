// Minimal i18n. `t(key, {vars})` for UI, `tErr(msg)` for backend error codes.

const en = {
  "nav.home": "Home",
  "nav.config": "Config",
  "nav.data": "Data",
  "nav.settings": "Settings",
  "app.title": "Rocket",

  "home.connected": "Connected",
  "home.notConnected": "Not Connected",
  "home.via": "via {name} · :{port}",
  "home.routing": "Routing",
  "home.connTest": "Connectivity Test",
  "home.servers": "Servers",
  "home.localServers": "Local Servers",
  "home.noLocal": "Nothing here — tap + to add",
  "home.noServers": "No servers",
  "home.footer": "Tap a header to fold. Right-click a server for actions.",
  "mode.global": "Global",
  "mode.bypass_ru": "RU → direct",

  "add.title": "Add Server",
  "add.placeholder": "Subscription URL, or vless:// vmess:// trojan:// ss:// socks5:// hysteria2:// tuic:// links — one per line",
  "add.btnAdd": "Add",
  "add.btnFile": "Import from file…",
  "add.hint": "A single http(s) link is fetched as a subscription. Files can be an Xray/v2ray/sing-box JSON config or a WireGuard / AmneziaWG .conf.",
  "add.nothing": "Nothing to add",
  "add.imported": "Imported {n} {n, server}",

  "srv.setDefault": "Set as default",
  "srv.test": "Test latency",
  "srv.copyAddr": "Copy address",
  "srv.share": "Share",
  "srv.qrTooLong": "This config is too large for a QR code — use Copy link, Copy JSON or Save config instead.",
  "srv.copyLink": "Copy link",
  "srv.copyJson": "Copy JSON",
  "srv.saveConfig": "Save config…",
  "srv.saved": "Saved",
  "srv.delete": "Delete",
  "srv.deleted": "Deleted",
  "srv.copied": "Copied",
  "srv.clipboardBlocked": "Clipboard blocked",
  "srv.timeout": "Timeout",
  "srv.udpNoProbe": "WireGuard/AmneziaWG uses UDP — latency can't be probed before connecting",
  "srv.testWhileConnected": "Disconnect to test other servers",
  "srv.switching": "Switching to {name}…",
  "srv.switched": "Connected to {name}",
  "srv.needServer": "Add a server first",

  "routing.title": "Routing",
  "routing.bypass_ru": "Russian sites without VPN",
  "routing.bypass_ru.desc": "RU domains and IPs go direct, everything else through the VPN",
  "routing.global": "Everything through VPN",
  "routing.global.desc": "Every connection is routed through the selected server",
  "routing.toast.bypass_ru": "Russian sites bypass the VPN",
  "routing.toast.global": "All traffic through the VPN",

  "act.updateAll": "Update all subscriptions",
  "act.testAll": "Test all latency",
  "act.sort": "Sort by latency",
  "act.updating": "Updating…",
  "act.updated": "Updated",
  "act.sorted": "Sorted",
  "act.servers": "{n} {n, server}",
  "act.testingN": "Testing {n}…",

  "sub.updateNow": "Update now",
  "sub.copyUrl": "Copy URL",
  "sub.remove": "Remove",
  "sub.until": "until {date}",
  "sub.count": "{n} srv",

  "cfg.routing": "Routing",
  "cfg.bypassRu.title": "Russian sites without VPN",
  "cfg.bypassRu.sub": "Sites and IP addresses in Russia open directly; everything else goes through the VPN",
  "cfg.footer.on": "geosite/geoip «ru» lists download through the tunnel on first connect and are cached.",
  "cfg.footer.off": "Every connection is routed through the selected server.",

  "dl.title": "Downloads without VPN",
  "dl.sub": "Steam, Epic, Battle.net and torrent clients download directly (full speed, real IP)",
  "dl.on": "Store & torrent downloads bypass the VPN",
  "dl.off": "Downloads go through the VPN",

  "zapret.title": "Zapret bypass",
  "zapret.notFound": "zapret folder not found — tap to choose",
  "zapret.notDir": "That folder isn't a zapret install",
  "zapret.folderSet": "zapret folder set",
  "zapret.on": "Sites handled by zapret skip the VPN",
  "zapret.off": "Zapret bypass off",
  "zapret.footer.on": "YouTube, Discord and the other domains from zapret's lists go direct so zapret can unblock them. Lists are re-read on every connect.",

  "data.upload": "Upload",
  "data.download": "Download",
  "data.sentTotal": "Sent total",
  "data.recvTotal": "Received total",
  "data.connection": "Connection",
  "data.mode": "Mode",
  "data.localListen": "Local listen",
  "data.coreLog": "Core log",
  "data.noOutput": "— no output —",
  "data.noServer": "no server selected",

  "set.general": "General",
  "set.startLogin": "Start on login",
  "set.fallback": "Fallback",
  "set.fallbackSub": "If the tunnel has no internet, switch to another server (lowest ping first)",
  "set.autoConnect": "Auto-connect last server",
  "set.language": "Language",
  "set.langAuto": "Auto (system)",
  "set.subscriptions": "Subscriptions",
  "set.autoUpdate": "Auto-update",
  "set.autoUpdateTitle": "Auto-update subscriptions",
  "set.updateAllNow": "Update all now",
  "set.off": "Off",
  "set.everyN": "Every {n} hours",
  "set.hoursShort": "{n} h",
  "set.advanced": "Advanced",
  "set.proxyPort": "Local proxy port",
  "set.portRange": "1025–65535",
  "set.dns": "DNS server",
  "set.openFolder": "Open config folder",
  "set.about": "About",
  "set.core": "Core",
  "set.version": "Version",
  "set.checkUpdate": "Check for updates",
  "set.checking": "checking…",
  "set.upToDate": "up to date",
  "set.updateAvailable": "Version {v} available",
  "set.updateBody": "You have {current}, the latest release is {latest}.",
  "set.download": "Download",
  "set.later": "Later",
  "set.saved": "Saved",
  "set.na": "n/a",

  "common.save": "Save",
  "common.enterUrl": "Enter a URL",

  "misc.qrNA": "QR scanning is not available on desktop",

  "err.no_servers_parsed": "No valid servers found",
  "err.cancelled": "Cancelled",
  "err.file_read_failed": "Could not read the file",
  "err.sub_empty": "Subscription has no valid servers",
  "err.no_server_selected": "No server selected",
  "err.admin_required": "Administrator rights are required",
  "err.relaunching": "Restarting with administrator rights…",
  "err.singbox_missing": "sing-box binary not found",
  "err.singbox_start_failed": "sing-box failed to start",
  "err.server_not_found": "Server not found",
  "err.cant_measure_connected": "Can't measure this while another server holds the tunnel — disconnect to test it",
  "err.update_check_failed": "Couldn't check for updates — no internet or GitHub is unreachable",
  "err.amneziawg_unsupported": "AmneziaWG isn't supported by the sing-box core (junk-packet obfuscation)",
  "err.amneziawg_start_failed": "AmneziaWG failed to start",
  "err.amneziawg_no_config": "This AmneziaWG server has no config",
  "err.service_required": "The Rocket helper service is needed for AmneziaWG — reinstall Rocket",
  "err.not_a_zapret_dir": "That folder isn't a zapret install",
};

const ru = {
  "nav.home": "Главная",
  "nav.config": "Конфиг",
  "nav.data": "Данные",
  "nav.settings": "Настройки",
  "app.title": "Rocket",

  "home.connected": "Подключено",
  "home.notConnected": "Не подключено",
  "home.via": "через {name} · :{port}",
  "home.routing": "Маршрутизация",
  "home.connTest": "Тест соединения",
  "home.servers": "Серверы",
  "home.localServers": "Локальные серверы",
  "home.noLocal": "Пусто — нажмите + чтобы добавить",
  "home.noServers": "Нет серверов",
  "home.footer": "Нажмите на заголовок, чтобы свернуть. ПКМ по серверу — действия.",
  "mode.global": "Всё через VPN",
  "mode.bypass_ru": "RU — напрямую",

  "add.title": "Добавить сервер",
  "add.placeholder": "URL подписки, или ссылки vless:// vmess:// trojan:// ss:// socks5:// hysteria2:// tuic:// — по одной на строку",
  "add.btnAdd": "Добавить",
  "add.btnFile": "Импорт из файла…",
  "add.hint": "Одна http(s)-ссылка загружается как подписка. Файл — JSON-конфиг Xray/v2ray/sing-box или WireGuard / AmneziaWG .conf.",
  "add.nothing": "Нечего добавлять",
  "add.imported": "Добавлено серверов: {n}",

  "srv.setDefault": "Сделать основным",
  "srv.test": "Проверить пинг",
  "srv.copyAddr": "Копировать адрес",
  "srv.share": "Поделиться",
  "srv.qrTooLong": "Этот конфиг слишком большой для QR-кода — используйте «Скопировать ссылку», JSON или сохранение файла.",
  "srv.copyLink": "Копировать ссылку",
  "srv.copyJson": "Копировать JSON",
  "srv.saveConfig": "Сохранить конфиг…",
  "srv.saved": "Сохранено",
  "srv.delete": "Удалить",
  "srv.deleted": "Удалено",
  "srv.copied": "Скопировано",
  "srv.clipboardBlocked": "Буфер обмена недоступен",
  "srv.timeout": "Таймаут",
  "srv.udpNoProbe": "WireGuard/AmneziaWG работает по UDP — пинг до подключения не измерить",
  "srv.testWhileConnected": "Отключитесь, чтобы проверить другие серверы",
  "srv.switching": "Переключаю на {name}…",
  "srv.switched": "Подключено к {name}",
  "srv.needServer": "Сначала добавьте сервер",

  "routing.title": "Маршрутизация",
  "routing.bypass_ru": "Российские сайты без VPN",
  "routing.bypass_ru.desc": "Домены и IP России — напрямую, остальное — через VPN",
  "routing.global": "Весь трафик через VPN",
  "routing.global.desc": "Каждое соединение идёт через выбранный сервер",
  "routing.toast.bypass_ru": "Российские сайты мимо VPN",
  "routing.toast.global": "Весь трафик через VPN",

  "act.updateAll": "Обновить все подписки",
  "act.testAll": "Проверить пинг у всех",
  "act.sort": "Сортировать по пингу",
  "act.updating": "Обновление…",
  "act.updated": "Обновлено",
  "act.sorted": "Отсортировано",
  "act.servers": "серверов: {n}",
  "act.testingN": "Проверка {n}…",

  "sub.updateNow": "Обновить сейчас",
  "sub.copyUrl": "Копировать URL",
  "sub.remove": "Удалить",
  "sub.until": "до {date}",
  "sub.count": "{n} серв.",

  "cfg.routing": "Маршрутизация",
  "cfg.bypassRu.title": "Российские сайты без VPN",
  "cfg.bypassRu.sub": "Сайты и IP-адреса России открываются напрямую, всё остальное — через VPN",
  "cfg.footer.on": "Списки geosite/geoip «ru» загружаются через туннель при первом подключении и кэшируются.",
  "cfg.footer.off": "Каждое соединение идёт через выбранный сервер.",

  "dl.title": "Загрузки без VPN",
  "dl.sub": "Steam, Epic, Battle.net и торрент-клиенты качают напрямую (полная скорость, реальный IP)",
  "dl.on": "Загрузки магазинов и торрентов идут мимо VPN",
  "dl.off": "Загрузки идут через VPN",

  "zapret.title": "Обход через zapret",
  "zapret.notFound": "папка zapret не найдена — нажмите, чтобы выбрать",
  "zapret.notDir": "Это не папка zapret",
  "zapret.folderSet": "папка zapret выбрана",
  "zapret.on": "Сайты из zapret идут мимо VPN",
  "zapret.off": "Обход через zapret выключен",
  "zapret.footer.on": "YouTube, Discord и остальные домены из списков zapret идут напрямую, чтобы zapret их разблокировал. Списки перечитываются при каждом подключении.",

  "data.upload": "Отдача",
  "data.download": "Загрузка",
  "data.sentTotal": "Отправлено всего",
  "data.recvTotal": "Принято всего",
  "data.connection": "Соединение",
  "data.mode": "Режим",
  "data.localListen": "Локальный порт",
  "data.coreLog": "Лог ядра",
  "data.noOutput": "— нет вывода —",
  "data.noServer": "сервер не выбран",

  "set.general": "Основные",
  "set.startLogin": "Запуск при входе в систему",
  "set.fallback": "Резервное подключение",
  "set.fallbackSub": "Если в туннеле нет интернета — переключиться на другой сервер (сначала с меньшим пингом)",
  "set.autoConnect": "Автоподключение к последнему серверу",
  "set.language": "Язык",
  "set.langAuto": "Авто (система)",
  "set.subscriptions": "Подписки",
  "set.autoUpdate": "Автообновление",
  "set.autoUpdateTitle": "Автообновление подписок",
  "set.updateAllNow": "Обновить все сейчас",
  "set.off": "Выкл",
  "set.everyN": "Каждые {n} ч",
  "set.hoursShort": "{n} ч",
  "set.advanced": "Дополнительно",
  "set.proxyPort": "Локальный порт прокси",
  "set.portRange": "1025–65535",
  "set.dns": "DNS-сервер",
  "set.openFolder": "Открыть папку конфигов",
  "set.about": "О программе",
  "set.core": "Ядро",
  "set.version": "Версия",
  "set.checkUpdate": "Проверить обновления",
  "set.checking": "проверка…",
  "set.upToDate": "актуальная версия",
  "set.updateAvailable": "Доступна версия {v}",
  "set.updateBody": "У вас {current}, последний релиз — {latest}.",
  "set.download": "Скачать",
  "set.later": "Позже",
  "set.saved": "Сохранено",
  "set.na": "недоступно",

  "common.save": "Сохранить",
  "common.enterUrl": "Введите URL",

  "misc.qrNA": "Сканирование QR недоступно на десктопе",

  "err.no_servers_parsed": "Не найдено ни одного валидного сервера",
  "err.cancelled": "Отменено",
  "err.file_read_failed": "Не удалось прочитать файл",
  "err.sub_empty": "В подписке нет валидных серверов",
  "err.no_server_selected": "Сервер не выбран",
  "err.admin_required": "Нужны права администратора",
  "err.relaunching": "Перезапуск с правами администратора…",
  "err.singbox_missing": "Ядро sing-box не найдено",
  "err.singbox_start_failed": "Не удалось запустить sing-box",
  "err.server_not_found": "Сервер не найден",
  "err.cant_measure_connected": "Нельзя измерить, пока туннель держит другой сервер — отключитесь, чтобы проверить",
  "err.update_check_failed": "Не удалось проверить обновления — нет интернета или GitHub недоступен",
  "err.amneziawg_unsupported": "AmneziaWG не поддерживается ядром sing-box (обфускация junk-пакетами)",
  "err.amneziawg_start_failed": "Не удалось запустить AmneziaWG",
  "err.amneziawg_no_config": "У этого сервера AmneziaWG нет конфига",
  "err.service_required": "Для AmneziaWG нужна служба-помощник Rocket — переустановите Rocket",
  "err.not_a_zapret_dir": "Это не папка zapret",
};

const dicts = { en, ru };
let lang = "en";

export const LANGS = [
  ["en", "English"],
  ["ru", "Русский"],
];

export function setLang(l) {
  lang = dicts[l] ? l : "en";
  try { localStorage.setItem("lang", lang); } catch {}
  document.documentElement.lang = lang;
}

export function getLang() {
  return lang;
}

export function initLang(preferred) {
  let l = preferred;
  if (!l) { try { l = localStorage.getItem("lang") || ""; } catch {} }
  if (!l) l = (navigator.language || "").toLowerCase().startsWith("ru") ? "ru" : "en";
  setLang(l);
}

function plural(word, n) {
  // only "server" is pluralised, both languages
  if (word !== "server") return word;
  return lang === "ru" ? "серверов" : n === 1 ? "server" : "servers";
}

export function t(key, vars) {
  let s = dicts[lang][key] ?? en[key] ?? key;
  if (vars) {
    // {n, server} -> pluralised word
    s = s.replace(/\{(\w+),\s*(\w+)\}/g, (_, v, w) => plural(w, vars[v]));
    s = s.replace(/\{(\w+)\}/g, (_, k) => (vars[k] !== undefined ? vars[k] : `{${k}}`));
  }
  return s;
}

// Backend errors arrive as "code" or "code: detail".
export function tErr(msg) {
  msg = String(msg);
  const i = msg.indexOf(": ");
  const code = i === -1 ? msg : msg.slice(0, i);
  const detail = i === -1 ? "" : msg.slice(i + 2);
  const key = "err." + code;
  if (en[key]) return t(key) + (detail ? ": " + detail : "");
  return msg;
}
