// shadcn/ui Scroll Area, restyled to the grid (D-037): a one-cell-wide bar with a block thumb.
// The viewport's scrollbar-hiding rule ships in tailwind.css (the CSP refuses Radix's own
// <style> element).
import * as ScrollAreaPrimitive from "@radix-ui/react-scroll-area";
import type { ComponentProps } from "react";
import { cn } from "../lib/utils.ts";

export function ScrollArea({ className, children, ...props }: ComponentProps<typeof ScrollAreaPrimitive.Root>) {
  return (
    <ScrollAreaPrimitive.Root data-slot="scroll-area" className={cn("relative overflow-hidden", className)} {...props}>
      <ScrollAreaPrimitive.Viewport className="size-full">{children}</ScrollAreaPrimitive.Viewport>
      <ScrollAreaPrimitive.Scrollbar orientation="vertical" className="flex w-1 touch-none bg-bar-bg select-none">
        <ScrollAreaPrimitive.Thumb className="relative flex-1 bg-border" />
      </ScrollAreaPrimitive.Scrollbar>
      <ScrollAreaPrimitive.Corner />
    </ScrollAreaPrimitive.Root>
  );
}
