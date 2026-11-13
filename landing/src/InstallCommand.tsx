import { CheckIcon, CopyIcon } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

/** Copies `text`, and says so for a moment. The text stays on the page to select if the browser refuses. */
function useCopy(text: string) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Not allowed here.
    }
  };
  return { copied, copy };
}

const COMMANDS = [
  { id: "unix", label: "macOS and Linux", command: "curl -fsSL https://stubprint.dev/install.sh | sh" },
  { id: "windows", label: "Windows", command: "irm https://stubprint.dev/install.ps1 | iex" },
] as const;

function Line({ command, label }: { command: string; label: string }) {
  const { copied, copy } = useCopy(command);
  return (
    <div className="flex items-center border bg-card">
      <pre className="min-w-0 flex-1 overflow-x-auto px-4 py-3.5 text-left font-mono text-sm whitespace-pre-wrap break-all sm:whitespace-pre sm:break-normal">
        <span className="text-muted-foreground select-none">$ </span>
        <code>{command}</code>
      </pre>
      <Button variant="ghost" size="icon-lg" className="m-1.5 shrink-0" onClick={copy} aria-label={`Copy the install command for ${label}`}>
        {copied ? <CheckIcon /> : <CopyIcon />}
      </Button>
    </div>
  );
}

/** The install command, one tab for each way to get it. */
export function InstallCommand() {
  return (
    <Tabs defaultValue="unix" className="w-full max-w-xl gap-2">
      <TabsList variant="line" className="justify-start">
        {COMMANDS.map(({ id, label }) => (
          <TabsTrigger key={id} value={id}>
            {label}
          </TabsTrigger>
        ))}
      </TabsList>
      {COMMANDS.map(({ id, label, command }) => (
        <TabsContent key={id} value={id}>
          <Line command={command} label={label} />
        </TabsContent>
      ))}
    </Tabs>
  );
}
