// The loading primitives (desktop.md §Look and feel v2 V7 component 1, D-054): CSS only, each
// held invisible for --loader-delay (150 ms) so a fast operation shows no flicker; under
// reduced motion each stands still and the text label beside it carries the meaning. The
// classes and keyframes live in src/style.css. Decorative: the label is the accessible part.
import type { ComponentProps } from "react";
import { LoaderCircle } from "../src/icons.ts";
import { cn } from "../lib/utils.ts";

/** The loader-circle icon turning, in the colour of its text: 12, 14 or 16 px. */
export function Spinner({ size = 14, className }: { size?: 12 | 14 | 16; className?: string }) {
  return <LoaderCircle size={size} aria-hidden="true" data-slot="spinner" className={cn("spinner", className)} />;
}

/** A 2 px indeterminate line in the accent colour that sweeps across its container. */
export function ProgressLine({ className }: { className?: string }) {
  return (
    <div aria-hidden="true" data-slot="progress-line" className={cn("progress-line", className)}>
      <span />
    </div>
  );
}

/** A rounded muted block with a slow pulse, standing in for content that has not arrived. */
export function Skeleton({ className, ...props }: ComponentProps<"div">) {
  return <div aria-hidden="true" data-slot="skeleton" className={cn("skeleton", className)} {...props} />;
}
