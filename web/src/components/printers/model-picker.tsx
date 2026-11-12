import { ChevronRight } from "lucide-react";
import { useMemo, useState } from "react";
import type { Dialect, ProfileInfo } from "@/api/generated/types.gen";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Sheet, SheetBody, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { columnsText, paperText } from "@/lib/profile";

const GENERIC = "Generic";
const ALL = "all";

const maker = (model: ProfileInfo) => model.vendor || GENERIC;
const isStar = (model: ProfileInfo) => model.features.includes("starCommands");
const facts = (model: ProfileInfo) => [paperText(model), columnsText(model.columns), model.code_pages ? `${model.code_pages} code pages` : ""].filter(Boolean);

/** Models by maker, the generic ones first. */
const byMaker = (a: ProfileInfo, b: ProfileInfo) => {
  const order = (model: ProfileInfo) => (maker(model) === GENERIC ? "" : maker(model));
  return order(a).localeCompare(order(b)) || a.name.localeCompare(b.name);
};

function ModelRow({ model, selected, fits, onPick }: { model: ProfileInfo; selected: boolean; fits: boolean; onPick: () => void }) {
  return (
    <button
      type="button"
      data-selected={selected}
      onClick={onPick}
      className="flex w-full flex-col gap-1.5 border-b px-4 py-3 text-left text-sm hover:bg-muted/50 data-[selected=true]:bg-muted data-[fits=false]:opacity-50"
      data-fits={fits}
    >
      <span className="flex items-center gap-2">
        <span className="font-medium">{model.name}</span>
        <Badge variant="outline">{maker(model)}</Badge>
      </span>
      {facts(model).length > 0 && <span className="text-sm text-muted-foreground">{facts(model).join(" · ")}</span>}
    </button>
  );
}

/** The model of a printer: the one chosen, and a sheet over the form to choose another from all of them. */
export function ModelPicker({ models, value, dialect, onChange }: { models: ProfileInfo[]; value: string; dialect: Dialect; onChange: (name: string) => void }) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [makerFilter, setMakerFilter] = useState(ALL);
  const chosen = models.find((model) => model.name === value);
  const makers = useMemo(() => [...new Set(models.map(maker))].sort((a, b) => (a === GENERIC ? -1 : b === GENERIC ? 1 : a.localeCompare(b))), [models]);
  const shown = useMemo(() => {
    const wanted = search.trim().toLowerCase();
    return models
      .filter((model) => makerFilter === ALL || maker(model) === makerFilter)
      .filter((model) => !wanted || `${model.name} ${model.vendor} ${model.model}`.toLowerCase().includes(wanted))
      .sort(byMaker);
  }, [models, search, makerFilter]);
  // A Star language reads the models with Star commands, and the others read the rest.
  const fits = (model: ProfileInfo) => (dialect === "escpos" ? !isStar(model) : isStar(model));

  return (
    <>
      <Button type="button" variant="outline" aria-label="Model" className="h-auto w-full justify-between px-3 py-2 text-left" onClick={() => setOpen(true)}>
        <span className="flex min-w-0 flex-col">
          <span className="font-medium">{chosen ? chosen.name : "Choose a model"}</span>
          {chosen && <span className="truncate text-sm font-normal text-muted-foreground">{[maker(chosen), ...facts(chosen)].join(" · ")}</span>}
        </span>
        <ChevronRight className="text-muted-foreground" />
      </Button>
      <Sheet open={open} onOpenChange={setOpen}>
        <SheetContent className="sm:max-w-xl">
          <SheetHeader>
            <SheetTitle>Choose a model</SheetTitle>
          </SheetHeader>
          <div className="flex gap-2 border-b p-4">
            <Input aria-label="Search models" placeholder="Search models" value={search} onChange={(event) => setSearch(event.target.value)} />
            <Select items={[{ value: ALL, label: "All makers" }, ...makers.map((name) => ({ value: name, label: name }))]} value={makerFilter} onValueChange={(name) => setMakerFilter(String(name))}>
              <SelectTrigger aria-label="Maker" className="w-44 shrink-0">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ALL}>All makers</SelectItem>
                {makers.map((name) => (
                  <SelectItem key={name} value={name}>
                    {name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <SheetBody className="gap-0 p-0">
            {shown.length === 0 && <p className="p-4 text-sm text-muted-foreground">No model matches.</p>}
            {shown.map((model) => (
              <ModelRow
                key={model.name}
                model={model}
                selected={model.name === value}
                fits={fits(model)}
                onPick={() => {
                  onChange(model.name);
                  setOpen(false);
                }}
              />
            ))}
          </SheetBody>
        </SheetContent>
      </Sheet>
    </>
  );
}
