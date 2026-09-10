// Tiny DOM helpers + iOS-style toast / bottom sheet.

export function h(tag, props = {}, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === "class") el.className = v;
    else if (k === "html") el.innerHTML = v;
    else if (k.startsWith("on") && typeof v === "function") el.addEventListener(k.slice(2), v);
    else if (v === true) el.setAttribute(k, "");
    else if (v !== false && v != null) el.setAttribute(k, v);
  }
  for (const c of children.flat()) {
    if (c == null || c === false) continue;
    el.append(c.nodeType ? c : document.createTextNode(String(c)));
  }
  return el;
}

const CHEVRON = `<span class="chevron"><svg viewBox="0 0 8 13" fill="none"><path d="M1 1l6 5.5L1 12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg></span>`;

export function cell({ icon, iconColor, title, sub, accessory, chevron, dot, onClick, noTap, danger }) {
  const parts = [];
  if (icon) parts.push(h("div", { class: "cell-icon", style: `background:${iconColor || "#8e8e93"}`, html: icon }));
  parts.push(h("div", { class: "cell-body" },
    h("div", { class: "cell-title" + (danger ? " danger" : "") }, title),
    sub ? h("div", { class: "cell-sub" }, sub) : null,
  ));
  const acc = h("div", { class: "cell-accessory" });
  if (accessory) acc.append(accessory.nodeType ? accessory : document.createTextNode(String(accessory)));
  if (chevron) acc.insertAdjacentHTML("beforeend", CHEVRON);
  parts.push(acc);
  const c = h("div", { class: "cell" + (icon ? "" : " no-icon") + (noTap ? " no-tap" : ""), onclick: onClick }, ...parts);
  if (dot) c.append(h("span", { class: "dot" }));
  return c;
}

export function group(...cells) {
  return h("div", { class: "group" }, ...cells.filter(Boolean));
}

export function sectionHeader(text, onMore) {
  const more = onMore ? h("span", { class: "more", onclick: onMore }, "•••") : null;
  return h("div", { class: "section-header" }, h("span", {}, text), more);
}

export function iosSwitch(checked, onChange) {
  const input = h("input", { type: "checkbox" });
  input.checked = !!checked;
  input.addEventListener("change", () => onChange(input.checked));
  const wrap = h("label", { class: "switch" }, input, h("span", { class: "track" }), h("span", { class: "knob" }));
  // don't let the toggle click bubble to a tappable parent cell
  wrap.addEventListener("click", (e) => e.stopPropagation());
  wrap.setBusy = (b) => wrap.classList.toggle("busy", b);
  wrap.setChecked = (v) => { input.checked = v; };
  wrap.setDisabled = (b) => { input.disabled = !!b; wrap.classList.toggle("disabled", !!b); };
  return wrap;
}

let toastEl;
export function toast(msg) {
  if (!toastEl) {
    toastEl = h("div", { class: "toast" });
    document.body.append(toastEl);
  }
  toastEl.textContent = msg;
  toastEl.classList.add("show");
  clearTimeout(toast._t);
  toast._t = setTimeout(() => toastEl.classList.remove("show"), 2200);
}

export function sheet(title, buildBody) {
  const body = h("div");
  const back = h("div", { class: "sheet-backdrop" },
    h("div", { class: "sheet" }, h("div", { class: "sheet-title" }, title), body),
  );
  const close = () => {
    back.classList.remove("show");
    setTimeout(() => back.remove(), 250);
  };
  back.addEventListener("click", (e) => { if (e.target === back) close(); });
  buildBody(body, close);
  document.body.append(back);
  requestAnimationFrame(() => back.classList.add("show"));
  return close;
}

export function latencyClass(ms) {
  if (ms < 0) return "bad";
  if (ms < 120) return "good";
  if (ms < 260) return "mid";
  return "bad";
}

export function fmtBytes(n) {
  if (n < 1024) return n + " B";
  const u = ["KB", "MB", "GB", "TB"];
  let i = -1;
  do { n /= 1024; i++; } while (n >= 1024 && i < u.length - 1);
  return n.toFixed(n < 10 ? 1 : 0) + " " + u[i];
}
