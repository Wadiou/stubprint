import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { snapshot } from "@/test/fixtures";

// The API client keeps the fetch it finds when it is made, so the answers are set before it loads.
const answers = vi.hoisted(() => {
  const answers: Record<string, unknown> = {};
  globalThis.fetch = async (input: RequestInfo | URL) => {
    const url = new URL(input instanceof Request ? input.url : String(input), "http://stubprint");
    const answer = answers[url.pathname] ?? [];
    if (typeof answer === "string") return new Response(answer, { headers: { "content-type": "text/plain" } });
    if (answer instanceof Uint8Array) {
      return new Response(answer.slice().buffer, { headers: { "content-type": "application/octet-stream" } });
    }
    return Response.json(answer);
  };
  return answers;
});

// A list that is made only for what is in view needs a view with a size.
for (const side of ["offsetHeight", "offsetWidth"]) {
  Object.defineProperty(HTMLElement.prototype, side, { configurable: true, value: 400 });
}

const printer = { name: "front", dialect: "escpos", listen: "127.0.0.1:9100" };
const receipt = (id: number) => ({
  id,
  printer: "front",
  dialect: "escpos",
  profile: "TM-T88IV",
  at: 0,
  bytes: 16,
  events: 3,
  preview: ["Green Market", "Total 4.20"],
});
const model = {
  name: "TM-T88IV",
  vendor: "Epson",
  model: "TM-T88IV",
  columns: [42, 56],
  width_dots: 512,
  width_mm: 80,
  dpi: 180,
  code_pages: 11,
  manuals: ["https://files.support.epson.com/pdf/pos/bulk/tm-t88iv_eng_trg_revc_02.pdf"],
  features: ["qrCode", "paperFullCut"],
};
const trace = [
  { index: 0, start: 0, end: 2, event: { type: "initialize" }, state: snapshot },
  {
    index: 1,
    start: 2,
    end: 15,
    event: { type: "text", value: { align: "center", style: snapshot.style, text: "Green Market" } },
    state: snapshot,
  },
  { index: 2, start: 15, end: 16, event: { type: "line_feed" }, state: snapshot },
];

function serve(...ids: number[]) {
  answers["/api/receipts"] = ids.map(receipt);
  answers["/api/printers"] = [printer];
  answers["/api/profiles"] = [model];
  for (const id of ids) {
    answers[`/api/receipts/${id}/trace`] = trace;
    answers[`/api/receipts/${id}/bytes`] = Uint8Array.from([0x1b, 0x40, ...new Array(14).fill(0x47)]);
    answers[`/api/receipts/${id}/text`] = "Green Market\nTotal 4.20";
  }
}

async function show(picked?: number, onPick: (id?: number) => void = () => {}) {
  const { LivePage } = await import("./live");
  render(
    <QueryClientProvider client={new QueryClient()}>
      <LivePage picked={picked} onPick={onPick} />
    </QueryClientProvider>,
  );
}

const picture = async () => (await screen.findByRole("img")).getAttribute("src");

test("the newest receipt is shown as the picture the server draws", async () => {
  serve(6, 7);
  await show();
  expect(await picture()).toBe("/api/receipts/7/picture");
});

test("a picked receipt is shown instead of the newest", async () => {
  serve(6, 7);
  await show(6);
  expect(await picture()).toBe("/api/receipts/6/picture");
});

test("the text tab shows the paper as text", async () => {
  serve(7);
  await show();
  fireEvent.click(await screen.findByRole("tab", { name: "Text" }));
  expect(await screen.findByText(/Total 4\.20/)).toBeTruthy();
});

test("the bytes tab shows the bytes the printer was sent", async () => {
  serve(7);
  await show();
  fireEvent.click(await screen.findByRole("tab", { name: "Bytes" }));
  expect(await screen.findByText(/1b 40 47/)).toBeTruthy();
});

test("the commands are listed in words and the paper is as it was after the one clicked", async () => {
  serve(7);
  await show();
  expect(await screen.findByText("Command 3 of 3")).toBeTruthy();
  fireEvent.click(await screen.findByText('"Green Market"'));
  expect(await screen.findByText("Command 2 of 3")).toBeTruthy();
  expect(await picture()).toBe("/api/receipts/7/picture?upto=2");
  expect(await screen.findByText(/offset 0x0002, 13 bytes/)).toBeTruthy();
});

test("the arrows step through the commands", async () => {
  serve(7);
  await show();
  await screen.findByText("Command 3 of 3");
  fireEvent.click(screen.getByRole("button", { name: "Previous command" }));
  expect(await screen.findByText("Command 2 of 3")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Next command" }));
  expect(await screen.findByText("Command 3 of 3")).toBeTruthy();
  expect(await picture()).toBe("/api/receipts/7/picture");
});

test("the state tab says what the printer believed after the command", async () => {
  serve(7);
  await show();
  await screen.findByText("Command 3 of 3");
  fireEvent.click(screen.getByRole("tab", { name: "State" }));
  expect(await screen.findByText("Double strike")).toBeTruthy();
});

test("the filter keeps the commands that match", async () => {
  serve(7);
  await show();
  expect(await screen.findByText("Initialize")).toBeTruthy();
  fireEvent.change(screen.getByLabelText("Filter commands"), { target: { value: "market" } });
  expect(screen.queryByText("Initialize")).toBeNull();
  expect(screen.getByText('"Green Market"')).toBeTruthy();
});

test("without receipts it says where to print", async () => {
  answers["/api/receipts"] = [];
  answers["/api/printers"] = [printer];
  await show();
  expect(await screen.findByText(/127\.0\.0\.1:9100/)).toBeTruthy();
});

test("the receipts are listed newest first and a click picks one", async () => {
  serve(5, 6, 7);
  const onPick = vi.fn();
  await show(undefined, onPick);
  const ids = (await screen.findAllByRole("button", { name: /^#\d/ })).map((button) => button.textContent?.slice(0, 2));
  expect(ids).toEqual(["#7", "#6", "#5"]);
  fireEvent.click(screen.getByRole("button", { name: /^#6/ }));
  expect(onPick).toHaveBeenCalledWith(6);
});

test("the list follows the newest until one is picked, and the switch goes back to following", async () => {
  serve(6, 7);
  const onPick = vi.fn();
  await show(undefined, onPick);
  const follow = await screen.findByRole("switch", { name: "Follow the newest receipt" });
  expect(follow.getAttribute("aria-checked")).toBe("true");
  fireEvent.click(follow);
  expect(onPick).toHaveBeenLastCalledWith(7);
});

test("a receipt that was picked is not followed and the switch follows again", async () => {
  serve(6, 7);
  const onPick = vi.fn();
  await show(6, onPick);
  const follow = await screen.findByRole("switch", { name: "Follow the newest receipt" });
  expect(follow.getAttribute("aria-checked")).toBe("false");
  fireEvent.click(follow);
  expect(onPick).toHaveBeenLastCalledWith(undefined);
});

test("the search keeps the receipts that printed the text", async () => {
  serve(6, 7);
  await show();
  fireEvent.change(await screen.findByLabelText("Search printed text"), { target: { value: "no such text" } });
  expect(await screen.findByText("No receipt has this text.")).toBeTruthy();
});

test("the printer model of the receipt is named and its details open", async () => {
  serve(7);
  await show();
  expect(await screen.findByText("Epson, 80 mm, 512 dots, 42 columns")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Details" }));
  expect(await screen.findByText("80 mm, 512 dots at 180 dpi")).toBeTruthy();
  expect(screen.getByText("42 in font A, 56 in font B")).toBeTruthy();
  expect(screen.getByText("QR code")).toBeTruthy();
  expect(screen.getByRole("link", { name: /files\.support\.epson\.com/ })).toBeTruthy();
});
