// A start that fails (desktop.md V14 components 3–5, D-068; E-94): the bar above the
// terminal and what happens when a tab's child ends. The words and buttons are
// `start-failed.ts`'s; the verdict is the core's (`project_check`), never read from the
// terminal's bytes.

import { Button } from "../components/button.tsx";
import { projectCheck } from "./ipc.ts";
import { afterSetUp, failedBar, failedFrom, type FailedAction, type FailedState } from "./start-failed.ts";

/** An open tab as this file sees it (the shell's own tab entries fit this shape). */
export interface FailedTab {
  failed?: FailedState | undefined;
  /** Set up was run from the bar in this tab open: no second offer (V14 component 5). */
  setUpOnce?: boolean | undefined;
  term: { tab?: { id: number } | null | undefined };
}

/** A tab's child ended. A failed set-up from the bar brings the bar back as `no_fix`; a
 *  failed start asks rein once (the core waits for a check already running). `live`: the tab
 *  is still open. */
export function onTabExit(t: FailedTab, code: number, startFailed: boolean, live: () => boolean, rerender: () => void): void {
  const after = afterSetUp(code, t.setUpOnce === true);
  if (after) {
    t.failed = after;
    rerender();
    return;
  }
  const id = t.term.tab?.id;
  if (!startFailed || id === undefined) return;
  t.failed = { at: "checking" };
  rerender();
  void projectCheck(id)
    .then(
      (r) => failedFrom(r),
      () => failedFrom(null),
    )
    .then((f) => {
      if (!live() || t.term.tab?.id !== id || t.failed?.at !== "checking") return;
      t.failed = t.setUpOnce === true && f.at === "needs_init" ? { at: "no_fix" } : f;
      rerender();
    });
}

/** The start-failed bar: two fixed lines and its buttons; the terminal stays below it. */
export function StartFailedBar({ state, onAction }: { state: FailedState; onAction: (a: FailedAction) => void }) {
  const v = failedBar(state);
  return (
    <div
      id="start-failed-bar"
      role="status"
      aria-busy={state.at === "checking" || undefined}
      className="fade-in flex shrink-0 items-center gap-[8px] border-b border-border bg-muted px-[12px] py-[6px] font-ui text-[13px] text-foreground"
    >
      <span className="grid min-w-0 flex-1">
        <span className="truncate font-medium">{v.lines[0]}</span>
        <span className="truncate text-muted-foreground">{v.lines[1]}</span>
      </span>
      {v.buttons.map((b) => (
        <Button key={b.action} size="sm" variant={b.primary ? "primary" : b.action === "close" ? "ghost" : undefined} onClick={() => onAction(b.action)}>
          {b.label}
        </Button>
      ))}
    </div>
  );
}
