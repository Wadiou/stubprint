import type { ReactNode } from "react";
import type { ProfileInfo } from "@/api/generated/types.gen";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

export const refused = (error: unknown) => (error as { error?: string } | null)?.error ?? "The server refused it.";

const modelName = (model: ProfileInfo) => (model.vendor ? `${model.vendor} ${model.model}` : model.name);

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-2">
      <span className="text-sm font-medium">{label}</span>
      {children}
    </div>
  );
}

/** The printer models of the database, to choose one from. */
export function ModelSelect({
  models,
  value,
  onChange,
  label,
}: {
  models: ProfileInfo[];
  value: string;
  onChange: (name: string) => void;
  label: string;
}) {
  return (
    <Select
      items={models.map((model) => ({ value: model.name, label: modelName(model) }))}
      value={value}
      onValueChange={(name) => onChange(String(name))}
    >
      <SelectTrigger aria-label={label} className="w-full">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {models.map((model) => (
          <SelectItem key={model.name} value={model.name}>
            {modelName(model)}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
