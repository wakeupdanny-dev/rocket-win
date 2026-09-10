import { api } from "../api.js";
import { h, cell, group, sectionHeader, iosSwitch, toast, sheet, latencyClass, fmtBytes } from "../ui.js";
import { t, tErr } from "../i18n.js";

const ICON = {
  rocket: `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M13.5 2C10 3 7 6 6 10l-2 1a2 2 0 0 0-.8 3l1.5 1.5L6 17l1.5 1.5a2 2 0 0 0 3-.8l1-2c4-1 7-4 8-7.5.4-1.5.5-3 .3-4.4A17 17 0 0 0 13.5 2M15 8a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3M5 19c-1 1-1.5 3-1.5 3s2-.5 3-1.5c.6-.6.6-1.5 0-2s-1.4-.6-1.5.5"/></svg>`,
  route: `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M6 3a3 3 0 0 0-1 5.8V15a3 3 0 0 0 3 3h5.2a3 3 0 1 0 0-2H8a1 1 0 0 1-1-1V8.8A3 3 0 0 0 6 3m0 2a1 1 0 1 1 0 2 1 1 0 0 1 0-2m12 12a1 1 0 1 1 0 2 1 1 0 0 1 0-2"/></svg>`,
  gauge: `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 4a9 9 0 0 0-7.7 13.7 1 1 0 0 0 .9.5h13.6a1 1 0 0 0 .9-.5A9 9 0 0 0 12 4m0 3a1 1 0 0 1 1 1v1a1 1 0 0 1-2 0V8a1 1 0 0 1 1-1m5.7 3.3-2.1 2.1a2 2 0 1 1-1.4-1.4l2.1-2.1a1 1 0 0 1 1.4 1.4"/></svg>`,
};

export function homeNavRight() {
  addServerSheet();
}

export async function renderHome(root, fresh = () => true) {
  const [state, servers, settings] = await Promise.all([
    api.getState(),
    api.listServers(),
    api.getSettings(),
  ]);
  if (!fresh()) return;
  root.innerHTML = "";

  const selected = servers.find((s) => s.id === state.selected);
  const sw = iosSwitch(state.connected, async (on) => {
    sw.setBusy(true);
    try {
      if (on) {
        if (!servers.length) { sw.setChecked(false); toast(t("srv.needServer")); return; }
        await api.connect();
      } else {
        await api.disconnect();
      }
    } catch (e) {
      sw.setChecked(!on);
      toast(tErr(e));
    } finally {
      sw.setBusy(false);
      bump();
    }
  });

  const statusCell = cell({
    icon: ICON.rocket,
    iconColor: state.connected ? "#34c759" : "#8e8e93",
    title: state.connected ? t("home.connected") : t("home.notConnected"),
    sub: state.connected && selected
      ? t("home.via", { name: selected.name, port: state.listen_port })
      : null,
    accessory: sw,
    noTap: true,
  });

  const routingCell = cell({
    icon: ICON.route,
    iconColor: "#5856d6",
    title: t("home.routing"),
    accessory: t("mode." + (state.mode === "bypass_ru" ? "bypass_ru" : "global")),
    chevron: true,
    onClick: () => routingSheet(state.mode),
  });

  // filled in by serverCell below; lets the ping refresh update badges in
  // place instead of tearing down and rebuilding the whole tab (was visibly
  // flickering on every "Connectivity Test" tap)
  const badgeRefs = new Map();

  const testCell = cell({
    icon: ICON.gauge,
    iconColor: "#34aadc",
    title: t("home.connTest"),
    onClick: async () => {
      if (!servers.length) return toast(t("srv.needServer"));
      await Promise.all(servers.map(async (s) => {
        try {
          const ms = await api.testLatency(s.id);
          const badge = badgeRefs.get(s.id);
          if (badge) setLatencyBadge(badge, ms);
        } catch {}
      }));
    },
  });

  root.append(group(statusCell, routingCell, testCell));
  root.append(sectionHeader(t("home.servers"), () => serverActionsSheet()));

  const serverCell = (s) => {
    const badge = latencyBadge(s.latency);
    badgeRefs.set(s.id, badge);
    const c = cell({
      title: s.name,
      sub: `${s.type.toUpperCase()} · ${s.address}:${s.port}`,
      dot: s.id === state.selected,
      accessory: badge,
      chevron: true,
      onClick: async () => {
        if (s.id === state.selected) return;
        const wasConnected = state.connected;
        if (wasConnected) toast(t("srv.switching", { name: s.name }));
        try {
          await api.selectServer(s.id);
          if (wasConnected) toast(t("srv.switched", { name: s.name }));
        } catch (e) { toast(tErr(e)); }
        bump();
      },
    });
    c.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      serverContextSheet(s);
    });
    return c;
  };

  const subs = settings.subscriptions || [];
  const subNames = new Set(subs.map((x) => x.name));
  const local = servers.filter((s) => !s.from_sub || !subNames.has(s.from_sub));

  root.append(foldGroup("local", t("home.localServers"), null,
    local.length ? local.map(serverCell) : [cell({ title: t("home.noLocal"), noTap: true })],
  ));

  for (const sub of subs) {
    const list = servers.filter((s) => s.from_sub === sub.name);
    root.append(foldGroup(
      "sub:" + sub.url,
      sub.name,
      subInfoLine(sub),
      list.length ? list.map(serverCell) : [cell({ title: t("home.noServers"), noTap: true })],
      {
        onRefresh: async () => {
          toast(t("act.updating"));
          try { await api.updateSubscriptions(); toast(t("act.updated")); }
          catch (e) { toast(tErr(e)); }
          bump();
        },
        onInfo: () => subMenu(sub, subs),
      },
    ));
  }

  root.append(h("div", { class: "section-footer" }, t("home.footer")));
}

// A collapsible card: header row + rows, fold state kept in localStorage.
function foldGroup(id, title, subtitle, rows, opts = {}) {
  let folded = false;
  try { folded = localStorage.getItem("fold:" + id) === "1"; } catch {}

  const chev = h("span", { class: "fold-chev" + (folded ? "" : " open"), html:
    `<svg viewBox="0 0 12 8" fill="none"><path d="M1 1l5 5 5-5" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>` });

  const acc = h("div", { class: "cell-accessory" });
  if (opts.onRefresh) {
    const r = h("button", { class: "hdr-btn", html:
      `<svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M4 10a8 8 0 0 1 14-4l2 2M20 14a8 8 0 0 1-14 4l-2-2M18 4v4h-4M6 20v-4h4"/></svg>` });
    r.addEventListener("click", (e) => { e.stopPropagation(); opts.onRefresh(); });
    acc.append(r);
  }
  if (opts.onInfo) {
    const i = h("button", { class: "hdr-btn", html:
      `<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 7.5v.5" stroke-linecap="round"/></svg>` });
    i.addEventListener("click", (e) => { e.stopPropagation(); opts.onInfo(); });
    acc.append(i);
  }

  const header = h("div", { class: "cell fold-header no-icon" },
    chev,
    h("div", { class: "cell-body" },
      h("div", { class: "cell-title" }, title),
      subtitle ? h("div", { class: "cell-sub" }, subtitle) : null,
    ),
    acc,
  );

  const bodyRows = h("div", { class: "fold-body" }, ...rows);
  bodyRows.hidden = folded;

  header.addEventListener("click", () => {
    folded = !folded;
    bodyRows.hidden = folded;
    chev.classList.toggle("open", !folded);
    try { localStorage.setItem("fold:" + id, folded ? "1" : "0"); } catch {}
  });

  return h("div", { class: "group" }, header, bodyRows);
}

function subInfoLine(sub) {
  const used = (sub.upload || 0) + (sub.download || 0);
  const parts = [];
  if (sub.total) parts.push(`${fmtBytes(used)} / ${fmtBytes(sub.total)}`);
  else if (used) parts.push(`↓ ${fmtBytes(sub.download || 0)}`);
  if (sub.expire) parts.push(t("sub.until", { date: new Date(sub.expire * 1000).toISOString().slice(0, 10) }));
  if (sub.count) parts.push(t("sub.count", { n: sub.count }));
  return parts.join("  ·  ") || sub.url;
}

function subMenu(sub, subs) {
  sheet(sub.name, (body, close) => {
    body.append(h("div", { class: "section-footer", style: "text-align:left;padding:8px 16px" }, sub.url));
    body.append(group(
      cell({ title: t("sub.updateNow"), onClick: async () => { close(); toast(t("act.updating")); try { await api.updateSubscriptions(); toast(t("act.updated")); } catch (e) { toast(tErr(e)); } bump(); } }),
      cell({ title: t("sub.copyUrl"), onClick: async () => { try { await navigator.clipboard.writeText(sub.url); toast(t("srv.copied")); } catch { toast(t("srv.clipboardBlocked")); } close(); } }),
      cell({ title: t("sub.remove"), danger: true, onClick: async () => { await api.setSettings({ subscriptions: subs.filter((x) => x.url !== sub.url) }); close(); bump(); } }),
    ));
  });
}

function bump() {
  document.dispatchEvent(new CustomEvent("state"));
}

function latencyParts(ms) {
  if (ms == null || ms < 0) {
    // null = untested, -1 = no answer, -2/-3 = legacy sentinels
    return ms === -1
      ? { cls: "latency bad", text: t("srv.timeout").toLowerCase() }
      : { cls: "latency", text: "—" };
  }
  return { cls: "latency " + latencyClass(ms), text: ms + " ms" };
}

function latencyBadge(ms) {
  const { cls, text } = latencyParts(ms);
  return h("span", { class: cls }, text);
}

// update an existing badge span in place (no re-render → no flicker)
function setLatencyBadge(el, ms) {
  const { cls, text } = latencyParts(ms);
  el.className = cls;
  el.textContent = text;
}

// ---------------- sheets ----------------

function addServerSheet() {
  sheet(t("add.title"), (body, close) => {
    const ta = h("textarea", { class: "sheet-field", placeholder: t("add.placeholder") });

    const done = (n) => {
      toast(t("add.imported", { n }));
      close();
      document.dispatchEvent(new CustomEvent("state"));
    };

    const addBtn = h("button", { class: "sheet-btn" }, t("add.btnAdd"));
    addBtn.addEventListener("click", async () => {
      const text = ta.value.trim();
      if (!text) return toast(t("add.nothing"));
      const isUrl = /^https?:\/\/\S+$/i.test(text) && !text.includes("\n");
      try {
        const n = isUrl ? await api.addSubscription("", text) : await api.addServersFromText(text);
        done(n);
      } catch (e) { toast(tErr(e)); }
    });

    const fileBtn = h("button", { class: "sheet-btn secondary" }, t("add.btnFile"));
    fileBtn.addEventListener("click", async () => {
      try { done(await api.importFromFile()); }
      catch (e) {
        const msg = String(e);
        if (msg !== "cancelled") toast(tErr(msg));
      }
    });

    body.append(ta, addBtn, fileBtn,
      h("div", { class: "section-footer", style: "text-align:left;padding:6px 16px 0" }, t("add.hint")));
  });
}

function serverContextSheet(s) {
  sheet(s.name, (body, close) => {
    body.append(group(
      cell({
        title: t("srv.setDefault"),
        onClick: async () => { await api.selectServer(s.id); close(); document.dispatchEvent(new CustomEvent("state")); },
      }),
      cell({
        title: t("srv.test"),
        onClick: async () => {
          close();
          try {
            const ms = await api.testLatency(s.id);
            toast(ms < 0 ? t("srv.timeout") : `${s.name}: ${ms} ms`);
          } catch (e) { toast(tErr(e)); }
          document.dispatchEvent(new CustomEvent("state"));
        },
      }),
      cell({
        title: t("srv.share"), chevron: true,
        onClick: () => { close(); shareSheet(s); },
      }),
      cell({
        title: t("srv.delete"), danger: true,
        onClick: async () => { await api.deleteServer(s.id); close(); toast(t("srv.deleted")); document.dispatchEvent(new CustomEvent("state")); },
      }),
    ));
  });
}

async function shareSheet(s) {
  let qrError = false;
  const [link, qr] = await Promise.all([
    api.exportLink(s.id).catch(() => ""),
    api.exportQr(s.id).catch(() => { qrError = true; return ""; }),
  ]);
  sheet(t("srv.share"), (body, close) => {
    if (qr) {
      body.append(h("div", {
        class: "qr-box",
        html: qr,
      }));
    } else if (qrError) {
      body.append(h("div", { class: "section-footer" }, t("srv.qrTooLong")));
    }
    body.append(h("div", { class: "section-footer", style: "word-break:break-all;text-align:left;padding:0 16px;-webkit-user-select:text;user-select:text" }, link));

    const copyLink = h("button", { class: "sheet-btn" }, t("srv.copyLink"));
    copyLink.addEventListener("click", async () => {
      try { await navigator.clipboard.writeText(link); toast(t("srv.copied")); } catch { toast(t("srv.clipboardBlocked")); }
    });

    const copyJson = h("button", { class: "sheet-btn secondary" }, t("srv.copyJson"));
    copyJson.addEventListener("click", async () => {
      try {
        const j = await api.exportJson(s.id);
        await navigator.clipboard.writeText(j);
        toast(t("srv.copied"));
      } catch (e) { toast(tErr(e)); }
    });

    const saveBtn = h("button", { class: "sheet-btn secondary" }, t("srv.saveConfig"));
    saveBtn.addEventListener("click", async () => {
      try { await api.saveConfig(s.id, "json"); toast(t("srv.saved")); close(); }
      catch (e) { if (String(e) !== "cancelled") toast(tErr(e)); }
    });

    body.append(copyLink, copyJson, saveBtn);
  });
}

function routingSheet(current) {
  sheet(t("routing.title"), (body, close) => {
    const opts = [
      ["bypass_ru", t("routing.bypass_ru"), t("routing.bypass_ru.desc")],
      ["global", t("routing.global"), t("routing.global.desc")],
    ];
    body.append(group(...opts.map(([id, label, desc]) =>
      cell({
        title: label,
        sub: desc,
        accessory: id === current ? checkmark() : null,
        onClick: async () => {
          await api.setMode(id);
          close();
          document.dispatchEvent(new CustomEvent("state"));
        },
      }),
    )));
  });
}

function serverActionsSheet() {
  sheet(t("home.servers"), (body, close) => {
    body.append(group(
      cell({
        title: t("act.updateAll"), chevron: true, onClick: async () => {
          close();
          toast(t("act.updating"));
          try { const n = await api.updateSubscriptions(); toast(t("act.servers", { n })); document.dispatchEvent(new CustomEvent("state")); }
          catch (e) { toast(tErr(e)); }
        },
      }),
      cell({
        title: t("act.testAll"), chevron: true, onClick: async () => {
          close();
          const servers = await api.listServers();
          toast(t("act.testingN", { n: servers.length }));
          await Promise.all(servers.map((s) => api.testLatency(s.id).catch(() => {})));
          document.dispatchEvent(new CustomEvent("state"));
        },
      }),
      cell({
        title: t("act.sort"), chevron: true, onClick: () => { close(); toast(t("act.sorted")); document.dispatchEvent(new CustomEvent("state")); },
      }),
    ));
  });
}

function checkmark() {
  return h("span", { class: "latency good", html: `<svg viewBox="0 0 16 13" width="16" height="13" fill="none"><path d="M1 7l5 5L15 1" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg>` });
}
