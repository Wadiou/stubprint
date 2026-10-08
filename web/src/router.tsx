import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { Shell } from "@/components/shell";
import { LivePage } from "@/pages/live";
import { Placeholder } from "@/pages/placeholder";
import { PrintersPage } from "@/components/printers/printers-page";
import { SamplesPage } from "@/components/samples/samples-page";

const rootRoute = createRootRoute({ component: Shell });

const liveRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  staticData: { title: "Live paper" },
  validateSearch: (search: Record<string, unknown>): { receipt?: number } =>
    typeof search.receipt === "number" ? { receipt: search.receipt } : {},
  component: LiveRoute,
});

function LiveRoute() {
  const { receipt } = liveRoute.useSearch();
  const navigate = liveRoute.useNavigate();
  return <LivePage picked={receipt} onPick={(id) => navigate({ search: id === undefined ? {} : { receipt: id } })} />;
}

const samplesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/samples",
  staticData: { title: "Samples" },
  component: SamplesRoute,
});

function SamplesRoute() {
  const navigate = samplesRoute.useNavigate();
  return <SamplesPage onOpen={(receipt) => navigate({ to: "/", search: { receipt } })} />;
}

const printersRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/printers",
  staticData: { title: "Printers" },
  validateSearch: (search: Record<string, unknown>): { add?: boolean; remove?: string } => ({
    ...(search.add === true || search.add === "true" ? { add: true } : {}),
    ...(typeof search.remove === "string" ? { remove: search.remove } : {}),
  }),
  component: PrintersRoute,
});

function PrintersRoute() {
  const search = printersRoute.useSearch();
  const navigate = printersRoute.useNavigate();
  return <PrintersPage search={search} onSearchChange={(next) => navigate({ search: next })} />;
}

const unbuilt = [
  { path: "/receipts", title: "Receipts" },
  { path: "/connections", title: "Connections" },
  { path: "/playground", title: "Send and decode" },
  { path: "/settings", title: "Settings" },
].map(({ path, title }) =>
  createRoute({ getParentRoute: () => rootRoute, path, staticData: { title }, component: Placeholder }),
);

export const router = createRouter({ routeTree: rootRoute.addChildren([liveRoute, samplesRoute, printersRoute, ...unbuilt]) });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
  interface StaticDataRouteOption {
    title?: string;
  }
}
