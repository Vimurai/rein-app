// shadcn/ui Command (cmdk), restyled to the grid (D-037): the menu's searchable action list —
// a `>` prompt row, then one row per item with its shortcut at the right.
import { Command as CommandPrimitive } from "cmdk";
import type { ComponentProps } from "react";
import { cn } from "../lib/utils.ts";

export function Command({ className, ...props }: ComponentProps<typeof CommandPrimitive>) {
  return (
    <CommandPrimitive
      data-slot="command"
      className={cn("flex w-full flex-col bg-bg font-mono text-fg", className)}
      {...props}
    />
  );
}

export function CommandInput({ className, ...props }: ComponentProps<typeof CommandPrimitive.Input>) {
  return (
    <div className="flex h-cell items-center gap-1 px-1">
      <span className="text-accent">&gt;</span>
      <CommandPrimitive.Input
        data-slot="command-input"
        className={cn("w-full bg-bg font-mono text-fg outline-none placeholder:text-dim", className)}
        {...props}
      />
    </div>
  );
}

export function CommandList({ className, ...props }: ComponentProps<typeof CommandPrimitive.List>) {
  return (
    <CommandPrimitive.List
      data-slot="command-list"
      className={cn("max-h-[60vh] overflow-y-auto", className)}
      {...props}
    />
  );
}

export function CommandEmpty(props: ComponentProps<typeof CommandPrimitive.Empty>) {
  return <CommandPrimitive.Empty data-slot="command-empty" className="h-cell px-2 text-dim" {...props} />;
}

export function CommandGroup({ className, ...props }: ComponentProps<typeof CommandPrimitive.Group>) {
  return (
    <CommandPrimitive.Group
      data-slot="command-group"
      className={cn("[&_[cmdk-group-heading]]:px-1 [&_[cmdk-group-heading]]:text-dim", className)}
      {...props}
    />
  );
}

export function CommandItem({ className, ...props }: ComponentProps<typeof CommandPrimitive.Item>) {
  return (
    <CommandPrimitive.Item
      data-slot="command-item"
      className={cn(
        "flex h-cell cursor-default items-center gap-1 px-2 select-none data-[disabled=true]:text-dim data-[selected=true]:bg-bar-bg data-[selected=true]:text-accent",
        className,
      )}
      {...props}
    />
  );
}

export function CommandShortcut({ className, ...props }: ComponentProps<"span">) {
  return <span data-slot="command-shortcut" className={cn("ml-auto text-dim", className)} {...props} />;
}
