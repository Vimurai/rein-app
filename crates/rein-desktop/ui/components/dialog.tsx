// shadcn/ui Dialog, new-york look driven only by the theme tokens (desktop.md §Look and feel
// v2 V2, E-50): a rounded card with a shadow over a dimmed stage, the title as its heading.
// `bare`: no padding and a hidden title, for a command palette (menu, project picker).
// V7 (E-54): an overlay closes with an exit animation — `Exit` keeps the last one mounted for
// --motion-fast after the window drops it, and `Dialog` reads that it is closing, so Radix
// marks it `data-state="closed"` and the stylesheet's exit keyframes run.
import * as DialogPrimitive from "@radix-ui/react-dialog";
import { createContext, useContext, useEffect, useReducer, useRef, type ComponentProps, type ReactNode } from "react";
import { MOTION, reducedMotion } from "../src/motion.ts";
import { cn } from "../lib/utils.ts";

/** True while an overlay is on its way out (inside `Exit`). */
export const ClosingContext = createContext(false);

/** Keep the overlay `render()` draws mounted for --motion-fast after `show` turns false, marked
 *  as closing, so it can animate out; it unmounts at once under reduced motion. The overlay is
 *  drawn as it was last shown — it takes no input while it leaves. */
export function Exit({ show, render }: { show: boolean; render: () => ReactNode }) {
  const kept = useRef<ReactNode>(null);
  const [, redraw] = useReducer((n: number) => n + 1, 0);
  if (show) kept.current = render();
  else if (kept.current !== null && reducedMotion()) kept.current = null;
  const leaving = !show && kept.current !== null;
  useEffect(() => {
    if (!leaving) return;
    const t = setTimeout(() => {
      kept.current = null;
      redraw();
    }, MOTION.fast);
    return () => clearTimeout(t);
  }, [leaving]);
  if (kept.current === null) return null;
  return (
    <ClosingContext.Provider value={!show}>
      <div className={show ? undefined : "pointer-events-none"}>{kept.current}</div>
    </ClosingContext.Provider>
  );
}

/** The dialog root: open unless it is leaving (`Exit`). */
export function Dialog({ open = true, ...props }: ComponentProps<typeof DialogPrimitive.Root>) {
  const closing = useContext(ClosingContext);
  return <DialogPrimitive.Root open={open && !closing} {...props} />;
}
export const DialogTrigger = DialogPrimitive.Trigger;
export const DialogClose = DialogPrimitive.Close;

export function DialogContent({
  className,
  children,
  title,
  bare = false,
  portal = true,
  ...props
}: ComponentProps<typeof DialogPrimitive.Content> & { title: ReactNode; bare?: boolean; portal?: boolean }) {
  const content = (
    <>
      <DialogPrimitive.Overlay data-slot="dialog-overlay" className="fixed inset-0 z-40 bg-background/70" />
      <DialogPrimitive.Content
        data-slot="dialog-content"
        className={cn(
          "fixed top-1/2 left-1/2 z-50 flex max-h-[80vh] w-[min(560px,calc(100vw-32px))] -translate-1/2 flex-col overflow-hidden rounded-lg border border-border bg-popover font-ui text-[13px] text-popover-foreground shadow-md outline-none",
          bare ? "gap-0 p-0" : "gap-4 p-5",
          className,
        )}
        {...props}
      >
        <DialogPrimitive.Title className={bare ? "sr-only" : "text-[15px] leading-5 font-semibold"}>{title}</DialogPrimitive.Title>
        {children}
      </DialogPrimitive.Content>
    </>
  );
  return portal ? <DialogPrimitive.Portal>{content}</DialogPrimitive.Portal> : content;
}

export function DialogDescription({ className, ...props }: ComponentProps<typeof DialogPrimitive.Description>) {
  return <DialogPrimitive.Description className={cn("-mt-2 text-muted-foreground", className)} {...props} />;
}

export function DialogFooter({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn("flex shrink-0 justify-end gap-2", className)} {...props} />;
}
