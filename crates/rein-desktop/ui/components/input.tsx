// shadcn/ui Input, new-york look driven only by the theme tokens (desktop.md §Look and feel v2
// V2, E-50): an outlined field with a focus ring in the accent.
import type { ComponentProps } from "react";
import { cn } from "../lib/utils.ts";

export function Input({ className, ...props }: ComponentProps<"input">) {
  return (
    <input
      data-slot="input"
      className={cn(
        "h-8 w-full rounded-md border border-border bg-background px-3 font-ui text-[13px] text-foreground outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring",
        className,
      )}
      {...props}
    />
  );
}
