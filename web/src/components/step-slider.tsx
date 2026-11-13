import { Slider } from "@/components/ui/slider";

/** Steps through the commands of a receipt, to see the paper as it was after each. */
export function StepSlider({ value, total, onChange }: { value: number; total: number; onChange: (step: number) => void }) {
  return (
    <div className="flex items-center gap-3">
      <Slider aria-label="Command" min={0} max={Math.max(total, 1)} value={value} onValueChange={(next) => onChange(Array.isArray(next) ? next[0] : next)} />
      <span className="shrink-0 text-sm text-muted-foreground">
        Command {value} of {total}
      </span>
    </div>
  );
}
