import { type ReactNode, useState } from "react";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";

/** A few settings one under the other, with a line between them. */
export function SettingRows({ children }: { children: ReactNode }) {
  return <div className="flex flex-col gap-3 [&>*:not(:first-child)]:border-t [&>*:not(:first-child)]:pt-3">{children}</div>;
}

/**
 * A setting that is a number and is off at 0: what it is, the number with its unit, and a switch. A
 * number typed while it is on applies when the field is left, and the switch turns it on with the
 * number typed or the one it starts with. `parse` says what a typed text means, and `format` how a
 * value is typed, for a setting that is shown in another unit than it is kept in.
 */
export function NumberSetting({
  label,
  description,
  unit,
  value,
  fallback,
  parse,
  format = String,
  error,
  inputLabel = `${label} in ${unit}`,
  switchLabel = label,
  apply,
}: {
  label: string;
  description: string;
  unit: string;
  value: number;
  fallback: number;
  parse: (text: string) => number | undefined;
  format?: (value: number) => string;
  /** What is said when the typed text is not a number the setting takes. */
  error: string;
  inputLabel?: string;
  switchLabel?: string;
  apply: (value: number) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [shown, setShown] = useState(value);
  const [text, setText] = useState(format(value || fallback));
  // The field shows its own typing, not the value: it only takes a new value when the value changes,
  // and not while it is being typed in.
  if (value !== shown) {
    setShown(value);
    if (!editing) setText(format(value || fallback));
  }
  const typed = parse(text);
  const on = value > 0;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-3">
        <div className="flex flex-1 flex-col">
          <span className="font-medium">{label}</span>
          <span className="text-sm text-muted-foreground">{description}</span>
        </div>
        <Input
          aria-label={inputLabel}
          aria-invalid={typed === undefined}
          inputMode="decimal"
          className="h-8 w-24 text-right"
          value={text}
          onChange={(event) => setText(event.target.value)}
          onFocus={() => setEditing(true)}
          onBlur={() => {
            setEditing(false);
            if (on && typed !== undefined && typed !== value) apply(typed);
          }}
        />
        <span className="w-14 text-sm text-muted-foreground">{unit}</span>
        <Switch aria-label={switchLabel} checked={on} disabled={!on && typed === undefined} onCheckedChange={(next) => apply(next ? (typed ?? fallback) : 0)} />
      </div>
      {typed === undefined && <p className="text-sm text-destructive">{error}</p>}
    </div>
  );
}
