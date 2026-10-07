import { receiptRows } from "./data.mjs";

/** Breaks text at spaces so no line is longer than `columns`; a word that is too long is cut. */
function wrap(text, columns) {
	const lines = [];
	let current = "";
	for (let word of text.split(" ")) {
		while (word.length > columns) {
			if (current) {
				lines.push(current);
				current = "";
			}
			lines.push(word.slice(0, columns));
			word = word.slice(columns);
		}
		const next = current ? `${current} ${word}` : word;
		if (next.length > columns) {
			lines.push(current);
			current = word;
		} else {
			current = next;
		}
	}
	if (current) lines.push(current);
	return lines;
}

const center = (text, columns) => " ".repeat(Math.floor((columns - text.length) / 2)) + text;
const justify = (start, end, columns) => start + " ".repeat(Math.max(1, columns - start.length - end.length)) + end;

/**
 * The receipt as lines of text for the printer's own font, where each character is one column.
 * Right to left receipts put the label on the right and the amount on the left, and the Arabic
 * is left in reading order, which is how a printer without shaping prints it.
 */
export function textLines(receipt, columns) {
	const rtl = receipt.lang === "ar";
	const lines = [];
	const add = (text, bold = false) => lines.push({ text, bold });

	for (const row of receiptRows(receipt)) {
		if (row.kind === "title") {
			for (const line of wrap(row.text, columns)) add(center(line, columns), true);
		} else if (row.kind === "center") {
			for (const line of wrap(row.text, columns)) add(center(line, columns));
		} else if (row.kind === "rule") {
			add((row.style === "double" ? "=" : "-").repeat(columns));
		} else if (row.kind === "blank") {
			add("");
		} else if (row.kind === "pair") {
			const [first, ...rest] = wrap(row.label, columns - row.value.length - 1);
			add(rtl ? justify(row.value, first, columns) : justify(first, row.value, columns), row.bold);
			for (const line of rest) add(rtl ? line.padStart(columns) : line);
		} else if (row.kind === "item") {
			for (const line of wrap(row.name, columns)) add(rtl ? line.padStart(columns) : line);
			add(rtl ? justify(row.total, row.detail, columns) : justify(`  ${row.detail}`, row.total, columns));
		}
	}
	return lines;
}
