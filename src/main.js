import "./style.css";
import { api } from "./api.js";
import { toast } from "./ui.js";
import { t, initLang } from "./i18n.js";
import { renderHome, homeNavRight } from "./views/home.js";
import { renderConfig } from "./views/config.js";
import { renderData } from "./views/data.js";
import { renderSettings } from "./views/settings.js";

const viewEl = document.getElementById("view");
const titleEl = document.getElementById("nav-title");
const navRight = document.getElementById("nav-right");
const navLeft = document.getElementById("nav-left");
const tabLabels = document.querySelectorAll(".tab span");

const tabs = {
  home: { titleKey: "app.title", render: renderHome, onRight: homeNavRight },
  config: { titleKey: "nav.config", render: renderConfig, onRight: null },
  data: { titleKey: "nav.data", render: renderData, onRight: null },
  settings: { titleKey: "nav.settings", render: renderSettings, onRight: null },
};

let gen = 0;

function paintChrome() {
  titleEl.textContent = t(tabs[app.current].titleKey);
  tabLabels.forEach((el) => {
    const tab = el.closest(".tab").dataset.tab;
    el.textContent = t("nav." + tab);
  });
}

export const app = {
  current: "home",
  state: null,

  async refreshState() {
    this.state = await api.getState();
    document.dispatchEvent(new CustomEvent("state", { detail: this.state }));
  },

  // (re)render the active tab. Every render is tagged with a generation so a
  // slow async render that has been superseded bails out instead of appending
  // a stale copy of the view.
  render() {
    const my = ++gen;
    paintChrome();
    viewEl.innerHTML = "";
    tabs[this.current].render(viewEl, () => my === gen);
  },

  go(tab) {
    if (!tabs[tab]) return;
    this.current = tab;
    document.querySelectorAll(".tab").forEach((el) => el.classList.toggle("active", el.dataset.tab === tab));
    navRight.style.visibility = tabs[tab].onRight ? "visible" : "hidden";
    this.render();
  },
};

document.querySelectorAll(".tab").forEach((el) =>
  el.addEventListener("click", () => app.go(el.dataset.tab)),
);
navRight.addEventListener("click", () => tabs[app.current].onRight?.());
navLeft.addEventListener("click", () => toast(t("misc.qrNA")));

// one central re-render on any state change (from the core or from a sheet)
document.addEventListener("state", () => app.render());

api.on("state-changed", (s) => {
  app.state = s;
  document.dispatchEvent(new CustomEvent("state", { detail: s }));
});
api.on("toast", (m) => toast(m));

// language: an explicit choice in Settings wins; otherwise follow the OS
// (Russian Windows → ru, anything else → en) — and don't persist the guess,
// so a later OS language change is still picked up.
const settings = await api.getSettings().catch(() => ({}));
let lang = settings.lang || "";
if (!lang) lang = await api.osLanguage().catch(() => "");
initLang(lang);

await app.refreshState();
app.go("home");
