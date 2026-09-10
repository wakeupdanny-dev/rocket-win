// Generates the NSIS installer images (no external deps):
//   src-tauri/installer/sidebar.bmp  164x314  — blue gradient + white rocket
//   src-tauri/installer/header.bmp   150x57   — white strip + small rocket
// 24-bit uncompressed BGR, bottom-up (the format NSIS/MUI2 is happiest with).
import { writeFileSync, mkdirSync } from "node:fs";

mkdirSync("src-tauri/installer", { recursive: true });

function canvas(w, h) {
  const px = new Uint8Array(w * h * 3); // RGB, top-down while drawing
  const set = (x, y, r, g, b, a = 1) => {
    x |= 0; y |= 0;
    if (x < 0 || y < 0 || x >= w || y >= h) return;
    const i = (y * w + x) * 3;
    px[i] = px[i] * (1 - a) + r * a;
    px[i + 1] = px[i + 1] * (1 - a) + g * a;
    px[i + 2] = px[i + 2] * (1 - a) + b * a;
  };
  return { w, h, px, set };
}

// even-odd polygon fill with vertical antialiasing on the edges
function polygon(c, pts, r, g, b) {
  const ys = pts.map((p) => p[1]);
  const y0 = Math.floor(Math.min(...ys));
  const y1 = Math.ceil(Math.max(...ys));
  for (let y = y0; y <= y1; y++) {
    const xs = [];
    for (let i = 0; i < pts.length; i++) {
      const [ax, ay] = pts[i];
      const [bx, by] = pts[(i + 1) % pts.length];
      if (ay === by) continue;
      const yy = y + 0.5;
      if (yy >= Math.min(ay, by) && yy < Math.max(ay, by)) {
        xs.push(ax + ((yy - ay) / (by - ay)) * (bx - ax));
      }
    }
    xs.sort((a, b) => a - b);
    for (let k = 0; k + 1 < xs.length; k += 2) {
      const xa = xs[k], xb = xs[k + 1];
      for (let x = Math.floor(xa); x < Math.ceil(xb); x++) {
        const cov = Math.min(x + 1, xb) - Math.max(x, xa);
        c.set(x, y, r, g, b, Math.max(0, Math.min(1, cov)));
      }
    }
  }
}

// the paper-plane rocket from the app icon, centred at (cx, top..bottom)
function rocket(c, cx, top, height, r = 255, g = 255, b = 255) {
  const s = height / 628; // glyph is ~628 units tall in the icon
  const Y = (v) => top + (v - 232) * s;
  const X = (v) => cx + (v - 512) * s;
  polygon(c, [[X(512), Y(232)], [X(762), Y(720)], [X(512), Y(610)], [X(262), Y(720)]], r, g, b);
  polygon(c, [[X(512), Y(232)], [X(512), Y(610)], [X(262), Y(720)]],
    Math.round(r * 0.87 + 222 * 0.13), Math.round(g * 0.9 + 236 * 0.1), b);
  polygon(c, [[X(442), Y(690)], [X(582), Y(690)], [X(552), Y(860)], [X(512), Y(800)], [X(472), Y(860)]], r, g, b);
}

function bmp(c) {
  const { w, h, px } = c;
  const rowSize = Math.ceil((w * 3) / 4) * 4;
  const pad = rowSize - w * 3;
  const size = 54 + rowSize * h;
  const b = Buffer.alloc(size);
  b.write("BM", 0);
  b.writeUInt32LE(size, 2);
  b.writeUInt32LE(54, 10);
  b.writeUInt32LE(40, 14);
  b.writeInt32LE(w, 18);
  b.writeInt32LE(h, 22); // positive = bottom-up
  b.writeUInt16LE(1, 26);
  b.writeUInt16LE(24, 28);
  b.writeUInt32LE(rowSize * h, 34);
  let o = 54;
  for (let y = h - 1; y >= 0; y--) {
    for (let x = 0; x < w; x++) {
      const i = (y * w + x) * 3;
      b[o++] = px[i + 2]; // B
      b[o++] = px[i + 1]; // G
      b[o++] = px[i];     // R
    }
    o += pad;
  }
  return b;
}

// 5x7 pixel font, just the glyphs we need for "ROCKET"
const FONT = {
  R: ["11110", "10001", "10001", "11110", "10100", "10010", "10001"],
  O: ["01110", "10001", "10001", "10001", "10001", "10001", "01110"],
  C: ["01110", "10001", "10000", "10000", "10000", "10001", "01110"],
  K: ["10001", "10010", "10100", "11000", "10100", "10010", "10001"],
  E: ["11111", "10000", "10000", "11110", "10000", "10000", "11111"],
  T: ["11111", "00100", "00100", "00100", "00100", "00100", "00100"],
};
function text(c, str, x, y, scale, r, g, b) {
  let cx = x;
  for (const ch of str) {
    const gl = FONT[ch];
    if (gl) {
      for (let row = 0; row < 7; row++)
        for (let col = 0; col < 5; col++)
          if (gl[row][col] === "1")
            for (let sy = 0; sy < scale; sy++)
              for (let sx = 0; sx < scale; sx++)
                c.set(cx + col * scale + sx, y + row * scale + sy, r, g, b);
    }
    cx += 6 * scale;
  }
}

// ---- sidebar: 164x314 ----
{
  const c = canvas(164, 314);
  for (let y = 0; y < c.h; y++) {
    const t = y / c.h;
    const r = Math.round(40 + t * 2);
    const g = Math.round(134 - t * 32);
    const bl = Math.round(248 - t * 26);
    for (let x = 0; x < c.w; x++) c.set(x, y, r, g, bl);
  }
  // soft radial glow behind the glyph
  for (let y = 20; y < 210; y++)
    for (let x = 0; x < c.w; x++) {
      const d = Math.hypot(x - 82, y - 108) / 130;
      if (d < 1) c.set(x, y, 255, 255, 255, (1 - d) * 0.07);
    }
  rocket(c, 82, 44, 150);
  // wordmark "ROCKET", centred (6 chars * 6 * scale - trailing gap)
  const scale = 3;
  const wmW = "ROCKET".length * 6 * scale - scale;
  text(c, "ROCKET", Math.round((c.w - wmW) / 2), 232, scale, 255, 255, 255);
  writeFileSync("src-tauri/installer/sidebar.bmp", bmp(c));
  console.log("sidebar.bmp", c.w + "x" + c.h);
}

// ---- header: 150x57, white so the black page title stays readable ----
{
  const c = canvas(150, 57);
  for (let i = 0; i < c.px.length; i++) c.px[i] = 255;
  // blue accent line along the bottom
  for (let x = 0; x < c.w; x++) { c.set(x, 55, 21, 119, 224); c.set(x, 56, 21, 119, 224); }
  // small rocket, bottom-right corner (page title text sits on the left)
  rocket(c, 126, 8, 40, 21, 119, 224);
  writeFileSync("src-tauri/installer/header.bmp", bmp(c));
  console.log("header.bmp", c.w + "x" + c.h);
}
