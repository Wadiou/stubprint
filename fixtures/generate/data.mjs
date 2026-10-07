/**
 * One small, fixed receipt in three languages. It is only here to have something to print:
 * a header, two items, a total, and a code. The fixtures are about how the printer commands are
 * decoded, not about the shop.
 */
const TEXT = {
	en: {
		store: "Green Market",
		address: "12 Main Street, Springfield",
		items: ["Whole milk 1 L", "White sugar 1 kg"],
		labels: { order: "Order", total: "TOTAL", thanks: "Thank you", currency: "USD" },
	},
	fr: {
		store: "Marché Vert",
		address: "12 rue Principale, Springfield",
		items: ["Lait entier 1 L", "Café moulu 250 g"],
		labels: { order: "Commande", total: "TOTAL", thanks: "Merci de votre visite", currency: "EUR" },
	},
	ar: {
		store: "السوق الأخضر",
		address: "12 الشارع الرئيسي، المدينة",
		items: ["حليب كامل الدسم 1 لتر", "سكر أبيض 1 كغ"],
		labels: { order: "الطلب", total: "المجموع", thanks: "شكرا لزيارتكم", currency: "دج" },
	},
};

/** 46050 cents is 460.50 in English and Arabic and 460,50 in French. */
export function money(cents, lang) {
	const whole = Math.floor(cents / 100).toString();
	const fraction = (cents % 100).toString().padStart(2, "0");
	return `${whole}${lang === "fr" ? "," : "."}${fraction}`;
}

export function makeReceipt({ lang }) {
	const text = TEXT[lang];
	if (!text) throw new Error(`no receipt text for the language "${lang}"`);
	const lines = [
		{ name: text.items[0], quantity: 2, unit: 14000 },
		{ name: text.items[1], quantity: 1, unit: 18050 },
	];
	return {
		lang,
		code: "8F2A",
		store: text.store,
		address: text.address,
		labels: text.labels,
		date: "2026-10-07 14:32",
		lines,
		total: lines.reduce((sum, line) => sum + line.quantity * line.unit, 0),
	};
}

/**
 * The receipt as rows, in reading order. Both ways of printing it, with the printer's own
 * fonts and as a picture, are drawn from these same rows.
 */
export function receiptRows(receipt) {
	const { labels, lang } = receipt;
	return [
		{ kind: "title", text: receipt.store },
		{ kind: "center", text: receipt.address },
		{ kind: "rule", style: "double" },
		{ kind: "pair", label: `${labels.order} ${receipt.code}`, value: receipt.date },
		{ kind: "rule", style: "single" },
		...receipt.lines.map((line) => ({
			kind: "item",
			name: line.name,
			detail: `${line.quantity} x ${money(line.unit, lang)}`,
			total: money(line.quantity * line.unit, lang),
		})),
		{ kind: "rule", style: "single" },
		{ kind: "pair", label: labels.total, value: `${money(receipt.total, lang)} ${labels.currency}`, bold: true },
		{ kind: "blank" },
		{ kind: "center", text: labels.thanks },
	];
}
