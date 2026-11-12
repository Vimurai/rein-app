// shadcn/ui Select, restyled to the grid (D-037): a value row with `▾`, a box-bordered list.
import * as SelectPrimitive from "@radix-ui/react-select";
import type { ComponentProps } from "react";
import { cn } from "../lib/utils.ts";

export const Select = SelectPrimitive.Root;
export const SelectValue = SelectPrimitive.Value;

export function SelectTrigger({ className, children, ...props }: ComponentProps<typeof SelectPrimitive.Trigger>) {
  return (
    <SelectPrimitive.Trigger
      data-slot="select-trigger"
      className={cn(
        "inline-flex h-cell items-center gap-1 bg-bar-bg px-1 font-mono text-bar-fg outline-none focus-visible:bg-accent focus-visible:text-bg",
        className,
      )}
      {...props}
    >
      {children}
      <SelectPrimitive.Icon>▾</SelectPrimitive.Icon>
    </SelectPrimitive.Trigger>
  );
}

export function SelectContent({ className, children, ...props }: ComponentProps<typeof SelectPrimitive.Content>) {
  return (
    <SelectPrimitive.Portal>
      <SelectPrimitive.Content
        data-slot="select-content"
        position="popper"
        className={cn("z-50 border border-border bg-bg font-mono text-fg", className)}
        {...props}
      >
        <SelectPrimitive.Viewport>{children}</SelectPrimitive.Viewport>
      </SelectPrimitive.Content>
    </SelectPrimitive.Portal>
  );
}

export function SelectItem({ className, children, ...props }: ComponentProps<typeof SelectPrimitive.Item>) {
  return (
    <SelectPrimitive.Item
      data-slot="select-item"
      className={cn(
        "flex h-cell cursor-default items-center px-1 outline-none select-none data-[highlighted]:bg-bar-bg data-[highlighted]:text-accent data-[state=checked]:text-accent",
        className,
      )}
      {...props}
    >
      <SelectPrimitive.ItemText>{children}</SelectPrimitive.ItemText>
    </SelectPrimitive.Item>
  );
}
