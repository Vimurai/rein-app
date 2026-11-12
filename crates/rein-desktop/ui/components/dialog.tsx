// shadcn/ui Dialog, new-york look driven only by the theme tokens (desktop.md §Look and feel
// v2 V2, E-50): a rounded card with a shadow over a dimmed stage, the title as its heading.
// `bare`: no padding and a hidden title, for a command palette (menu, project picker).
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
