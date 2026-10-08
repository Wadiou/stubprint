import { Pause, Play } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";

/** How fast it plays, as how many times the pace of one command in 350 ms. */
const SPEEDS = [0.5, 1, 2, 4, 10].map((times) => ({ value: String(times), label: `${times}x`, ms: 350 / times }));

/** Steps through the commands of a receipt. It can play them one after the other, as the paper was built. */
export function StepSlider({ value, total, onChange }: { value: number; total: number; onChange: (step: number) => void }) {
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState("1");

  useEffect(() => {
    if (!playing) return;
    if (value >= total) return setPlaying(false);
    const next = setTimeout(() => onChange(value + 1), SPEEDS.find((item) => item.value === speed)?.ms);
    return () => clearTimeout(next);
  }, [playing, speed, value, total, onChange]);

  const toggle = () => {
    if (!playing && value >= total) onChange(0);
    setPlaying(!playing);
  };

  return (
    <div className="flex items-center gap-3">
      <Button variant="outline" size="icon-sm" aria-label={playing ? "Pause" : "Play"} onClick={toggle}>
        {playing ? <Pause /> : <Play />}
      </Button>
      <Slider
        aria-label="Command"
        min={0}
        max={Math.max(total, 1)}
        value={value}
        onValueChange={(next) => {
          setPlaying(false);
          onChange(Array.isArray(next) ? next[0] : next);
        }}
      />
      <span className="shrink-0 text-sm text-muted-foreground">
        Command {value} of {total}
      </span>
      <Select items={SPEEDS} value={speed} onValueChange={(next) => next && setSpeed(next)}>
        <SelectTrigger size="sm" aria-label="Speed" className="w-20 shrink-0">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {SPEEDS.map(({ value: item, label }) => (
            <SelectItem key={item} value={item}>
              {label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
