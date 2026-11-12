// shadcn/ui Dialog, restyled to the grid (D-037, desktop.md "Screen": an overlay floats over
// the stage with a box border and the title in the border). Square, no shadow, no animation.
import * as DialogPrimitive from "@radix-ui/react-dialog";
import type { ComponentProps, ReactNode } from "react";
import { cn } from "../lib/utils.ts";

export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;
export const DialogClose = DialogPrimitive.Close;

export function DialogContent({
  className,
  children,
  title,
  portal = true,
  ...props
}: ComponentProps<typeof DialogPrimitive.Content> & { title: ReactNode; portal?: boolean }) {
  const content = (
    <>
      <DialogPrimitive.Overlay data-slot="dialog-overlay" className="fixed inset-0 z-40 bg-bg/60" />
      <DialogPrimitive.Content
        data-slot="dialog-content"
        className={cn(
          "fixed top-1/2 left-1/2 z-50 flex max-h-[80vh] w-[min(80ch,90vw)] -translate-1/2 flex-col border border-border bg-bg font-mono text-fg outline-none",
          className,
        )}
        {...props}
      >
        <DialogPrimitive.Title className="-mt-[0.6em] ml-2 self-start bg-bg px-1 text-accent">
          {title}
        </DialogPrimitive.Title>
        {children}
      </DialogPrimitive.Content>
    </>
  );
  return portal ? <DialogPrimitive.Portal>{content}</DialogPrimitive.Portal> : content;
}

export function DialogDescription({ className, ...props }: ComponentProps<typeof DialogPrimitive.Description>) {
  return <DialogPrimitive.Description className={cn("px-2 text-dim", className)} {...props} />;
}

export function DialogFooter({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn("flex justify-end gap-2 px-2 py-0", className)} {...props} />;
}
