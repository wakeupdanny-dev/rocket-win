import { api } from "../api.js";
import { h, group, cell, sectionHeader, fmtBytes } from "../ui.js";
import { t } from "../i18n.js";

// Live traffic + log console — mirrors Shadowrocket's "Data" tab.
export async function renderData(root, fresh = () => true) {
  const [state0, traffic0, servers0, logs0] = await Promise.all([
    api.getState(),
    api.getTraffic(),
    api.listServers(),
    api.getLogs(),
  ]);
  if (!fresh()) return;
  root.innerHTML = "";

  const wrap = h("div");
  root.append(wrap);

  let logLines = logs0 || [];

  function paint(state, traffic, servers) {
    const sel = servers.find((s) => s.id === state.selected);
    wrap.innerHTML = "";

    wrap.append(h("div", { class: "big-status" },
      h("div", { class: "state " + (state.connected ? "on" : "off") }, state.connected ? t("home.connected") : t("home.notConnected")),
      h("div", { class: "sub" }, sel ? sel.name : t("data.noServer")),
    ));

    wrap.append(h("div", { class: "stat-grid" },
      stat(fmtBytes(traffic.up) + "/s", t("data.upload")),
      stat(fmtBytes(traffic.down) + "/s", t("data.download")),
      stat(fmtBytes(traffic.up_total), t("data.sentTotal")),
      stat(fmtBytes(traffic.down_total), t("data.recvTotal")),
    ));

    wrap.append(sectionHeader(t("data.connection")));
    const modeLabel = t("mode." + (state.mode === "bypass_ru" ? "bypass_ru" : "global"));
    wrap.append(group(
      cell({ title: t("data.mode"), accessory: "TUN · " + modeLabel, noTap: true }),
      cell({ title: t("data.localListen"), accessory: "127.0.0.1:" + state.listen_port, noTap: true }),
    ));

    wrap.append(sectionHeader(t("data.coreLog")));
    const logEl = h("div", { class: "log" }, logLines.slice(-400).join("\n") || t("data.noOutput"));
    wrap.append(logEl);
    logEl.scrollTop = logEl.scrollHeight;
  }

  paint(state0, traffic0, servers0);

  // live traffic tick
  const refresh = setInterval(async () => {
    const t = await api.getTraffic();
    const grid = wrap.querySelector(".stat-grid");
    if (!grid) return;
    grid.children[0].querySelector(".v").textContent = fmtBytes(t.up) + "/s";
    grid.children[1].querySelector(".v").textContent = fmtBytes(t.down) + "/s";
    grid.children[2].querySelector(".v").textContent = fmtBytes(t.up_total);
    grid.children[3].querySelector(".v").textContent = fmtBytes(t.down_total);
  }, 1000);

  const unlistenLog = await api.on("log-line", (line) => {
    logLines.push(line);
    const logEl = wrap.querySelector(".log");
    if (logEl) {
      logEl.textContent = logLines.slice(-400).join("\n");
      logEl.scrollTop = logEl.scrollHeight;
    }
  });

  // cleanup when this view is removed from the DOM
  const obs = new MutationObserver(() => {
    if (!document.body.contains(wrap)) {
      clearInterval(refresh);
      unlistenLog?.();
      obs.disconnect();
    }
  });
  obs.observe(document.getElementById("view"), { childList: true });
}

function stat(v, k) {
  return h("div", { class: "stat" }, h("div", { class: "v" }, v), h("div", { class: "k" }, k));
}
