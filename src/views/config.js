import { api } from "../api.js";
import { h, cell, group, sectionHeader, iosSwitch, toast } from "../ui.js";
import { t, tErr } from "../i18n.js";

// Config tab — routing switches.
export async function renderConfig(root, fresh = () => true) {
  const [state, settings] = await Promise.all([
    api.getState(),
    api.getSettings(),
  ]);
  let zdir = settings.zapret_dir || (await api.detectZapret().catch(() => ""));
  if (!fresh()) return;
  root.innerHTML = "";

  const bypassRu = state.mode === "bypass_ru";

  const ruSw = iosSwitch(bypassRu, async (on) => {
    ruSw.setBusy(true);
    try {
      await api.setMode(on ? "bypass_ru" : "global");
      toast(t(on ? "routing.toast.bypass_ru" : "routing.toast.global"));
    } catch (e) {
      ruSw.setChecked(!on);
      toast(tErr(e));
    } finally {
      ruSw.setBusy(false);
      bump();
    }
  });

  const dlSw = iosSwitch(!!settings.download_bypass, async (on) => {
    dlSw.setBusy(true);
    try {
      await api.setSettings({ download_bypass: on });
      toast(t(on ? "dl.on" : "dl.off"));
    } catch (e) {
      dlSw.setChecked(!on);
      toast(tErr(e));
    } finally {
      dlSw.setBusy(false);
      bump();
    }
  });

  const zSw = iosSwitch(settings.zapret_mode && !!zdir, async (on) => {
    zSw.setBusy(true);
    try {
      if (on && !zdir) {
        try {
          zdir = await api.pickZapretDir();
        } catch (e) {
          zSw.setChecked(false);
          if (String(e) === "not_a_zapret_dir") toast(t("zapret.notDir"));
          return;
        }
      }
      await api.setSettings({ zapret_mode: on });
      toast(t(on ? "zapret.on" : "zapret.off"));
    } catch (e) {
      zSw.setChecked(!on);
      toast(tErr(e));
    } finally {
      zSw.setBusy(false);
      bump();
    }
  });

  const zCell = cell({
    title: t("zapret.title"),
    sub: zdir ? shorten(zdir) : t("zapret.notFound"),
    accessory: zSw,
    onClick: async () => {
      try {
        const d = await api.pickZapretDir();
        zdir = d;
        toast(t("zapret.folderSet"));
        bump();
      } catch (e) {
        if (String(e) !== "cancelled") toast(String(e) === "not_a_zapret_dir" ? t("zapret.notDir") : tErr(e));
      }
    },
  });

  root.append(sectionHeader(t("cfg.routing")));
  root.append(group(
    cell({
      title: t("cfg.bypassRu.title"),
      sub: t("cfg.bypassRu.sub"),
      accessory: ruSw,
      noTap: true,
    }),
    cell({
      title: t("dl.title"),
      sub: t("dl.sub"),
      accessory: dlSw,
      noTap: true,
    }),
    zCell,
  ));

  root.append(h("div", { class: "section-footer" },
    settings.zapret_mode && zdir ? t("zapret.footer.on") : t("cfg.footer.off"),
  ));
}

function shorten(p) {
  const parts = p.replace(/\\/g, "/").split("/");
  return parts.length > 3 ? "…/" + parts.slice(-2).join("/") : p;
}

function bump() {
  document.dispatchEvent(new CustomEvent("state"));
}
