import { PlusIcon } from "lucide-react";
import type { ReactNode } from "react";
import { InstallCommand } from "./InstallCommand.tsx";
import { Receipt } from "./Receipt.tsx";
import { HEADLINE, NAME, REPO, TAGLINE } from "./brand.ts";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";

const DOCS = `${REPO}/tree/main/docs`;

/** Turns the `**word**` marks of a brand.ts string into highlighted text. */
function highlight(text: string) {
  return text.split(/(\*\*.+?\*\*)/g).map((part, index) =>
    part.startsWith("**") ? (
      <strong key={index} className="font-semibold text-foreground">
        {part.slice(2, -2)}
      </strong>
    ) : (
      part
    ),
  );
}

const FEATURES = [
  { title: "Reads real printer bytes", body: "ESC/POS, Star Line and StarPRNT, with 50 printer models." },
  { title: "Shows what came out", body: "The paper as a picture, as text, or command by command." },
  { title: "Prints like a printer", body: "Receipts print line by line, and stop on an open cover or no paper." },
  { title: "Limits like a printer", body: "Buffer, paper roll and connection limits, off by default." },
  { title: "An API for tests and agents", body: "REST, OpenAPI and live events. One call waits for the next receipt." },
  { title: "A command line", body: "print, wait and decode, with exit codes for CI." },
  { title: "Samples to try", body: "Receipts from python-escpos and node-thermal-printer." },
  { title: "One program", body: "One binary, with its web page inside." },
];

/** The column of the page: its two vertical lines run from the top to the bottom, and each section's line crosses them. */
function Frame({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cn("relative mx-auto w-full max-w-5xl border-x", className)}>{children}</div>;
}

/** A mark where a line of a section meets the two lines of the column. */
function Crosses() {
  const mark = "absolute z-10 size-[17px] bg-background text-muted-foreground";
  return (
    <>
      <PlusIcon aria-hidden="true" strokeWidth={1} className={cn(mark, "-bottom-[9px] -left-[9px]")} />
      <PlusIcon aria-hidden="true" strokeWidth={1} className={cn(mark, "-right-[9px] -bottom-[9px]")} />
    </>
  );
}

/** The mark of GitHub: lucide has no brand icons. */
function GithubMark() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" className="size-6 fill-current">
      <path d="M12 .5C5.65.5.5 5.65.5 12c0 5.08 3.29 9.39 7.86 10.91.58.1.79-.25.79-.56v-2c-3.2.7-3.87-1.37-3.87-1.37-.52-1.33-1.28-1.69-1.28-1.69-1.04-.71.08-.7.08-.7 1.15.08 1.76 1.18 1.76 1.18 1.03 1.76 2.69 1.25 3.35.96.1-.74.4-1.25.73-1.54-2.55-.29-5.24-1.28-5.24-5.69 0-1.26.45-2.28 1.18-3.09-.12-.29-.51-1.46.11-3.05 0 0 .97-.31 3.17 1.18a11 11 0 0 1 5.77 0c2.2-1.49 3.17-1.18 3.17-1.18.62 1.59.23 2.76.11 3.05.74.81 1.18 1.83 1.18 3.09 0 4.42-2.7 5.4-5.26 5.68.41.36.78 1.06.78 2.14v3.17c0 .31.21.67.8.56A11.51 11.51 0 0 0 23.5 12C23.5 5.65 18.35.5 12 .5Z" />
    </svg>
  );
}

export function App() {
  return (
    <>
      <header className="border-b">
        <Frame className="flex h-16 items-center justify-between px-6">
          <a href="/" className="text-xl font-semibold tracking-tight">
            {NAME}
          </a>
          <a
            href={REPO}
            aria-label="Stubprint on GitHub"
            className="text-muted-foreground transition-colors hover:text-foreground"
          >
            <GithubMark />
          </a>
          <Crosses />
        </Frame>
      </header>

      <main>
        <section className="border-b">
          <Frame className="flex flex-col items-center gap-6 px-6 pt-20 pb-24 text-center">
            <Badge variant="outline">{`v${__VERSION__}`}</Badge>
            <h1 className="max-w-3xl text-4xl font-semibold tracking-tight text-balance sm:text-6xl sm:leading-[1.05]">{HEADLINE}</h1>
            <p className="max-w-xl text-lg text-pretty text-muted-foreground">{highlight(TAGLINE)}</p>
            <InstallCommand />

            <div className="mt-12 flex w-full flex-col items-center gap-3">
              <div className="w-full max-w-sm border bg-card px-4 py-3.5 text-left font-mono text-[13px] leading-relaxed">
                <span className="text-muted-foreground select-none">$ </span>stubprint print --sample cafe
              </div>
              <div className="text-[13px] text-muted-foreground" aria-hidden="true">
                ↓ what came out
              </div>
              <Receipt />
            </div>
            <Crosses />
          </Frame>
        </section>

        <section className="border-b">
          <Frame>
            <h2 className="px-6 py-16 text-center text-3xl font-semibold tracking-tight text-balance">It behaves like the printers it copies</h2>
            <ul className="grid gap-px border-t bg-border sm:grid-cols-2 lg:grid-cols-4">
              {FEATURES.map((feature) => (
                <li key={feature.title} className="bg-background">
                  <Card className="h-full bg-transparent ring-0">
                    <CardHeader>
                      <CardTitle>
                        <h3>{feature.title}</h3>
                      </CardTitle>
                    </CardHeader>
                    <CardContent className="text-muted-foreground">{feature.body}</CardContent>
                  </Card>
                </li>
              ))}
            </ul>
            <Crosses />
          </Frame>
        </section>

      </main>

      <footer className="text-sm text-muted-foreground">
        <Frame className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 px-6 py-8">
          <span>
            {NAME} {__VERSION__}. MIT licence.
          </span>
          <nav aria-label="Footer" className="flex gap-6">
            <a href={REPO} className="hover:text-foreground">
              GitHub
            </a>
            <a href={DOCS} className="hover:text-foreground">
              Docs
            </a>
            <a href={`${REPO}/releases`} className="hover:text-foreground">
              Releases
            </a>
          </nav>
        </Frame>
      </footer>
    </>
  );
}
