import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { Shell } from "@/components/shell";
import { LivePage } from "@/pages/live";
import { Placeholder } from "@/pages/placeholder";

const rootRoute = createRootRoute({ component: Shell });

const pages = [
  { path: "/", component: LivePage },
  { path: "/receipts", component: () => <Placeholder title="Receipts" /> },
  { path: "/connections", component: () => <Placeholder title="Connections" /> },
  { path: "/printers", component: () => <Placeholder title="Printers" /> },
  { path: "/samples", component: () => <Placeholder title="Samples" /> },
  { path: "/playground", component: () => <Placeholder title="Send and decode" /> },
  { path: "/settings", component: () => <Placeholder title="Settings" /> },
];

export const router = createRouter({
  routeTree: rootRoute.addChildren(pages.map((page) => createRoute({ getParentRoute: () => rootRoute, ...page }))),
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
