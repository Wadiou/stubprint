import { Link, Outlet, useMatches } from "@tanstack/react-router";
import {
  Activity,
  LayoutGrid,
  type LucideIcon,
  Plug,
  Printer,
  Receipt,
  Settings,
  Terminal,
} from "lucide-react";
import { useLiveEvents } from "@/api/events";
import { Badge } from "@/components/ui/badge";

const GROUPS: { label: string; items: { to: string; label: string; icon: LucideIcon }[] }[] = [
  {
    label: "Watch",
    items: [
      { to: "/", label: "Live paper", icon: Activity },
      { to: "/receipts", label: "Receipts", icon: Receipt },
      { to: "/connections", label: "Connections", icon: Plug },
    ],
  },
  {
    label: "Control",
    items: [
      { to: "/printers", label: "Printers", icon: Printer },
      { to: "/samples", label: "Samples", icon: LayoutGrid },
      { to: "/playground", label: "Send and decode", icon: Terminal },
    ],
  },
  { label: "Connect", items: [{ to: "/settings", label: "Settings", icon: Settings }] },
];

export function Shell() {
  const live = useLiveEvents();
  const title = useMatches({ select: (matches) => matches.at(-1)?.staticData.title });
  return (
    <div className="flex min-h-screen bg-background text-foreground">
      <aside className="flex w-58 shrink-0 flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground">
        <div className="flex items-center gap-2 p-4 font-semibold">
          <Receipt className="size-5" />
          Stubprint
        </div>
        <nav className="flex flex-col gap-1 px-2">
          {GROUPS.map((group) => (
            <div key={group.label} className="flex flex-col gap-1">
              <div className="px-2 pt-4 pb-1 text-xs text-muted-foreground">{group.label}</div>
              {group.items.map(({ to, label, icon: Icon }) => (
                <Link
                  key={to}
                  to={to}
                  activeOptions={{ exact: true }}
                  className="flex h-8 items-center gap-2 rounded-md px-2 text-sm hover:bg-sidebar-accent data-[status=active]:bg-sidebar-accent data-[status=active]:font-medium"
                >
                  <Icon className="size-4" />
                  {label}
                </Link>
              ))}
            </div>
          ))}
        </nav>
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 items-center gap-3 border-b px-5">
          <h1 className="text-base font-semibold">{title}</h1>
          <div className="flex-1" />
          <Badge variant="outline">{live ? "Live" : "Offline"}</Badge>
        </header>
        <main className="flex-1 p-6">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
