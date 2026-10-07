import { Resvg } from "@resvg/resvg-js";
import { receiptRows } from "./data.mjs";

const FONT_DIR = "node_modules/@expo-google-fonts";
const FONT_FILES = [
	`${FONT_DIR}/noto-sans-mono/400Regular/NotoSansMono_400Regular.ttf`,
	`${FONT_DIR}/noto-sans-mono/700Bold/NotoSansMono_700Bold.ttf`,
	`${FONT_DIR}/noto-naskh-arabic/400Regular/NotoNaskhArabic_400Regular.ttf`,
	`${FONT_DIR}/noto-naskh-arabic/700Bold/NotoNaskhArabic_700Bold.ttf`,
];

const escapeXml = (text) => text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** Keeps a code like 8F2A in one piece inside right to left text, where bidi would split it. */
const isolateLatin = (text) => text.replace(/[A-Za-z0-9]*[A-Za-z][A-Za-z0-9]*/g, "\u2066$&\u2069");

/** Breaks text at spaces so no line is longer than `columns`. */
function wrap(text, columns) {
	const lines = [];
	let current = "";
	for (const word of text.split(" ")) {
		const next = current ? `${current} ${word}` : word;
		if (next.length > columns && current) {
			lines.push(current);
			current = word;
		} else {
			current = next;
		}
	}
	if (current) lines.push(current);
	return lines;
}

/**
 * Draws the receipt as a picture `width` dots wide, in pure black and white, and returns it both as
 * pixels for the library and as the packed bits a printer must end up with.
 */
export function drawReceipt(receipt, width) {
	const rtl = receipt.lang === "ar";
	const margin = 12;
	const size = width === 384 ? 22 : 26;
	const lineHeight = Math.round(size * 1.5);
	const maxChars = Math.floor((width - margin * 2) / (size * 0.6));
	const start = rtl ? width - margin : margin;
	const end = rtl ? margin : width - margin;
	const startAnchor = rtl ? "end" : "start";
	const endAnchor = rtl ? "start" : "end";

	let y = lineHeight;
	const parts = [];
	const text = (value, x, anchor, { bold = false, scale = 1 } = {}) => {
		const attributes = `x="${x}" y="${y}" text-anchor="${anchor}"${rtl ? ' direction="rtl"' : ""}${bold ? ' font-weight="700"' : ""}${scale !== 1 ? ` font-size="${Math.round(size * scale)}"` : ""}`;
		parts.push(`<text ${attributes}>${escapeXml(rtl ? isolateLatin(value) : value)}</text>`);
	};
	const rule = (double) => {
		const line = (offset) => `<line x1="${margin}" y1="${y + offset}" x2="${width - margin}" y2="${y + offset}" stroke="black" stroke-width="2"/>`;
		parts.push(line(-size / 2), ...(double ? [line(-size / 2 + 5)] : []));
	};

	for (const row of receiptRows(receipt)) {
		if (row.kind === "title") {
			text(row.text, width / 2, "middle", { bold: true, scale: 1.3 });
			y += lineHeight * 1.2;
		} else if (row.kind === "center") {
			for (const line of wrap(row.text, maxChars)) {
				text(line, width / 2, "middle");
				y += lineHeight;
			}
		} else if (row.kind === "rule") {
			rule(row.style === "double");
			y += lineHeight * 0.7;
		} else if (row.kind === "blank") {
			y += lineHeight;
		} else if (row.kind === "pair") {
			text(row.label, start, startAnchor, { bold: row.bold, scale: row.bold ? 1.15 : 1 });
			text(row.value, end, endAnchor, { bold: row.bold, scale: row.bold ? 1.15 : 1 });
			y += lineHeight;
		} else if (row.kind === "item") {
			for (const line of wrap(row.name, maxChars)) {
				text(line, start, startAnchor);
				y += lineHeight;
			}
			text(row.detail, start, startAnchor);
			text(row.total, end, endAnchor);
			y += lineHeight;
		}
	}

	const height = Math.ceil((y + lineHeight / 2) / 8) * 8;
	const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
<rect width="${width}" height="${height}" fill="white"/>
<g font-family="Noto Sans Mono, Noto Naskh Arabic" font-size="${size}" fill="black">${parts.join("")}</g>
</svg>`;

	const rendered = new Resvg(svg, {
		font: { fontFiles: FONT_FILES, loadSystemFonts: false, defaultFontFamily: "Noto Sans Mono" },
	}).render();

	// Anti-aliasing leaves greys; make every pixel pure black or white before anyone thresholds it.
	const pixels = rendered.pixels;
	const rgba = new Uint8ClampedArray(width * height * 4);
	const rowBytes = width / 8;
	const bits = new Uint8Array(rowBytes * height);
	for (let index = 0; index < width * height; index++) {
		const luminance = (pixels[index * 4] * 299 + pixels[index * 4 + 1] * 587 + pixels[index * 4 + 2] * 114) / 1000;
		const dark = luminance < 128;
		const value = dark ? 0 : 255;
		rgba.set([value, value, value, 255], index * 4);
		if (dark) bits[Math.floor(index / width) * rowBytes + ((index % width) >> 3)] |= 0x80 >> (index % 8);
	}
	return { rgba, bits, width, height };
}
