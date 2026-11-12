// shadcn/ui Context Menu, new-york look driven only by the theme tokens (desktop.md §Look and
// feel v2 V2, E-50): the terminal's Copy / Paste / Select all menu — a rounded popover, the
// shortcut at the right of each row.
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
        collisionPadding={8}
        className={cn(
          "z-50 min-w-48 overflow-hidden rounded-md border border-border bg-popover p-1 font-ui text-[13px] text-popover-foreground shadow-md",
          className,
        )}
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
        "group flex h-8 cursor-default items-center gap-2 rounded-sm px-2 outline-none select-none data-[disabled]:opacity-50 data-[highlighted]:bg-primary/15",
        className,
      )}
      {...props}
    />
  );
}

export function ContextMenuSeparator({ className, ...props }: ComponentProps<typeof ContextMenuPrimitive.Separator>) {
  return <ContextMenuPrimitive.Separator className={cn("-mx-1 my-1 h-px bg-border", className)} {...props} />;
}

export function ContextMenuShortcut({ className, ...props }: ComponentProps<"span">) {
  return <span className={cn("ml-auto pl-6 font-mono text-[11px] text-muted-foreground group-data-[highlighted]:text-popover-foreground", className)} {...props} />;
}
