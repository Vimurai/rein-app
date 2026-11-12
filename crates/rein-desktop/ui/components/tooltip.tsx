// shadcn/ui Tooltip, new-york look driven only by the theme tokens (desktop.md §Look and feel
// v2 V2, E-50): a small rounded popover in the UI font. It drops in and fades out (V7,
// E-54: the keyframes in src/style.css, keyed on Radix's data-state).
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
  sideOffset = 6,
  portal = true,
  ...props
}: ComponentProps<typeof TooltipPrimitive.Content> & { portal?: boolean }) {
  const content = (
    <TooltipPrimitive.Content
      data-slot="tooltip-content"
      sideOffset={sideOffset}
      collisionPadding={8}
      className={cn(
        "z-50 rounded-md border border-border bg-popover px-2 py-1 font-ui text-[12px] whitespace-pre text-popover-foreground shadow-md",
        className,
      )}
      {...props}
    />
  );
  return portal ? <TooltipPrimitive.Portal>{content}</TooltipPrimitive.Portal> : content;
}
