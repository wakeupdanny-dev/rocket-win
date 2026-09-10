import { api } from "../api.js";
import { h, cell, group, sectionHeader, iosSwitch, toast, sheet } from "../ui.js";
import { t, tErr, setLang, LANGS } from "../i18n.js";

export async function renderSettings(root, fresh = () => true) {
  const [s, version] = await Promise.all([
    api.getSettings(),
    api.appVersion().catch(() => "—"),
  ]);
  if (!fresh()) return;
  root.innerHTML = "";

  const toggle = (key, checked) => {
    const sw = iosSwitch(checked, async (v) => {
      sw.setBusy(true);
      try { await api.setSettings({ [key]: v }); toast(t("set.saved")); }
      catch (e) { sw.setChecked(!v); toast(tErr(e)); }
      finally { sw.setBusy(false); }
    });
    return sw;
  };

  const check = () => h("span", { class: "latency good", html: `<svg viewBox="0 0 16 13" width="15" height="12" fill="none"><path d="M1 7l5 5L15 1" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg>` });

  root.append(sectionHeader(t("set.general")));
  const langLabel = s.lang
    ? (LANGS.find(([c]) => c === s.lang) || LANGS[0])[1]
    : t("set.langAuto");
  root.append(group(
    cell({ title: t("set.language"), accessory: langLabel, chevron: true, onClick: () => langSheet(check, s.lang || "") }),
    cell({ title: t("set.startLogin"), accessory: toggle("autostart", s.autostart), noTap: true }),
    cell({ title: t("set.fallback"), sub: t("set.fallbackSub"), accessory: toggle("fallback", s.fallback), noTap: true }),
  ));

  const subs = s.subscriptions || [];
  if (subs.length) {
    root.append(sectionHeader(t("set.subscriptions")));
    const hrs = s.sub_update_hours ?? 12;
    const autoLabel = hrs === 0 ? t("set.off") : t("set.hoursShort", { n: hrs });
    root.append(group(
      cell({ title: t("set.autoUpdate"), accessory: autoLabel, chevron: true, onClick: () => autoUpdateSheet(hrs, check) }),
      ...subs.map((sub) =>
        cell({ title: sub.name, sub: sub.url, chevron: true, onClick: () => subSheet(subs, sub) }),
      ),
      cell({
        title: t("set.updateAllNow"), chevron: true,
        onClick: async () => {
          toast(t("act.updating"));
          try { const n = await api.updateSubscriptions(); toast(t("act.servers", { n })); document.dispatchEvent(new CustomEvent("state")); }
          catch (e) { toast(tErr(e)); }
        },
      }),
    ));
  }

  root.append(sectionHeader(t("set.advanced")));
  root.append(group(
    cell({ title: t("set.proxyPort"), accessory: String(s.listen_port || 1089), chevron: true, onClick: () => portSheet(s.listen_port || 1089) }),
    cell({ title: t("set.dns"), accessory: s.dns || "https://1.1.1.1/dns-query", chevron: true, onClick: () => dnsSheet(s.dns || "https://1.1.1.1/dns-query") }),
    cell({ title: t("set.openFolder"), chevron: true, onClick: () => api.setSettings({ __open_config: true }).then(() => {}).catch(() => toast(t("set.na"))) }),
  ));

  const updateState = h("span", { class: "latency" }, "");
  root.append(sectionHeader(t("set.about")));
  root.append(group(
    cell({ title: t("set.core"), accessory: "sing-box", noTap: true }),
    cell({ title: t("set.version"), accessory: version, noTap: true }),
    cell({
      title: t("set.checkUpdate"),
      accessory: updateState,
      chevron: true,
      onClick: async () => {
        updateState.className = "latency";
        updateState.textContent = t("set.checking");
        try {
          const info = await api.checkUpdate();
          if (info.available) {
            updateState.className = "latency good";
            updateState.textContent = "v" + info.latest;
            updateSheet(info);
          } else {
            updateState.className = "latency good";
            updateState.textContent = "✓ " + t("set.upToDate");
          }
        } catch (e) {
          updateState.className = "latency bad";
          updateState.textContent = t("srv.timeout");
          toast(tErr(e));
        }
      },
    }),
  ));
}

function updateSheet(info) {
  sheet(t("set.updateAvailable", { v: info.latest }), (body, close) => {
    body.append(h("div", { class: "section-footer", style: "text-align:left;padding:0 16px" },
      t("set.updateBody", { current: info.current, latest: info.latest })));
    const download = h("button", { class: "sheet-btn" }, t("set.download"));
    download.addEventListener("click", async () => {
      try { await api.openUrl(info.download_url || info.url); } catch {}
      close();
    });
    const later = h("button", { class: "sheet-btn secondary" }, t("set.later"));
    later.addEventListener("click", close);
    body.append(download, later);
  });
}

function langSheet(check, current) {
  sheet(t("set.language"), (body, close) => {
    const opts = [["", t("set.langAuto")], ...LANGS];
    body.append(group(...opts.map(([code, label]) =>
      cell({
        title: label,
        accessory: code === current ? check() : null,
        onClick: async () => {
          try { await api.setSettings({ lang: code }); } catch {}
          if (code) {
            setLang(code);
          } else {
            // follow the OS again
            try { setLang(await api.osLanguage()); } catch { setLang("en"); }
          }
          close();
          document.dispatchEvent(new CustomEvent("state"));
        },
      }),
    )));
  });
}

function autoUpdateSheet(current, check) {
  sheet(t("set.autoUpdateTitle"), (body, close) => {
    const opts = [[0, t("set.off")], [6, t("set.everyN", { n: 6 })], [12, t("set.everyN", { n: 12 })], [24, t("set.everyN", { n: 24 })]];
    body.append(group(...opts.map(([v, label]) =>
      cell({
        title: label,
        accessory: v === current ? check() : null,
        onClick: async () => {
          await api.setSettings({ sub_update_hours: v });
          close();
          document.dispatchEvent(new CustomEvent("state"));
        },
      }),
    )));
  });
}

function subSheet(subs, existing) {
  sheet(existing.name, (body, close) => {
    body.append(h("div", { class: "section-footer", style: "text-align:left;padding:8px 16px" }, existing.url));

    const update = h("button", { class: "sheet-btn" }, t("sub.updateNow"));
    update.addEventListener("click", async () => {
      close();
      toast(t("act.updating"));
      try { const n = await api.updateSubscriptions(); toast(t("act.servers", { n })); document.dispatchEvent(new CustomEvent("state")); }
      catch (e) { toast(tErr(e)); }
    });

    const del = h("button", { class: "sheet-btn secondary" }, t("sub.remove"));
    del.addEventListener("click", async () => {
      await api.setSettings({ subscriptions: subs.filter((x) => x.url !== existing.url) });
      close();
      document.dispatchEvent(new CustomEvent("state"));
    });

    body.append(update, del);
  });
}

function portSheet(current) {
  sheet(t("set.proxyPort"), (body, close) => {
    const inp = h("input", { class: "sheet-field", type: "number", value: String(current) });
    const save = h("button", { class: "sheet-btn" }, t("common.save"));
    save.addEventListener("click", async () => {
      const p = parseInt(inp.value, 10);
      if (!(p > 1024 && p < 65536)) return toast(t("set.portRange"));
      await api.setSettings({ listen_port: p });
      close();
      document.dispatchEvent(new CustomEvent("state"));
    });
    body.append(inp, save);
  });
}

function dnsSheet(current) {
  sheet(t("set.dns"), (body, close) => {
    const inp = h("input", { class: "sheet-field", value: current });
    const save = h("button", { class: "sheet-btn" }, t("common.save"));
    save.addEventListener("click", async () => {
      await api.setSettings({ dns: inp.value.trim() });
      close();
      document.dispatchEvent(new CustomEvent("state"));
    });
    body.append(inp, save);
  });
}
