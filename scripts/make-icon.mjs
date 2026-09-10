// Generates a 1024x1024 app icon PNG (no external deps) — a Shadowrocket-style
// blue rounded square with a white paper-rocket glyph. Run: node scripts/make-icon.mjs
import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";

const S = 1024;
const buf = Buffer.alloc(S * S * 4);

function px(x, y, r, g, b, a = 255) {
  if (x < 0 || y < 0 || x >= S || y >= S) return;
  const i = (y * S + x) * 4;
  const ia = a / 255;
  buf[i] = buf[i] * (1 - ia) + r * ia;
  buf[i + 1] = buf[i + 1] * (1 - ia) + g * ia;
  buf[i + 2] = buf[i + 2] * (1 - ia) + b * ia;
  buf[i + 3] = Math.max(buf[i + 3], a);
}

function roundedRect(x0, y0, x1, y1, rad, fn) {
  for (let y = y0; y < y1; y++) {
    for (let x = x0; x < x1; x++) {
      let dx = 0, dy = 0;
      if (x < x0 + rad) dx = x0 + rad - x;
      else if (x > x1 - rad) dx = x - (x1 - rad);
      if (y < y0 + rad) dy = y0 + rad - y;
      else if (y > y1 - rad) dy = y - (y1 - rad);
      const d = Math.hypot(dx, dy);
      const aa = Math.max(0, Math.min(1, rad - d + 0.5));
      if (aa > 0) fn(x, y, aa);
    }
  }
}

// vertical gradient background
roundedRect(0, 0, S, S, 220, (x, y, aa) => {
  const t = y / S;
  const r = Math.round(46 + t * -6);
  const g = Math.round(140 + t * -30);
  const b = Math.round(246 + t * -20);
  px(x, y, r, g, b, Math.round(255 * aa));
});

// polygon fill (even-odd) for a paper-plane rocket
function polygon(points, r, g, b) {
  const ys = points.map((p) => p[1]);
  const minY = Math.floor(Math.min(...ys));
  const maxY = Math.ceil(Math.max(...ys));
  for (let y = minY; y <= maxY; y++) {
    const xs = [];
    for (let i = 0; i < points.length; i++) {
      const [x1, y1] = points[i];
      const [x2, y2] = points[(i + 1) % points.length];
      if (y1 === y2) continue;
      const yy = y + 0.5;
      if (yy >= Math.min(y1, y2) && yy < Math.max(y1, y2)) {
        xs.push(x1 + ((yy - y1) / (y2 - y1)) * (x2 - x1));
      }
    }
    xs.sort((a, b) => a - b);
    for (let k = 0; k + 1 < xs.length; k += 2) {
      for (let x = Math.floor(xs[k]); x < Math.ceil(xs[k + 1]); x++) px(x, y, r, g, b, 255);
    }
  }
}

const cx = S / 2;
// main body (upward arrow / plane)
polygon(
  [
    [cx, 232],
    [cx + 250, 720],
    [cx, 610],
    [cx - 250, 720],
  ],
  255, 255, 255,
);
// inner shadow fold
polygon(
  [
    [cx, 232],
    [cx, 610],
    [cx - 250, 720],
  ],
  222, 236, 255,
);
// exhaust
polygon(
  [
    [cx - 70, 690],
    [cx + 70, 690],
    [cx + 40, 860],
    [cx, 800],
    [cx - 40, 860],
  ],
  255, 255, 255,
);

// ---- encode PNG ----
function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(td) >>> 0);
  return Buffer.concat([len, td, crc]);
}
function crc32(b) {
  let c = ~0;
  for (let i = 0; i < b.length; i++) {
    c ^= b[i];
    for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
  }
  return ~c;
}

const raw = Buffer.alloc(S * (S * 4 + 1));
for (let y = 0; y < S; y++) {
  raw[y * (S * 4 + 1)] = 0;
  buf.copy(raw, y * (S * 4 + 1) + 1, y * S * 4, (y + 1) * S * 4);
}
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(S, 0);
ihdr.writeUInt32BE(S, 4);
ihdr[8] = 8;
ihdr[9] = 6; // RGBA
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

mkdirSync("src-tauri/icons", { recursive: true });
writeFileSync("src-tauri/icons/source.png", png);
console.log("wrote src-tauri/icons/source.png", png.length, "bytes");
