import { Link } from "@tanstack/react-router";
import { Activity, LayoutGrid, type LucideIcon, Printer, Settings } from "lucide-react";

const ITEMS: { to: string; label: string; icon: LucideIcon }[] = [
  { to: "/", label: "Live paper", icon: Activity },
  { to: "/printers", label: "Printers", icon: Printer },
  { to: "/samples", label: "Samples", icon: LayoutGrid },
  { to: "/settings", label: "Settings", icon: Settings },
];

/**
 * The pages, in the middle of the header and as tall as it is, so they touch its edges, with a gap
 * between them. The page that is open has a line under its name, which lies over the bottom line of
 * the header, and the line of another one is drawn from left to right while the pointer is on it. On a
 * narrow screen it is a bar at the bottom, where a thumb reaches it, with the line above the name.
 */
export function Nav() {
  return (
    <nav
      aria-label="Pages"
      className="fixed inset-x-0 bottom-0 z-40 flex items-stretch border-t bg-card md:relative md:z-auto md:-mb-px md:gap-2 md:border-0 md:bg-transparent"
    >
      {ITEMS.map(({ to, label, icon: Icon }) => (
        <Link
          key={to}
          to={to}
          activeOptions={{ exact: true }}
          className="relative flex h-14 min-w-0 flex-1 flex-col items-center justify-center gap-1 px-1 text-xs font-medium text-muted-foreground after:absolute after:inset-x-0 after:top-0 after:h-0.5 after:origin-left after:scale-x-0 after:bg-muted-foreground after:transition-transform after:duration-300 after:ease-out hover:text-foreground/75 data-[status=active]:text-foreground data-[status=active]:hover:text-foreground data-[status=active]:after:scale-x-100 data-[status=active]:after:bg-primary motion-reduce:after:transition-none md:h-full md:flex-none md:flex-row md:gap-2 md:px-4 md:text-sm md:after:top-auto md:after:bottom-0 md:hover:after:scale-x-100"
        >
          <Icon className="size-5 md:size-4" />
          <span className="max-w-full truncate">{label}</span>
        </Link>
      ))}
    </nav>
  );
}
