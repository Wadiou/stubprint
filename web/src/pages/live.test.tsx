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
const receipt = (id: number) => ({ id, printer: "front", dialect: "escpos", at: 0, bytes: 16, events: 3, preview: [] });
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
  for (const id of ids) {
    answers[`/api/receipts/${id}/trace`] = trace;
    answers[`/api/receipts/${id}/bytes`] = Uint8Array.from([0x1b, 0x40, ...new Array(14).fill(0x47)]);
    answers[`/api/receipts/${id}/text`] = "Green Market\nTotal 4.20";
  }
}

async function show(picked?: number, onPick = () => {}) {
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
