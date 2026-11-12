// shadcn/ui Button, new-york look driven only by the theme tokens (desktop.md §Look and feel
// v2 V1–V2, E-50): UI font, 6 px radius, a focus outline in the accent, set off from the
// button so it still shows on a primary (accent) button.
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";
import { cn } from "../lib/utils.ts";

export const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-2 rounded-md font-ui text-[13px] font-medium whitespace-nowrap select-none transition-colors duration-150 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        default: "border border-border bg-background text-foreground hover:bg-muted",
        primary: "bg-primary text-primary-foreground hover:bg-primary/90",
        ghost: "text-foreground hover:bg-muted",
      },
      size: {
        default: "h-8 px-3",
        sm: "h-7 px-2 text-[12px]",
        icon: "size-8",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  },
);

export function Button({
  className,
  variant,
  size,
  asChild = false,
  ...props
}: ComponentProps<"button"> & VariantProps<typeof buttonVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "button";
  return <Comp data-slot="button" className={cn(buttonVariants({ variant, size }), className)} {...props} />;
}

/** A key as the UI shows it (`Ctrl+Shift+P`, `Esc`): a small chip in the mono font. */
export function Kbd({ className, ...props }: ComponentProps<"kbd">) {
  return (
    <kbd
      data-slot="kbd"
      className={cn(
        "inline-flex h-5 min-w-5 items-center justify-center rounded-sm border border-border bg-background px-1 font-mono text-[11px] text-foreground",
        className,
      )}
      {...props}
    />
  );
}
