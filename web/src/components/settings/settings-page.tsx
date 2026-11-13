import { PageLayout } from "@/components/layout/page";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useServer } from "@/lib/use-server";
import { ServerAddress } from "./server-address";

/** What the API serves, as the path to open and what it is. */
const API_PATHS = [
  { label: "OpenAPI description", path: "/api/openapi.json" },
  { label: "Entry point", path: "/api" },
  { label: "Live events", path: "/api/events" },
];

/** How the server runs and where it listens, and where its API is. */
export function SettingsPage() {
  const server = useServer();

  return (
    <PageLayout width="narrow">
      <Card>
        <CardHeader>
          <CardTitle>Server</CardTitle>
          <CardDescription>Where the server listens.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col [&>*:not(:first-child)]:border-t">
          {server.isError && (
            <p className="py-2.5 text-sm text-muted-foreground">
              The server did not say how it was started. It may be an older version: start the latest build.
            </p>
          )}
          {server.data && <ServerAddress key={`${server.data.http}-${server.data.next_http}`} server={server.data} />}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>API</CardTitle>
          <CardDescription>Everything on this page is also available over HTTP.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col [&>*:not(:first-child)]:border-t">
          {API_PATHS.map(({ label, path }) => (
            <div key={path} className="flex items-center justify-between gap-4 py-2.5 text-sm">
              <span>{label}</span>
              <a href={path} target="_blank" rel="noreferrer" className="font-mono text-sm underline-offset-4 hover:underline">
                {path}
              </a>
            </div>
          ))}
          <div className="flex items-center justify-between gap-4 py-2.5 text-sm">
            <span>Server and how it was started</span>
            <a href="/api/server" target="_blank" rel="noreferrer" className="font-mono text-sm underline-offset-4 hover:underline">
              /api/server
            </a>
          </div>
        </CardContent>
      </Card>
    </PageLayout>
  );
}
