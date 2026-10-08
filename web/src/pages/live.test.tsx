import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

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

const printer = { name: "front", dialect: "escpos", listen: "127.0.0.1:9100" };
const receipt = (id: number) => ({ id, printer: "front", dialect: "escpos", at: 0, bytes: 90, events: 5, preview: [] });

async function show(picked?: number, onPick = () => {}) {
  const { LivePage } = await import("./live");
  render(
    <QueryClientProvider client={new QueryClient()}>
      <LivePage picked={picked} onPick={onPick} />
    </QueryClientProvider>,
  );
}

test("the newest receipt is shown as the picture the server draws", async () => {
  answers["/api/receipts"] = [receipt(6), receipt(7)];
  answers["/api/printers"] = [printer];
  await show();
  const picture = await screen.findByRole("img");
  expect(picture.getAttribute("src")).toBe("/api/receipts/7/picture");
});

test("a picked receipt is shown instead of the newest", async () => {
  answers["/api/receipts"] = [receipt(6), receipt(7)];
  answers["/api/printers"] = [printer];
  await show(6);
  const picture = await screen.findByRole("img");
  expect(picture.getAttribute("src")).toBe("/api/receipts/6/picture");
});

test("the text tab shows the paper as text", async () => {
  answers["/api/receipts"] = [receipt(7)];
  answers["/api/printers"] = [printer];
  answers["/api/receipts/7/text"] = "Green Market\nTotal 4.20";
  await show();
  fireEvent.click(await screen.findByRole("tab", { name: "Text" }));
  expect(await screen.findByText(/Green Market/)).toBeTruthy();
});

test("the bytes tab shows the bytes the printer was sent", async () => {
  answers["/api/receipts"] = [receipt(7)];
  answers["/api/printers"] = [printer];
  answers["/api/receipts/7/bytes"] = Uint8Array.from([0x1b, 0x40, 0x48, 0x69]);
  await show();
  fireEvent.click(await screen.findByRole("tab", { name: "Bytes" }));
  expect(await screen.findByText(/1b 40 48 69/)).toBeTruthy();
});

test("without receipts it says where to print", async () => {
  answers["/api/receipts"] = [];
  answers["/api/printers"] = [printer];
  await show();
  expect(await screen.findByText(/127\.0\.0\.1:9100/)).toBeTruthy();
});
