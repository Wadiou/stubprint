import { useQuery } from "@tanstack/react-query";
import { ChevronDown, Printer } from "lucide-react";
import { listProfilesOptions } from "@/api/generated/@tanstack/react-query.gen";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { columnsText, featureLabels, paperText } from "@/lib/profile";

/** A printer model, as a receipt was printed on it or a sample was written for it, and what the database says about it. */
export function ModelStrip({ profile }: { profile: string }) {
  const profiles = useQuery({ ...listProfilesOptions(), staleTime: Number.POSITIVE_INFINITY });
  const model = profiles.data?.find((item) => item.name === profile);
  if (!model) return null;

  const about = [
    model.vendor,
    model.width_mm ? `${model.width_mm} mm` : undefined,
    model.width_dots ? `${model.width_dots} dots` : undefined,
    model.columns[0] ? `${model.columns[0]} columns` : undefined,
  ].filter(Boolean);

  return (
    <div className="flex flex-wrap items-center gap-3 rounded-lg border px-3 py-2">
      <Printer className="size-4 shrink-0 text-muted-foreground" />
      <div className="min-w-0 flex-1">
        <div className="font-medium">{model.name}</div>
        <div className="text-sm text-muted-foreground">{about.join(", ")}</div>
      </div>
      <Popover>
        <PopoverTrigger render={<Button variant="outline" size="sm" />}>
          Details
          <ChevronDown />
        </PopoverTrigger>
        <PopoverContent aria-label="Printer model">
          <div className="flex items-center gap-2">
            <span className="text-base font-semibold">{model.name}</span>
            {model.vendor && <Badge variant="outline">{model.vendor}</Badge>}
          </div>
          <dl className="mt-3 grid grid-cols-[6.5rem_1fr] gap-x-3 gap-y-2 text-sm">
            {paperText(model) && (
              <>
                <dt className="text-muted-foreground">Paper</dt>
                <dd>{paperText(model)}</dd>
              </>
            )}
            {columnsText(model.columns) && (
              <>
                <dt className="text-muted-foreground">Columns</dt>
                <dd>{columnsText(model.columns)}</dd>
              </>
            )}
            <dt className="text-muted-foreground">Code pages</dt>
            <dd>{model.code_pages}</dd>
          </dl>
          <div className="mt-3 flex flex-wrap gap-1.5">
            {featureLabels(model.features).map((label) => (
              <Badge key={label} variant="outline">
                {label}
              </Badge>
            ))}
          </div>
        </PopoverContent>
      </Popover>
    </div>
  );
}
