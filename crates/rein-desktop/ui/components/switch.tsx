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
        "group inline-flex h-cell items-center bg-bar-bg px-1 font-mono text-bar-fg outline-none focus-visible:bg-accent focus-visible:text-bg data-[state=checked]:text-accent",
        className,
      )}
      {...props}
    >
      <span className="group-data-[state=checked]:hidden">[off]</span>
      <span className="hidden group-data-[state=checked]:inline">[ on]</span>
      <SwitchPrimitive.Thumb className="hidden" />
    </SwitchPrimitive.Root>
  );
}
