// Builds the printer streams the decoder tests read, from the recipes in fixtures/specs/.
//
//   npm run generate       write fixtures/out/ and check it against fixtures/fixtures.lock.json
//   npm run update-lock    the same, then accept the result as the new lock
//
// The outputs are not committed. The lock holds a hash of each one, so a library, font or spec
// change that moves a single dot is noticed instead of silently changing what the tests check.
// The expected results come from what a spec asks for, not from reading the library's output.
import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { crc32, deflateSync } from "node:zlib";
import CodepageEncoder from "@point-of-sale/codepage-encoder";
import ReceiptPrinterEncoder from "@point-of-sale/receipt-printer-encoder";
import { makeReceipt } from "./data.mjs";
import { drawReceipt } from "./layout-image.mjs";
import { renderPreview } from "./preview.mjs";
import { textLines } from "./layout-text.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const SPECS = path.resolve(HERE, "../specs");
const OUT_ROOT = path.resolve(HERE, "../out");
const OUT = path.join(OUT_ROOT, "escpos");
const PREVIEWS = path.join(OUT_ROOT, "previews");
const LOCK = path.resolve(HERE, "../fixtures.lock.json");
const UPDATE_LOCK = process.argv.includes("--update-lock");

const encoder = (columns = 48, imageMode = "raster") => new ReceiptPrinterEncoder({ language: "esc-pos", imageMode, columns });
const columnsFor = (width) => (width === 384 ? 32 : 48);
const dotsFor = (paper) => (paper === 58 ? 384 : 576);
const orderUrl = (receipt) => `https://example.com/orders/${receipt.code}`;

function fail(spec, message) {
	throw new Error(`spec ${spec.name}: ${message}`);
}

/** How a code page is read back in Node, to prove the library's bytes decode to the same text. */
const CHECK_LABELS = { cp437: "utf-8", windows1252: "windows-1252", windows1256: "windows-1256" };

function checkCodepage(spec, text, codepage) {
	if (!CHECK_LABELS[codepage]) {
		if (/[^\x20-\x7e]/.test(text)) fail(spec, `no way to check "${text}" in the code page "${codepage}"`);
		return;
	}
	const decoded = new TextDecoder(CHECK_LABELS[codepage]).decode(CodepageEncoder.encode(text, codepage));
	if (decoded !== text) fail(spec, `"${text}" does not survive ${codepage}, it comes back as "${decoded}"`);
}

const codes = (receipt) => ({
	barcodes: [{ symbology: "code128", data: receipt.code, height: 60 }],
	qrs: [{ data: orderUrl(receipt), model: 2, size: 4, level: "Q" }],
});

/** A receipt in the printer's own font, one character per column, in a code page. */
function native(spec) {
	const receipt = makeReceipt(spec.receipt);
	const width = dotsFor(spec.paper);
	const columns = columnsFor(width);
	const lines = textLines(receipt, columns);
	const out = encoder(columns).initialize().codepage(spec.codepage);
	for (const { text, bold } of lines) {
		checkCodepage(spec, text, spec.codepage);
		if (bold) out.bold(true).line(text).bold(false);
		else out.line(text);
	}
	const bytes = out.barcode(receipt.code, "code128", 60).qrcode(orderUrl(receipt), 2, 4, "q").newline().cut("full").encode();
	return {
		bytes,
		expected: {
			receipts: 1,
			codepage: spec.codepage,
			columns,
			texts: lines.filter((line) => line.text !== "").map((line) => line.text),
			boldTexts: lines.filter((line) => line.bold).map((line) => line.text),
			...codes(receipt),
			cuts: ["full"],
		},
		text: lines.map((line) => line.text).join("\n"),
	};
}

/** A receipt drawn as a picture, so any language prints. A hybrid adds the printer's own barcode and QR. */
function picture(spec) {
	const receipt = makeReceipt(spec.receipt);
	const drawn = drawReceipt(receipt, dotsFor(spec.paper));
	const out = encoder().initialize().image({ data: drawn.rgba, width: drawn.width, height: drawn.height, colorSpace: "srgb" }, drawn.width, drawn.height, "threshold", 128);
	const hybrid = spec.mode === "hybrid";
	if (hybrid) out.barcode(receipt.code, "code128", 60).qrcode(orderUrl(receipt), 2, 4, "q");
	const bytes = out.newline().cut("full").encode();
	const bands = Math.ceil(drawn.height / 255);
	const found = countRasterCommands(bytes);
	if (found !== bands) fail(spec, `expected ${bands} raster commands, the stream has ${found}`);
	return {
		bytes,
		bitmap: drawn.bits,
		expected: {
			receipts: 1,
			image: { width: drawn.width, height: drawn.height },
			rasterBands: bands,
			...(hybrid ? codes(receipt) : {}),
			cuts: ["full"],
		},
	};
}

function countRasterCommands(bytes) {
	let count = 0;
	for (let i = 0; i + 3 < bytes.length; i++) {
		if (bytes[i] === 0x1d && bytes[i + 1] === 0x76 && bytes[i + 2] === 0x30 && bytes[i + 3] === 0x00) count++;
	}
	return count;
}

/**
 * A frame, two diagonals and, when there is room, a checkerboard. A wrong bit shows as a visible
 * flaw. Raster mode pads the height to a multiple of 8, column mode to a multiple of 24.
 */
function pattern(spec) {
	const { height } = spec;
	const width = spec.width ?? 384;
	const mode = spec.imageMode ?? "raster";
	const strip = mode === "column" ? 24 : 8;
	const padded = Math.ceil(height / strip) * strip;
	const rowBytes = width / 8;
	const bits = new Uint8Array(rowBytes * padded);
	const data = new Uint8ClampedArray(width * height * 4).fill(255);
	for (let y = 0; y < height; y++) {
		for (let x = 0; x < width; x++) {
			const frame = x < 4 || x >= width - 4 || y < 4 || y >= height - 4;
			const diagonal = Math.abs(x - y) < 3 || Math.abs(width - x - y) < 3;
			const checker = width >= 128 && height >= 128 && x >= 40 && x < 104 && y >= 40 && y < 104 && ((x >> 3) + (y >> 3)) % 2 === 0;
			if (frame || diagonal || checker) {
				bits[y * rowBytes + (x >> 3)] |= 0x80 >> (x & 7);
				data.set([0, 0, 0], (y * width + x) * 4);
			}
		}
	}
	const stream = encoder(48, mode).initialize().image({ data, width, height, colorSpace: "srgb" }, width, height, "threshold", 128).cut("full").encode();
	const expected = { receipts: 1, image: { width, height: padded }, cuts: ["full"] };
	if (mode === "raster") {
		expected.rasterBands = Math.ceil(padded / 255);
		const found = countRasterCommands(stream);
		if (found !== expected.rasterBands) fail(spec, `expected ${expected.rasterBands} raster commands, the stream has ${found}`);
	} else {
		expected.columnStrips = padded / 24;
		const found = countColumnStrips(stream);
		if (found !== expected.columnStrips) fail(spec, `expected ${expected.columnStrips} column strips, the stream has ${found}`);
	}
	return { bytes: stream, bitmap: bits, expected };
}

function countColumnStrips(bytes) {
	let count = 0;
	for (let i = 0; i + 2 < bytes.length; i++) {
		if (bytes[i] === 0x1b && bytes[i + 1] === 0x2a && bytes[i + 2] === 0x21) count++;
	}
	return count;
}

/** The options that were given, so the library's own defaults apply to the rest. */
const defined = (options) => Object.fromEntries(Object.entries(options).filter(([, value]) => value !== undefined));

/**
 * An ordered list of library calls, for one feature at a time. What the stream must decode to is
 * written in the spec, by hand, and is not read back from the library.
 */
function steps(spec) {
	const out = encoder();
	let codepage;
	for (const step of spec.steps) {
		switch (step.op) {
			case "initialize": out.initialize(); break;
			case "codepage": out.codepage(step.name); codepage = step.name; break;
			case "font": out.font(step.name); break;
			case "align": out.align(step.value); break;
			case "bold": out.bold(step.value); break;
			case "underline": out.underline(step.value); break;
			case "invert": out.invert(step.value); break;
			case "size": out.size(step.width, step.height); break;
			case "line":
				if (codepage) checkCodepage(spec, step.text, codepage);
				else if (/[^\x20-\x7e]/.test(step.text)) fail(spec, `"${step.text}" needs a codepage step before it`);
				out.line(step.text);
				break;
			case "newline": out.newline(step.count); break;
			case "rule": out.rule({ style: step.style }); break;
			case "barcode": out.barcode(step.value, step.symbology, defined({ height: step.height, width: step.width, text: step.text })); break;
			case "qrcode": out.qrcode(step.value, defined({ model: step.model, size: step.size, errorlevel: step.level })); break;
			case "pdf417": out.pdf417(step.value); break;
			case "pulse": out.pulse(step.device, step.on, step.off); break;
			case "cut": out.cut(step.mode); break;
			default: fail(spec, `unknown step "${step.op}"`);
		}
	}
	return { bytes: out.encode(), expected: spec.expect };
}

/** Hand written bytes: hex pairs and `text:...`, for cases no encoder would ever produce. */
function bytes(spec) {
	const out = [];
	for (const token of spec.bytes) {
		if (token.startsWith("text:")) {
			const text = token.slice(5);
			for (const char of text) {
				if (char.charCodeAt(0) < 0x20 || char.charCodeAt(0) > 0x7e) fail(spec, `"${text}" is not printable ASCII`);
				out.push(char.charCodeAt(0));
			}
		} else {
			for (const pair of token.trim().split(/\s+/)) {
				if (!/^[0-9a-fA-F]{2}$/.test(pair)) fail(spec, `"${pair}" is not a hex byte`);
				out.push(Number.parseInt(pair, 16));
			}
		}
	}
	return { bytes: Uint8Array.from(out), expected: spec.expect };
}

function build(spec) {
	if (spec.kind === "receipt") {
		if (spec.mode === "native") return native(spec);
		if (spec.mode === "picture" || spec.mode === "hybrid") return picture(spec);
		return fail(spec, `unknown mode "${spec.mode}"`);
	}
	if (spec.kind === "pattern") return pattern(spec);
	if (spec.kind === "steps") return steps(spec);
	if (spec.kind === "bytes") return bytes(spec);
	return fail(spec, `unknown kind "${spec.kind}"`);
}

// --- previews, so an editor can show what each file holds. The tests do not read them.

/** A 1-bit grayscale PNG of a packed bitmap (black is 1). */
function bitmapToPng(bits, width, height) {
	const rowBytes = width / 8;
	const raw = Buffer.alloc((rowBytes + 1) * height);
	for (let y = 0; y < height; y++) {
		for (let i = 0; i < rowBytes; i++) raw[y * (rowBytes + 1) + 1 + i] = ~bits[y * rowBytes + i] & 0xff;
	}
	const chunk = (type, body) => {
		const data = Buffer.concat([Buffer.from(type), body]);
		const out = Buffer.alloc(data.length + 8);
		out.writeUInt32BE(body.length, 0);
		data.copy(out, 4);
		out.writeUInt32BE(crc32(data), data.length + 4);
		return out;
	};
	const header = Buffer.alloc(13);
	header.writeUInt32BE(width, 0);
	header.writeUInt32BE(height, 4);
	header.set([1, 0, 0, 0, 0], 8);
	return Buffer.concat([
		Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
		chunk("IHDR", header),
		chunk("IDAT", deflateSync(raw)),
		chunk("IEND", Buffer.alloc(0)),
	]);
}

/** Offset, hex bytes, and the printable characters of a stream. */
function hexDump(stream) {
	const lines = [];
	for (let offset = 0; offset < stream.length; offset += 16) {
		const row = stream.subarray(offset, offset + 16);
		const hex = [...row].map((b) => b.toString(16).padStart(2, "0")).join(" ");
		const text = [...row].map((b) => (b >= 0x20 && b <= 0x7e ? String.fromCharCode(b) : ".")).join("");
		lines.push(`${offset.toString(16).padStart(8, "0")}  ${hex.padEnd(47)}  |${text}|`);
	}
	return `${lines.join("\n")}\n`;
}

const sha256 = (data) => createHash("sha256").update(data).digest("hex");

// --- run

const specFiles = (await readdir(SPECS)).filter((file) => file.endsWith(".json")).sort();
await rm(OUT_ROOT, { recursive: true, force: true });
await mkdir(OUT, { recursive: true });
for (const folder of ["png", "hex", "text"]) await mkdir(path.join(PREVIEWS, folder), { recursive: true });

const hashes = {};
for (const file of specFiles) {
	const spec = JSON.parse(await readFile(path.join(SPECS, file), "utf8"));
	if (`${spec.name}.json` !== file) fail(spec, `the file must be named ${spec.name}.json`);
	const { bytes: stream, expected, bitmap, text } = build(spec);

	const expectedJson = `${JSON.stringify(expected, null, 2)}\n`;
	await writeFile(path.join(OUT, `${spec.name}.bin`), stream);
	await writeFile(path.join(OUT, `${spec.name}.expected.json`), expectedJson);
	hashes[spec.name] = { bin: sha256(stream), expected: sha256(expectedJson) };
	let bitmapPng;
	if (bitmap) {
		await writeFile(path.join(OUT, `${spec.name}.bitmap`), bitmap);
		hashes[spec.name].bitmap = sha256(bitmap);
		bitmapPng = bitmapToPng(bitmap, expected.image.width, expected.image.height);
	} else {
		await writeFile(path.join(PREVIEWS, "hex", `${spec.name}.hex.txt`), hexDump(stream));
	}
	const preview = await renderPreview({ expected, bitmapPng, lines: text?.split("\n") });
	if (preview) await writeFile(path.join(PREVIEWS, "png", `${spec.name}.png`), preview);
	if (text) await writeFile(path.join(PREVIEWS, "text", `${spec.name}.txt`), `${text}\n`);
	console.log(`${spec.name}: ${stream.length} bytes`);
}

// --- check against, or update, the lock

if (UPDATE_LOCK) {
	await writeFile(LOCK, `${JSON.stringify(hashes, null, 2)}\n`);
	console.log(`\nLock updated: ${specFiles.length} fixtures.`);
} else {
	let lock = {};
	try {
		lock = JSON.parse(await readFile(LOCK, "utf8"));
	} catch {
		console.error("\nfixtures.lock.json is missing. Run: npm run update-lock");
		process.exit(1);
	}
	const problems = [];
	for (const [name, files] of Object.entries(hashes)) {
		if (!lock[name]) problems.push(`${name} is new and not in the lock`);
		else
			for (const [kind, hash] of Object.entries(files)) {
				if (lock[name][kind] !== hash) problems.push(`${name}.${kind} changed`);
			}
	}
	for (const name of Object.keys(lock)) if (!hashes[name]) problems.push(`${name} is in the lock but has no spec`);
	if (problems.length > 0) {
		console.error("\nFixture drift. A spec, the library or a font changed what is generated:");
		for (const problem of problems) console.error(`  - ${problem}`);
		console.error("Look at the previews in fixtures/out/previews/, then run: npm run update-lock");
		process.exit(1);
	}
	console.log(`\nAll ${specFiles.length} fixtures match the lock.`);
}
