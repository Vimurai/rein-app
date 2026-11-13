// shadcn/ui Card, new-york look driven only by the theme tokens (desktop.md §Look and feel v2
// V9 component 2, E-63): a bordered surface in the popover colour.
import type { ComponentProps } from "react";
import { cn } from "../lib/utils.ts";

export function Card({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="card"
      className={cn("rounded-lg border border-border bg-popover font-ui text-[13px] text-popover-foreground shadow-md", className)}
      {...props}
    />
  );
}
