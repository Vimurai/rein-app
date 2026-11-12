// Health, update / rollback and profiles in the window (desktop.md §Components 5, §Core
// Concept "Two ways in", D-045, E-39). Every machine-changing step ends in two labelled
// buttons with the safe one focused; `y` / `n` answer too. The CLI's lines are shown as text,
// escaped by the core and unchanged; the exit code is the only thing decided on.

import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Button, Kbd } from "../components/button.tsx";
import { Dialog, DialogContent, DialogFooter } from "../components/dialog.tsx";
import { healthBadge } from "./chrome.ts";
import { Spinner } from "../components/loaders.tsx";
import { CircleCheck, CircleHelp, CircleX, TriangleAlert } from "./icons.ts";
import {
  Channel,
  profilesApply,
  profilesPlan,
  releaseApply,
  releaseCheck,
  type HealthView,
  type ProfilesRun,
  type ReleaseCheck,
} from "./ipc.ts";

const errText = (e: unknown) => (e as { message?: string }).message ?? String(e);

/** A card: the title, the body (scrolls inside the dialog), the buttons. */
function Panel({
  title,
  onClose,
  onKeyDown,
  children,
  footer,
  yes,
}: {
  title: string;
  onClose: () => void;
  onKeyDown?: (e: KeyboardEvent<HTMLDivElement>) => void;
  children: ReactNode;
  footer?: ReactNode;
  /** A confirm step: what `y` does (`n` cancels), shown at the left of the buttons. */
  yes?: string | undefined;
}) {
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent title={title} aria-describedby={undefined} onKeyDown={onKeyDown} className="w-[min(680px,calc(100vw-32px))]">
        {/* Scrolls inside the dialog (min-h-0 lets it shrink in the column), by wheel, arrows or
            PageDown once focused. */}
        <div tabIndex={0} className="-mx-1 grid min-h-0 flex-1 content-start gap-3 overflow-y-auto px-1 outline-none">
          {children}
        </div>
        {footer && (
          <DialogFooter className="items-center">
            {yes && (
              <span className="mr-auto flex items-center gap-1.5 text-[12px] text-muted-foreground">
                <Kbd>y</Kbd> {yes} <Kbd className="ml-2">n</Kbd> cancel
              </span>
            )}
            {footer}
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}

/** What a CLI printed, as it printed it: mono, in a well. */
function Output({ lines }: { lines: readonly string[] }) {
  if (!lines.length) return null;
  return (
    <pre className="m-0 rounded-md border border-border bg-background p-3 font-mono text-[12px] [overflow-wrap:anywhere] whitespace-pre-wrap text-foreground">
      {lines.join("\n")}
    </pre>
  );
}

/** The command a step runs, shown so the CLI can be learned (rule 4). */
function Runs({ cmd }: { cmd: string }) {
  return (
    <div className="flex items-center gap-2 text-[12px] text-muted-foreground">
      <span>Runs</span>
      <code className="rounded-sm border border-border bg-background px-1.5 py-0.5 font-mono text-[11px] text-foreground">{cmd}</code>
    </div>
  );
}

/** A status line: an icon in the status colour, the text at full contrast. */
function StatusLine({ status, children }: { status: "ok" | "warn" | "fail" | "unknown" | "busy"; children: ReactNode }) {
  const p = { size: 15, "aria-hidden": true, className: "mt-px shrink-0" } as const;
  const icon =
    status === "ok" ? (
      <CircleCheck {...p} className={`${p.className} text-ansi-10`} />
    ) : status === "warn" ? (
      <TriangleAlert {...p} className={`${p.className} text-ansi-11`} />
    ) : status === "fail" ? (
      <CircleX {...p} className={`${p.className} text-destructive`} />
    ) : status === "busy" ? (
      <span className="mt-px flex size-[15px] shrink-0 items-center justify-center text-muted-foreground">
        <Spinner size={14} />
      </span>
    ) : (
      <CircleHelp {...p} className={`${p.className} text-muted-foreground`} />
    );
  return (
    <div className="flex items-start gap-2" aria-busy={status === "busy" || undefined}>
      {icon}
      <div className="min-w-0" role={status === "busy" ? "status" : undefined}>
        {children}
      </div>
    </div>
  );
}

export function HealthOverlay({ health, onClose }: { health: HealthView | null; onClose: () => void }) {
  const st = (s: string | undefined) => (s === "ok" || s === "warn" || s === "fail" || s === "unknown" ? s : "busy");
  return (
    <Panel
      title="Installation health"
      onClose={onClose}
      footer={
        <Button variant="primary" autoFocus onClick={onClose}>
          Close
        </Button>
      }
    >
      <StatusLine status={st(health?.status)}>
        <div className="font-medium">{healthBadge(health).text}</div>
        {health?.reason && <div className="text-muted-foreground">{health.reason}</div>}
      </StatusLine>
      {!!health?.checks.length && (
        <ul className="m-0 grid list-none gap-2 rounded-md border border-border bg-background p-3">
          {health.checks.map((c, i) => (
            <li key={i}>
              <StatusLine status={st(c.status)}>
                <div className="[overflow-wrap:anywhere]">
                  <span className="font-mono text-[12px]">{c.id}</span> <span>{c.line}</span>
                </div>
                {c.fix && <div className="font-mono text-[12px] [overflow-wrap:anywhere] text-muted-foreground">fix: {c.fix}</div>}
              </StatusLine>
            </li>
          ))}
        </ul>
      )}
      <Runs cmd="rein doctor --scope install --json" />
    </Panel>
  );
}

type Phase =
  | { at: "checking" }
  | { at: "confirm"; check: ReleaseCheck | null }
  | { at: "running"; lines: string[] }
  | { at: "done"; lines: string[]; code: number | null; note: string };

/** Update or rollback: check, confirm, run, report (D-045: `update --check`, then `--yes`). */
export function ReleaseOverlay({
  kind,
  onClose,
  onCommand,
}: {
  kind: "update" | "rollback";
  onClose: () => void;
  /** The exact CLI command this run is, for the command line (`y` copies it). */
  onCommand: (cmd: string) => void;
}) {
  const [phase, setPhase] = useState<Phase>(kind === "update" ? { at: "checking" } : { at: "confirm", check: null });
  const lines = useRef<string[]>([]);

  useEffect(() => {
    if (kind !== "update") return;
    onCommand("rein-app update --check");
    releaseCheck().then(
      (check) => {
        if (check.code === 0) setPhase({ at: "confirm", check });
        else {
          const note =
            check.code === 10 ? "Rein is up to date" : check.code === 11 ? "the release server cannot be reached" : `exit ${check.code ?? "?"}`;
          setPhase({ at: "done", lines: check.lines, code: check.code, note });
        }
      },
      (e) => setPhase({ at: "done", lines: [errText(e)], code: null, note: "could not check for updates" }),
    );
  }, [kind, onCommand]);

  /** Set before the call: a second `y` (or key repeat) before React re-renders starts nothing. */
  const started = useRef(false);
  const run = () => {
    if (started.current) return;
    started.current = true;
    const cmd = kind === "update" ? "rein-app update --yes" : "rein-app rollback";
    onCommand(cmd);
    lines.current = [];
    setPhase({ at: "running", lines: [] });
    const out = new Channel<string>((l) => {
      lines.current = [...lines.current, l];
      setPhase({ at: "running", lines: lines.current });
    });
    releaseApply(kind, out).then(
      (code) => setPhase({ at: "done", lines: lines.current, code, note: code === 0 ? "done" : `ended with exit ${code ?? "?"}` }),
      (e) => {
        // `busy`: another run (this one's double) is going; this overlay already shows it.
        if ((e as { kind?: string }).kind === "busy") return;
        setPhase({ at: "done", lines: [...lines.current, errText(e)], code: null, note: "did not run" });
      },
    );
  };

  const title = kind === "update" ? "Update Rein" : "Undo last update";
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (phase.at !== "confirm") return;
    if (e.key === "y") {
      e.preventDefault();
      run();
    } else if (e.key === "n") {
      e.preventDefault();
      onClose();
    }
  };
  const busy = phase.at === "running" || phase.at === "checking";
  return (
    <Panel
      title={title}
      onClose={() => !busy && onClose()}
      onKeyDown={onKeyDown}
      yes={phase.at === "confirm" ? (kind === "update" ? "install" : "roll back") : undefined}
      footer={
        phase.at === "confirm" ? (
          <>
            <Button autoFocus onClick={onClose}>
              Cancel
            </Button>
            <Button variant="primary" onClick={run}>
              {kind === "update" ? `Update to ${phase.check?.newest ?? "the newest"}` : "Undo last update"}
            </Button>
          </>
        ) : phase.at === "running" ? (
          // V7: the confirm button turns busy; nothing can be cancelled once a run started.
          <>
            <Button disabled>Cancel</Button>
            <Button variant="primary" disabled aria-busy="true">
              <Spinner size={14} />
              {kind === "update" ? "Updating…" : "Rolling back…"}
            </Button>
          </>
        ) : phase.at === "done" ? (
          <Button variant="primary" autoFocus onClick={onClose}>
            Close
          </Button>
        ) : null
      }
    >
      {phase.at === "checking" && <StatusLine status="busy">Checking for a newer release…</StatusLine>}
      {phase.at === "confirm" && kind === "update" && phase.check && (
        <>
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground">Installed</span>
            <span className="font-mono text-[12px]">{phase.check.installed ?? "none"}</span>
            <span className="text-muted-foreground">→ newest</span>
            <span className="font-mono text-[12px] font-semibold">{phase.check.newest ?? "?"}</span>
          </div>
          <Output lines={phase.check.lines.slice(2)} />
          <Runs cmd="rein-app update --yes" />
        </>
      )}
      {phase.at === "confirm" && kind === "rollback" && (
        <>
          <div>Restore the previous Rein version from its snapshot (your data is kept), then check it.</div>
          <Runs cmd="rein-app rollback" />
        </>
      )}
      {(phase.at === "running" || phase.at === "done") && <Output lines={phase.lines} />}
      {phase.at === "done" && (
        <StatusLine status={phase.code === 0 || phase.code === 10 ? "ok" : "fail"}>
          <span className="font-medium">{phase.note}</span>
        </StatusLine>
      )}
    </Panel>
  );
}

/** Profiles: the dry run's per-profile diff of the three Rein-owned keys, then apply. */
export function ProfilesOverlay({ onClose, onCommand }: { onClose: () => void; onCommand: (cmd: string) => void }) {
  const [plan, setPlan] = useState<ProfilesRun | null>(null);
  const [result, setResult] = useState<ProfilesRun | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);

  useEffect(() => {
    profilesPlan([]).then(
      (p) => {
        setPlan(p);
        onCommand(["rein-app profiles apply --dry-run", ...p.profiles.map((n) => `--profile ${n}`)].join(" "));
      },
      (e) => setError(errText(e)),
    );
  }, [onCommand]);

  const apply = () => {
    if (!plan) return;
    setRunning(true);
    onCommand(["rein-app profiles apply", ...plan.profiles.map((n) => `--profile ${n}`)].join(" "));
    profilesApply(plan.profiles).then(
      (r) => {
        setResult(r);
        setRunning(false);
      },
      (e) => {
        setError(errText(e));
        setRunning(false);
      },
    );
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (!plan || result || running) return;
    if (e.key === "y") {
      e.preventDefault();
      apply();
    } else if (e.key === "n") {
      e.preventDefault();
      onClose();
    }
  };
  const shown = result ?? plan;
  return (
    <Panel
      title="Profiles"
      onClose={() => !running && onClose()}
      onKeyDown={onKeyDown}
      yes={plan && !result && !error && !running ? "apply" : undefined}
      footer={
        plan && !result && !error ? (
          <>
            {/* Cancel stays usable while applying, as before E-54 (V7: "where cancelling is possible today"). */}
            <Button autoFocus onClick={onClose}>
              Cancel
            </Button>
            <Button variant="primary" onClick={apply} disabled={running} aria-busy={running || undefined}>
              {running ? (
                <>
                  <Spinner size={14} />
                  Applying…
                </>
              ) : (
                <>
                  Apply to {plan.profiles.length} profile{plan.profiles.length === 1 ? "" : "s"}
                </>
              )}
            </Button>
          </>
        ) : (
          <Button variant="primary" autoFocus onClick={onClose}>
            Close
          </Button>
        )
      }
    >
      {!shown && !error && <StatusLine status="busy">Reading the profiles…</StatusLine>}
      {shown && <Output lines={shown.lines} />}
      {plan && !result && !error && <Runs cmd={["rein-app profiles apply", ...plan.profiles.map((n) => `--profile ${n}`)].join(" ")} />}
      {result && (
        <StatusLine status={result.code === 0 ? "ok" : "fail"}>
          <span className="font-medium">{result.code === 0 ? "Applied" : `Ended with exit ${result.code ?? "?"}`}</span>
        </StatusLine>
      )}
      {error && (
        <StatusLine status="fail">
          <span className="[overflow-wrap:anywhere]">{error}</span>
        </StatusLine>
      )}
    </Panel>
  );
}
