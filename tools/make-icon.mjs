// Generates the application icon (app-icon.png) with no external dependencies.
// Run: node tools/make-icon.mjs
// Then: npx tauri icon app-icon.png -o src-tauri/icons
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";

const SIZE = 1024;
const SS = 3; // supersampling factor for anti-aliased edges
const W = SIZE * SS;

const px = new Float32Array(W * W * 4); // straight RGBA, 0..1

const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const mix = (a, b, t) => a + (b - a) * t;

// Signed distance to a rounded rectangle centred at (cx, cy).
function sdRoundRect(x, y, cx, cy, hw, hh, r) {
  const qx = Math.abs(x - cx) - (hw - r);
  const qy = Math.abs(y - cy) - (hh - r);
  const ax = Math.max(qx, 0);
  const ay = Math.max(qy, 0);
  return Math.hypot(ax, ay) + Math.min(Math.max(qx, qy), 0) - r;
}

function over(dst, i, r, g, b, a) {
  const da = dst[i + 3];
  const outA = a + da * (1 - a);
  if (outA <= 0) return;
  dst[i] = (r * a + dst[i] * da * (1 - a)) / outA;
  dst[i + 1] = (g * a + dst[i + 1] * da * (1 - a)) / outA;
  dst[i + 2] = (b * a + dst[i + 2] * da * (1 - a)) / outA;
  dst[i + 3] = outA;
}

const cx = W / 2;
const cy = W / 2;

for (let y = 0; y < W; y += 1) {
  for (let x = 0; x < W; x += 1) {
    const i = (y * W + x) * 4;

    // Backplate: rounded square with a subtle top-down gradient.
    const plate = sdRoundRect(x, y, cx, cy, W * 0.49, W * 0.49, W * 0.2);
    const plateAlpha = clamp01(0.5 - plate);
    if (plateAlpha > 0) {
      const t = y / W;
      over(px, i, mix(0.075, 0.043, t), mix(0.098, 0.055, t), mix(0.145, 0.08, t), plateAlpha);
    }

    // Inner hairline so the plate reads as a physical object.
    const ringOuter = sdRoundRect(x, y, cx, cy, W * 0.49, W * 0.49, W * 0.2);
    const ringInner = sdRoundRect(x, y, cx, cy, W * 0.49 - W * 0.012, W * 0.49 - W * 0.012, W * 0.19);
    const ring = Math.min(Math.abs(ringOuter), Math.abs(ringInner));
    const ringAlpha = clamp01((W * 0.0018 - ring) * 2.5) * 0.55;
    if (ringAlpha > 0 && ringOuter < 0) {
      over(px, i, 0.31, 0.66, 1.0, ringAlpha);
    }

    // Device body: rounded rectangle outline.
    const halfW = W * 0.163;
    const halfH = W * 0.272;
    const radius = W * 0.068;
    const sd = sdRoundRect(x, y, cx, cy, halfW, halfH, radius);
    const stroke = W * 0.021;
    const edge = Math.abs(sd) - stroke / 2;
    const edgeAlpha = clamp01(0.5 - edge);

    if (sd < -stroke / 2) {
      // Glass screen inside the device.
      const t = (y - (cy - halfH)) / (halfH * 2);
      over(px, i, mix(0.16, 0.09, t), mix(0.42, 0.28, t), mix(0.78, 0.62, t), 0.16);
    }

    if (edgeAlpha > 0) {
      const t = (y - (cy - halfH)) / (halfH * 2);
      over(px, i, mix(0.36, 0.62, t), mix(0.71, 0.78, t), 1.0, edgeAlpha);
    }

    // Home indicator pill.
    const pill = sdRoundRect(x, y, cx, cy + halfH * 0.78, W * 0.048, W * 0.0095, W * 0.0095);
    const pillAlpha = clamp01(0.5 - pill) * 0.9;
    if (pillAlpha > 0) {
      over(px, i, 0.58, 0.82, 1.0, pillAlpha);
    }

    // Live indicator: green dot inside the top of the screen.
    const dot = Math.hypot(x - cx, y - (cy - halfH * 0.66)) - W * 0.019;
    const dotAlpha = clamp01(0.5 - dot);
    if (dotAlpha > 0) {
      over(px, i, 0.22, 0.83, 0.63, dotAlpha);
    }
  }
}

// Downsample with a box filter, then encode RGBA8 PNG.
const out = Buffer.alloc(SIZE * SIZE * 4);
for (let y = 0; y < SIZE; y += 1) {
  for (let x = 0; x < SIZE; x += 1) {
    let r = 0;
    let g = 0;
    let b = 0;
    let a = 0;
    for (let sy = 0; sy < SS; sy += 1) {
      for (let sx = 0; sx < SS; sx += 1) {
        const i = ((y * SS + sy) * W + (x * SS + sx)) * 4;
        const alpha = px[i + 3];
        r += px[i] * alpha;
        g += px[i + 1] * alpha;
        b += px[i + 2] * alpha;
        a += alpha;
      }
    }
    const n = SS * SS;
    const o = (y * SIZE + x) * 4;
    const alpha = a / n;
    if (alpha > 0) {
      out[o] = Math.round(clamp01(r / a) * 255);
      out[o + 1] = Math.round(clamp01(g / a) * 255);
      out[o + 2] = Math.round(clamp01(b / a) * 255);
    }
    out[o + 3] = Math.round(clamp01(alpha) * 255);
  }
}

const table = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i += 1) c = table[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([length, body, crc]);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // RGBA
ihdr[10] = 0;
ihdr[11] = 0;
ihdr[12] = 0;

const stride = SIZE * 4;
const raw = Buffer.alloc((stride + 1) * SIZE);
for (let y = 0; y < SIZE; y += 1) {
  raw[y * (stride + 1)] = 0; // filter: none
  out.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
}

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

writeFileSync("app-icon.png", png);
console.log(`wrote app-icon.png (${(png.length / 1024).toFixed(1)} kB, ${SIZE}x${SIZE})`);
