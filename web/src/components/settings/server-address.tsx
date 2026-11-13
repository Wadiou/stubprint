import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { toast } from "sonner";
import { getServerQueryKey, updateServerMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { ServerInfo } from "@/api/generated/types.gen";
import { refused } from "@/components/printers/printer-fields";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

/** The address the server listens on, which is saved and used from the next start. */
export function ServerAddress({ server }: { server: ServerInfo }) {
  const queryClient = useQueryClient();
  const saved = server.next_http ?? server.http;
  const [text, setText] = useState(saved);
  const keeps = server.kept;
  const update = useMutation({
    ...updateServerMutation(),
    onSuccess: () => {
      toast.success("Saved", { description: "The server listens on it after the next start." });
      return queryClient.invalidateQueries({ queryKey: getServerQueryKey() });
    },
  });
  const changed = text.trim() !== saved;

  return (
    <form
      className="flex flex-col gap-2 py-2.5"
      onSubmit={(event) => {
        event.preventDefault();
        if (changed) update.mutate({ body: { http: text.trim() } });
      }}
    >
      <label htmlFor="http-address" className="text-sm font-medium">
        HTTP address
      </label>
      <div className="flex gap-2">
        <Input
          id="http-address"
          value={text}
          disabled={!keeps}
          onChange={(event) => setText(event.target.value)}
          className="max-w-sm font-mono"
          spellCheck={false}
        />
        <Button type="submit" disabled={!keeps || !changed || update.isPending}>
          Save
        </Button>
      </div>
      <p className="text-sm text-muted-foreground">
        {keeps
          ? "Where the API and this page listen. It is used from the next start."
          : "Nothing is kept, so a new address cannot be saved."}
      </p>
      {server.next_http && (
        <p className="text-sm text-muted-foreground">
          Listening on <span className="font-mono">{server.http}</span> now, and on <span className="font-mono">{server.next_http}</span> after a
          restart.
        </p>
      )}
      {update.isError && <p className="text-sm text-destructive">{refused(update.error)}</p>}
    </form>
  );
}
