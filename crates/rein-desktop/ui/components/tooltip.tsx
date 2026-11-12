// shadcn/ui Tooltip, restyled to the grid (D-037): one bar-coloured row, no arrow, no shadow,
// appears without animation.
import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ComponentProps } from "react";
import { cn } from "../lib/utils.ts";

export function TooltipProvider({ delayDuration = 400, ...props }: ComponentProps<typeof TooltipPrimitive.Provider>) {
  return <TooltipPrimitive.Provider delayDuration={delayDuration} {...props} />;
}

export const Tooltip = TooltipPrimitive.Root;
export const TooltipTrigger = TooltipPrimitive.Trigger;

export function TooltipContent({
  className,
  sideOffset = 0,
  portal = true,
  ...props
}: ComponentProps<typeof TooltipPrimitive.Content> & { portal?: boolean }) {
  const content = (
    <TooltipPrimitive.Content
      data-slot="tooltip-content"
      sideOffset={sideOffset}
      className={cn("z-50 border border-border bg-bar-bg px-1 font-mono whitespace-pre text-bar-fg", className)}
      {...props}
    />
  );
  return portal ? <TooltipPrimitive.Portal>{content}</TooltipPrimitive.Portal> : content;
}
