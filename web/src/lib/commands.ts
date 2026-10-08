import type { Align, Event, FeedAmount, LineSpacing, Page, Position, Snapshot } from "@/api/generated/types.gen";

export type Described = { label: string; detail: string };

const ALIGN: Record<Align, string> = { left: "left", center: "centre", right: "right" };

const amount = (feed: FeedAmount) =>
  feed === "page" ? "a page" : "lines" in feed ? `${feed.lines} lines` : `${feed.units} units`;

const spacing = (line: LineSpacing) => {
  if (line === "default") return "default";
  if ("units" in line) return `${line.units} units`;
  if ("sixtieths" in line) return `${line.sixtieths}/60 in`;
  if ("three_sixtieths" in line) return `${line.three_sixtieths}/360 in`;
  return `${line.dots} dots`;
};

const position = (at: Position) =>
  "absolute" in at ? `${at.absolute} units from the left` : `${at.relative} units from here`;

const page = (mode: Page) => {
  if (typeof mode === "string") return mode;
  if ("direction" in mode) return `direction ${mode.direction}`;
  if ("area" in mode) return `${mode.area.width} × ${mode.area.height} at ${mode.area.x}, ${mode.area.y}`;
  return `vertical ${position(mode.vertical)}`;
};

const onOff = (on: boolean) => (on ? "on" : "off");

function assertNever(value: never): never {
  throw new Error(`an event was not described: ${JSON.stringify(value)}`);
}

/** What a command is called, and what it asked for, in words. */
export function describe(event: Event): Described {
  switch (event.type) {
    case "initialize":
      return { label: "Initialize", detail: "" };
    case "align":
      return { label: "Align", detail: ALIGN[event.value] };
    case "font":
      return { label: "Font", detail: String.fromCharCode(65 + event.value) };
    case "feed":
      return { label: "Feed", detail: amount(event.value.amount) + (event.value.reverse ? " backwards" : "") };
    case "line_spacing":
      return { label: "Line spacing", detail: spacing(event.value) };
    case "position":
      return { label: "Position", detail: position(event.value) };
    case "margin":
      return { label: "Left margin", detail: `${event.value} units` };
    case "print_width":
      return { label: "Print width", detail: `${event.value} units` };
    case "motion_units":
      return { label: "Motion units", detail: `x 1/${event.value.x} in, y 1/${event.value.y} in` };
    case "tab_stops":
      return { label: "Tab stops", detail: event.value.join(", ") };
    case "density":
      return { label: "Print density", detail: String(event.value) };
    case "text":
      return { label: "Text", detail: JSON.stringify(event.value.text) };
    case "line_feed":
      return { label: "Line feed", detail: "" };
    case "image":
      return { label: "Image", detail: `${event.value.width} × ${event.value.height} dots` };
    case "barcode":
      return { label: "Barcode", detail: `${event.value.symbology}, ${event.value.data}` };
    case "qr":
      return { label: "QR code", detail: `model ${event.value.model}, level ${event.value.level}` };
    case "pdf417":
      return { label: "PDF417", detail: event.value.data };
    case "symbol":
      return { label: "Symbol", detail: event.value.kind };
    case "cut":
      return { label: "Cut", detail: event.value };
    case "pulse":
      return { label: "Cash drawer", detail: `pin ${event.value.pin}` };
    case "beep":
      return { label: "Beep", detail: `${event.value.times} times` };
    case "define_character":
      return { label: "Define character", detail: String(event.value.code) };
    case "delete_character":
      return { label: "Delete character", detail: String(event.value) };
    case "user_characters":
      return { label: "User characters", detail: onOff(event.value) };
    case "define_kanji":
      return { label: "Define Kanji", detail: event.value.code.join(" ") };
    case "graphics_density":
      return { label: "Graphics density", detail: String(event.value) };
    case "page":
      return { label: "Page mode", detail: page(event.value) };
    case "unknown":
      return { label: "Not read", detail: event.value.bytes };
    default:
      return assertNever(event);
  }
}

/** What the printer believed after a command, as names and values. */
export function stateRows(state: Snapshot): { name: string; value: string }[] {
  const { style, layout } = state;
  return [
    { name: "Align", value: ALIGN[state.align] },
    { name: "Font", value: String.fromCharCode(65 + style.font) },
    { name: "Size", value: `${style.width} × ${style.height}` },
    { name: "Bold", value: onOff(style.bold) },
    { name: "Double strike", value: onOff(style.double_strike) },
    { name: "Underline", value: ["off", "thin", "thick"][style.underline] ?? String(style.underline) },
    { name: "Invert", value: onOff(style.invert) },
    { name: "Upside down", value: onOff(style.upside_down) },
    { name: "Rotated", value: onOff(style.rotated) },
    { name: "Overline", value: onOff(style.overline) },
    { name: "Smooth", value: onOff(style.smooth) },
    { name: "Colour", value: style.color === 0 ? "black" : "second" },
    { name: "Character spacing", value: `${style.spacing} units` },
    { name: "Code page", value: state.code_page ?? `number ${state.code_page_number}` },
    { name: "International set", value: String(state.international) },
    { name: "Line spacing", value: spacing(layout.line_spacing) },
    { name: "Left margin", value: `${layout.margin} units` },
    { name: "Print width", value: layout.print_width == null ? "the rest of the paper" : `${layout.print_width} units` },
    { name: "Kanji", value: onOff(state.kanji) },
    { name: "UTF-8", value: onOff(state.utf8) },
    { name: "User characters", value: onOff(state.user_characters) },
    { name: "Page mode", value: onOff(state.page_mode) },
  ];
}
