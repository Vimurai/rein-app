// shadcn/ui Input, restyled to the grid (D-037): one text row, underlined, theme colours only.
import type { ComponentProps } from "react";
import { cn } from "../lib/utils.ts";

export function Input({ className, ...props }: ComponentProps<"input">) {
  return (
    <input
      data-slot="input"
      className={cn(
        "h-cell w-full border-b border-border bg-bg px-1 font-mono text-fg outline-none placeholder:text-dim focus:border-accent",
        className,
      )}
      {...props}
    />
  );
}
