/**
 * Draws everything a link to Stubprint shows: the favicon and the other icons, which the web page
 * of the program and the landing page both use, and the card that a link to the landing page
 * shows, from the name, the headline and the colours of landing/src/brand.ts and the receipt
 * picture and the typefaces of the landing page.
 *
 * It lives here, its own small package, and not inside landing/, because the icons are not only
 * the landing page's: a single script, installed once, draws them for both.
 *
 * Writes the icons to landing/public/ and web/public/, the card to landing/public/og.png, and the
 * card again, under another name since it is not a link into there, to docs/banner.png for the
 * README.
 *
 * Run: pnpm install, once, then pnpm brand
 */
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { COLORS, DOMAIN, HEADLINE, NAME } from "../landing/src/brand.ts";

const require = createRequire(import.meta.url);
const HERE = path.dirname(fileURLToPath(import.meta.url));
const RECEIPT_PNG = `data:image/png;base64,${(await fs.readFile(path.join(HERE, "../landing/src/assets/receipt.png"))).toString("base64")}`;
const DOCS = path.join(HERE, "../docs");
const LANDING_PUBLIC = path.join(HERE, "../landing/public");
const WEB_PUBLIC = path.join(HERE, "../web/public");

/** A typeface of the page as a rule that loads it into the card, so the card needs no network. */
async function face(pkg, family, file) {
  const dir = path.dirname(require.resolve(`${pkg}/package.json`));
  const data = await fs.readFile(path.join(dir, "files", file));
  return `@font-face{font-family:"${family}";font-weight:100 900;src:url(data:font/woff2;base64,${data.toString("base64")}) format("woff2")}`;
}

const FONTS = [
  await face("@fontsource-variable/geist", "Geist", "geist-latin-wght-normal.woff2"),
  await face("@fontsource-variable/geist-mono", "Geist Mono", "geist-mono-latin-wght-normal.woff2"),
].join("\n");

const BASE = `${FONTS}
*{box-sizing:border-box;margin:0}
html,body{width:100%;height:100%;background:transparent}`;

/** The icon: the first letter of the name on a light tile. */
const icon = () => `<style>${BASE}
div{width:100vw;height:100vh;display:grid;place-items:center;background:${COLORS.foreground};color:#18181B;
font:600 66vmin/1 Geist,sans-serif;letter-spacing:-0.04em}
</style><body><div>${NAME[0]}</div></body>`;

/** The card that a link to the landing page shows. */
const card = () => `<style>${BASE}
body{display:flex;align-items:center;justify-content:space-between;gap:64px;padding:72px 88px;
background:${COLORS.background};color:${COLORS.foreground};font-family:Geist,sans-serif}
.text{display:flex;flex-direction:column;gap:28px;max-width:600px}
.name{font-size:44px;font-weight:600;letter-spacing:-0.03em}
h1{font-size:60px;line-height:1.08;font-weight:600;letter-spacing:-0.035em}
.domain{font:500 26px "Geist Mono",monospace;color:${COLORS.muted}}
.receipt{flex:none;width:280px;margin:-40px 0;padding:16px 16px 10px;box-sizing:border-box;background:${COLORS.paper};transform:rotate(2deg);box-shadow:0 24px 60px rgb(0 0 0/.55)}
.receipt img{display:block;width:100%}
</style><body>
<div class="text"><div class="name">${NAME}</div><h1>${HEADLINE}</h1><div class="domain">${DOMAIN}</div></div>
<div class="receipt"><img src="${RECEIPT_PNG}"></div>
</body>`;

/** An .ico is a small header followed by the PNGs themselves. */
function toIco(pngs) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(pngs.length, 4);
  const entries = Buffer.alloc(16 * pngs.length);
  let offset = header.length + entries.length;
  pngs.forEach(({ size, data }, index) => {
    const at = index * 16;
    entries.writeUInt8(size, at);
    entries.writeUInt8(size, at + 1);
    entries.writeUInt16LE(1, at + 4);
    entries.writeUInt16LE(32, at + 6);
    entries.writeUInt32LE(data.length, at + 8);
    entries.writeUInt32LE(offset, at + 12);
    offset += data.length;
  });
  return Buffer.concat([header, entries, ...pngs.map(({ data }) => data)]);
}

/** Writes an icon to every folder that uses it. */
async function writeIcon(file, data) {
  for (const dir of [LANDING_PUBLIC, WEB_PUBLIC]) {
    await fs.mkdir(dir, { recursive: true });
    await fs.writeFile(path.join(dir, file), data);
  }
  console.log(`  landing/public/${file}, web/public/${file}`);
}

const browser = await chromium.launch();
try {
  const shot = async (html, width, height) => {
    const page = await browser.newPage({ viewport: { width, height } });
    await page.setContent(html, { waitUntil: "load" });
    await page.evaluate(() => document.fonts.ready);
    const data = await page.screenshot({ type: "png" });
    await page.close();
    return data;
  };

  console.log("Icons");
  await writeIcon("apple-touch-icon.png", await shot(icon(), 180, 180));
  await writeIcon("favicon-32x32.png", await shot(icon(), 32, 32));
  await writeIcon(
    "favicon.ico",
    toIco(await Promise.all([16, 32, 48].map(async (size) => ({ size, data: await shot(icon(), size, size) })))),
  );

  console.log("Card");
  await fs.mkdir(LANDING_PUBLIC, { recursive: true });
  const og = await shot(card(), 1200, 630);
  await fs.writeFile(path.join(LANDING_PUBLIC, "og.png"), og);
  console.log("  landing/public/og.png");
  await fs.writeFile(path.join(DOCS, "banner.png"), og);
  console.log("  docs/banner.png");
} finally {
  await browser.close();
}
