import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { Shell } from "@/components/shell";
import { liveSearch, printersSearch } from "@/lib/search";
import { LivePage } from "@/pages/live";
import { PrintersPage } from "@/components/printers/printers-page";
import { SamplesPage } from "@/components/samples/samples-page";

const rootRoute = createRootRoute({ component: Shell });

const liveRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  staticData: { title: "Live paper" },
  validateSearch: (search: Record<string, unknown>) => liveSearch.parse(search),
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
  return <SamplesPage />;
}

const printersRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/printers",
  staticData: { title: "Printers" },
  validateSearch: (search: Record<string, unknown>) => printersSearch.parse(search),
  component: PrintersRoute,
});

function PrintersRoute() {
  const search = printersRoute.useSearch();
  const navigate = printersRoute.useNavigate();
  return <PrintersPage search={search} onSearchChange={(next) => navigate({ search: next })} />;
}

export const router = createRouter({
  routeTree: rootRoute.addChildren([liveRoute, printersRoute, samplesRoute]),
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
  interface StaticDataRouteOption {
    title?: string;
  }
}
