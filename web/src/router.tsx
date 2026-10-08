import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { Shell } from "@/components/shell";
import { LivePage } from "@/pages/live";
import { Placeholder } from "@/pages/placeholder";

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

const unbuilt = [
  { path: "/receipts", title: "Receipts" },
  { path: "/connections", title: "Connections" },
  { path: "/printers", title: "Printers" },
  { path: "/samples", title: "Samples" },
  { path: "/playground", title: "Send and decode" },
  { path: "/settings", title: "Settings" },
].map(({ path, title }) =>
  createRoute({ getParentRoute: () => rootRoute, path, staticData: { title }, component: Placeholder }),
);

export const router = createRouter({ routeTree: rootRoute.addChildren([liveRoute, ...unbuilt]) });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
  interface StaticDataRouteOption {
    title?: string;
  }
}
