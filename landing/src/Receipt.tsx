import { RECEIPT } from "./brand.ts";
import { cn } from "@/lib/utils";

const STYLES = {
  title: "text-center text-[15px] font-semibold",
  centre: "text-center",
  bold: "font-semibold",
} as const;

/** The paper of a receipt, as the printer would put it out. */
export function Receipt() {
  return (
    <div
      className="receipt relative w-[300px] max-w-full bg-paper px-[18px] pt-5 pb-5 font-mono text-xs leading-[1.8] whitespace-pre text-ink shadow-[0_16px_40px_rgb(0_0_0/0.5)]"
      role="img"
      aria-label="A receipt from Green Market: two coffees and a croissant, total 10.40"
    >
      <div aria-hidden="true">
        {RECEIPT.map((line) => (
          <div key={line.text} className={cn("style" in line && STYLES[line.style])}>
            {line.text}
          </div>
        ))}
      </div>
    </div>
  );
}
