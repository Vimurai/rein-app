// shadcn/ui Switch, restyled to the grid (D-037): `[ on]` / `[off]` drawn as text, no track
// animation.
import * as SwitchPrimitive from "@radix-ui/react-switch";
import type { ComponentProps } from "react";
import { cn } from "../lib/utils.ts";

export function Switch({ className, ...props }: ComponentProps<typeof SwitchPrimitive.Root>) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        "inline-flex h-cell items-center bg-bar-bg px-1 font-mono text-bar-fg outline-none focus-visible:bg-accent focus-visible:text-bg disabled:text-dim data-[state=checked]:text-accent",
        className,
      )}
      {...props}
    >
      {/* The state as text, drawn on the grid: [  on ] / [ off ] (same width, so the right-aligned switch does not move). */}
      {props.checked ? "[  on ]" : "[ off ]"}
      <SwitchPrimitive.Thumb className="hidden" />
    </SwitchPrimitive.Root>
  );
}
