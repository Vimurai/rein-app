// shadcn/ui Context Menu, restyled to the grid (D-037): the terminal's Copy / Paste / Select
// all menu — box border, one row per item, shortcut at the right.
import * as ContextMenuPrimitive from "@radix-ui/react-context-menu";
import type { ComponentProps } from "react";
import { cn } from "../lib/utils.ts";

export const ContextMenu = ContextMenuPrimitive.Root;
export const ContextMenuTrigger = ContextMenuPrimitive.Trigger;

export function ContextMenuContent({ className, ...props }: ComponentProps<typeof ContextMenuPrimitive.Content>) {
  return (
    <ContextMenuPrimitive.Portal>
      <ContextMenuPrimitive.Content
        data-slot="context-menu-content"
        className={cn("z-50 min-w-[20ch] border border-border bg-bg py-0 font-mono text-fg", className)}
        {...props}
      />
    </ContextMenuPrimitive.Portal>
  );
}

export function ContextMenuItem({ className, ...props }: ComponentProps<typeof ContextMenuPrimitive.Item>) {
  return (
    <ContextMenuPrimitive.Item
      data-slot="context-menu-item"
      className={cn(
        "flex h-cell cursor-default items-center gap-2 px-1 outline-none select-none data-[disabled]:text-dim data-[highlighted]:bg-bar-bg data-[highlighted]:text-accent",
        className,
      )}
      {...props}
    />
  );
}

export function ContextMenuShortcut({ className, ...props }: ComponentProps<"span">) {
  return <span className={cn("ml-auto text-dim", className)} {...props} />;
}
