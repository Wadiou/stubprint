import { Resvg } from "@resvg/resvg-js";
import bwipjs from "bwip-js";

const FONT_DIR = "node_modules/@expo-google-fonts";
const FONT_FILES = [
	`${FONT_DIR}/noto-sans-mono/400Regular/NotoSansMono_400Regular.ttf`,
	`${FONT_DIR}/noto-naskh-arabic/400Regular/NotoNaskhArabic_400Regular.ttf`,
];

/** The names the library uses, and the ones bwip-js draws them with. */
const BWIP_SYMBOLOGY = {
	upca: "upca",
	upce: "upce",
	ean13: "ean13",
	ean8: "ean8",
	code39: "code39",
	itf: "interleaved2of5",
	codabar: "rationalizedCodabar",
	code93: "code93",
	code128: "code128",
};

const escapeXml = (text) => text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const pngSize = (png) => ({ width: png.readUInt32BE(16), height: png.readUInt32BE(20) });
const dataUri = (png) => `data:image/png;base64,${png.toString("base64")}`;

/** A barcode, QR code or PDF417 as the picture a printer would end up with, drawn by bwip-js. */
async function codeImage(options) {
	const png = await bwipjs.toBuffer({ backgroundcolor: "FFFFFF", paddingwidth: 4, paddingheight: 4, ...options });
	return { png, ...pngSize(png) };
}

/**
 * A picture of what a stream prints, for browsing the fixtures: the printed picture, the text
 * lines, then every barcode and 2D code, which the printer would draw itself. The codes are drawn
 * by bwip-js, so they show what is encoded, not how a particular printer renders it.
 * Returns nothing when a stream has nothing to show.
 */
export async function renderPreview({ expected, bitmapPng, lines }) {
	const codes = [];
	for (const barcode of expected.barcodes ?? []) {
		const hri = barcode.text !== "none";
		codes.push({
			caption: `${barcode.symbology}  ${barcode.data}`,
			...(await codeImage({ bcid: BWIP_SYMBOLOGY[barcode.symbology], text: barcode.data, scale: barcode.width ?? 2, height: Math.round((barcode.height ?? 60) / 4), includetext: hri })),
		});
	}
	for (const qr of expected.qrs ?? []) {
		codes.push({
			caption: `qr model ${qr.model}, level ${qr.level}, size ${qr.size}  ${qr.data}`,
			...(await codeImage({ bcid: "qrcode", text: qr.data, scale: Math.max(2, qr.size), eclevel: qr.level })),
		});
	}
	for (const pdf of expected.pdf417 ?? []) {
		codes.push({ caption: `pdf417  ${pdf.data}`, ...(await codeImage({ bcid: "pdf417", text: pdf.data, scale: 3 })) });
	}
	const texts = lines ?? expected.texts ?? [];
	if (!bitmapPng && codes.length === 0 && texts.length === 0) return undefined;

	const width = Math.max(expected.image?.width ?? 0, expected.columns ? expected.columns * 12 : 0, 384);
	const parts = [];
	let y = 8;

	if (bitmapPng) {
		parts.push(`<image x="${(width - expected.image.width) / 2}" y="${y}" width="${expected.image.width}" height="${expected.image.height}" href="${dataUri(bitmapPng)}"/>`);
		y += expected.image.height + 8;
	}
	for (const line of texts) {
		y += 24;
		parts.push(`<text x="12" y="${y}" xml:space="preserve" font-size="20">${escapeXml(line)}</text>`);
		y += 4;
	}
	if (texts.length > 0) y += 8;
	for (const code of codes) {
		y += 18;
		parts.push(`<text x="12" y="${y}" font-size="14" fill="#666">${escapeXml(code.caption)}</text>`);
		y += 6;
		parts.push(`<image x="${Math.max(0, (width - code.width) / 2)}" y="${y}" width="${code.width}" height="${code.height}" href="${dataUri(code.png)}"/>`);
		y += code.height + 10;
	}
	const height = y + 8;
	const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"><rect width="${width}" height="${height}" fill="white"/><g font-family="Noto Sans Mono, Noto Naskh Arabic" fill="black">${parts.join("")}</g></svg>`;
	return new Resvg(svg, {
		font: { fontFiles: FONT_FILES, loadSystemFonts: false, defaultFontFamily: "Noto Sans Mono" },
	}).render().asPng();
}
