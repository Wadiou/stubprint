import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

// The API client keeps the fetch it finds when it is made, so the answers are set before it loads.
const answers = vi.hoisted(() => {
  const answers: Record<string, unknown> = {};
  globalThis.fetch = async (input: RequestInfo | URL) => {
    const url = new URL(input instanceof Request ? input.url : String(input), "http://stubprint");
    return Response.json(answers[url.pathname] ?? []);
  };
  return answers;
});

const printer = { name: "front", dialect: "escpos", listen: "127.0.0.1:9100" };

async function show() {
  const { LivePage } = await import("./live");
  render(
    <QueryClientProvider client={new QueryClient()}>
      <LivePage />
    </QueryClientProvider>,
  );
}

test("the newest receipt is shown as the picture the server draws", async () => {
  answers["/api/receipts"] = [
    { id: 6, printer: "front", dialect: "escpos", at: 0, bytes: 90, events: 5, preview: [] },
    { id: 7, printer: "front", dialect: "escpos", at: 0, bytes: 120, events: 9, preview: [] },
  ];
  answers["/api/printers"] = [printer];
  await show();
  const picture = await screen.findByRole("img");
  expect(picture.getAttribute("src")).toBe("/api/receipts/7.png");
});

test("without receipts it says where to print", async () => {
  answers["/api/receipts"] = [];
  answers["/api/printers"] = [printer];
  await show();
  expect(await screen.findByText(/127\.0\.0\.1:9100/)).toBeTruthy();
});
