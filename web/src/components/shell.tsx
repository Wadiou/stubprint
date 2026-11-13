import { Link, Outlet, useMatches } from "@tanstack/react-router";
import { useEffect } from "react";
import { useLiveEvents } from "@/api/events";
import { Nav } from "@/components/nav";
import { PrintersStatus } from "@/components/printers-status";
import { Toaster } from "@/components/ui/sonner";
import { useServer } from "@/lib/use-server";

export function Shell() {
  useLiveEvents();
  const version = useServer().data?.version;
  const title = useMatches({ select: (matches) => matches.at(-1)?.staticData.title });
  useEffect(() => {
    document.title = title ? `${title} · Stubprint` : "Stubprint";
  }, [title]);
  return (
    <>
      <div className="flex h-dvh flex-col overflow-hidden bg-background text-foreground">
        <header className="flex h-14 shrink-0 items-center gap-4 border-b bg-background px-4 md:grid md:grid-cols-[1fr_auto_1fr] md:items-stretch">
          <Link to="/" className="justify-self-start leading-tight md:self-center">
            <div className="font-semibold">Stubprint</div>
            {version && <div className="text-xs text-muted-foreground">v{version}</div>}
          </Link>
          <Nav />
          <div className="ms-auto md:ms-0 md:self-center md:justify-self-end">
            <PrintersStatus />
          </div>
        </header>
        {/* The one place the page scrolls, so a short page fills the screen and a tall one scrolls here
            and nowhere else; most panels inside cap their own height and scroll on their own instead. */}
        <main className="flex min-h-0 flex-1 flex-col overflow-y-auto p-6 pb-24 md:pb-6">
          <Outlet />
        </main>
      </div>
      <Toaster position="top-center" />
    </>
  );
}
