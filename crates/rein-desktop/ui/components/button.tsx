// shadcn/ui Button, restyled to the grid (D-037): text in brackets-like cells, square, no
// shadow; colours only from the theme variables.
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";
import { cn } from "../lib/utils.ts";

export const buttonVariants = cva(
  "inline-flex h-cell items-center gap-1 px-1 font-mono whitespace-pre select-none outline-none focus-visible:bg-accent focus-visible:text-bg disabled:text-dim",
  {
    variants: {
      variant: {
        default: "bg-bar-bg text-bar-fg hover:bg-accent hover:text-bg",
        primary: "bg-accent text-bg hover:bg-fg",
        ghost: "text-fg hover:bg-bar-bg",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

export function Button({
  className,
  variant,
  asChild = false,
  ...props
}: ComponentProps<"button"> & VariantProps<typeof buttonVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "button";
  return <Comp data-slot="button" className={cn(buttonVariants({ variant }), className)} {...props} />;
}
