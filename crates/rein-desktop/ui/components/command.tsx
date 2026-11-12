// shadcn/ui Command (cmdk), new-york look driven only by the theme tokens (desktop.md §Look
// and feel v2 V2, E-50): a search field with an icon, then rounded rows with the shortcut at
// the right. The selected row is tinted with the accent, and its dim text turns full
// foreground there (the dim colour on the tint is under 4.5:1 in gruvbox-dark).
import { Command as CommandPrimitive } from "cmdk";
import type { ComponentProps } from "react";
import { Search } from "../src/icons.ts";
import { cn } from "../lib/utils.ts";

export function Command({ className, ...props }: ComponentProps<typeof CommandPrimitive>) {
  return (
    <CommandPrimitive
      data-slot="command"
      className={cn("flex min-h-0 w-full flex-col overflow-hidden bg-popover font-ui text-[13px] text-popover-foreground", className)}
      {...props}
    />
  );
}

export function CommandInput({ className, ...props }: ComponentProps<typeof CommandPrimitive.Input>) {
  return (
    <div className="flex h-11 shrink-0 items-center gap-2 border-b border-border px-3">
      <Search size={16} aria-hidden="true" className="shrink-0 text-muted-foreground" />
      <CommandPrimitive.Input
        data-slot="command-input"
        className={cn("h-full w-full bg-transparent text-popover-foreground outline-none placeholder:text-muted-foreground", className)}
        {...props}
      />
    </div>
  );
}

export function CommandList({ className, ...props }: ComponentProps<typeof CommandPrimitive.List>) {
  return (
    <CommandPrimitive.List
      data-slot="command-list"
      className={cn("max-h-[min(400px,60vh)] scroll-py-1 overflow-y-auto p-1", className)}
      {...props}
    />
  );
}

export function CommandEmpty(props: ComponentProps<typeof CommandPrimitive.Empty>) {
  return <CommandPrimitive.Empty data-slot="command-empty" className="py-6 text-center text-muted-foreground" {...props} />;
}

export function CommandGroup({ className, ...props }: ComponentProps<typeof CommandPrimitive.Group>) {
  return (
    <CommandPrimitive.Group
      data-slot="command-group"
      className={cn(
        "[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:text-muted-foreground",
        className,
      )}
      {...props}
    />
  );
}

export function CommandItem({ className, ...props }: ComponentProps<typeof CommandPrimitive.Item>) {
  return (
    <CommandPrimitive.Item
      data-slot="command-item"
      className={cn(
        "group flex min-h-8 cursor-default items-center gap-2 rounded-sm px-2 py-1 select-none data-[disabled=true]:opacity-50 data-[selected=true]:bg-primary/15",
        className,
      )}
      {...props}
    />
  );
}

export function CommandShortcut({ className, ...props }: ComponentProps<"span">) {
  return (
    <span data-slot="command-shortcut" className={cn("ml-auto shrink-0 pl-4 font-mono text-[11px] text-muted-foreground group-data-[selected=true]:text-popover-foreground", className)} {...props} />
  );
}
