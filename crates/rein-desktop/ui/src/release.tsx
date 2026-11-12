// Health, update / rollback and profiles in the window (desktop.md §Components 5, §Core
// Concept "Two ways in", D-045, E-39). Every machine-changing step ends in two labelled
// buttons with the safe one focused; `y` / `n` answer too. The CLI's lines are shown as text,
// escaped by the core and unchanged; the exit code is the only thing decided on.

import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Button } from "../components/button.tsx";
import { Dialog, DialogContent } from "../components/dialog.tsx";
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

/** A statusline segment for the health view. */
export function healthSegment(h: HealthView | null): string {
  switch (h?.status) {
    case "ok":
      return "✓ install ok";
    case "warn":
      return "! install warn";
    case "fail":
      return "✗ install FAIL";
    case "unknown":
      return "? health unknown";
    default:
      return "… checking health";
  }
}

/** A box with a title in its border, scrolling lines, and the buttons. */
function Panel({
  title,
  onClose,
  onKeyDown,
  children,
  footer,
}: {
  title: string;
  onClose: () => void;
  onKeyDown?: (e: KeyboardEvent<HTMLDivElement>) => void;
  children: ReactNode;
  footer?: ReactNode;
}) {
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent title={title} aria-describedby={undefined} onKeyDown={onKeyDown}>
        {/* Scrolls inside the dialog (min-h-0 lets it shrink in the column), by wheel, arrows or
            PageDown once focused; the half-shown last row says that more follows. */}
        <div tabIndex={0} className="min-h-0 flex-1 overflow-y-auto px-1 whitespace-pre-wrap outline-none">
          {children}
        </div>
        {footer && <div className="flex justify-end gap-2 px-1 pb-[calc(var(--cell-h)/2)]">{footer}</div>}
      </DialogContent>
    </Dialog>
  );
}

export function HealthOverlay({ health, onClose }: { health: HealthView | null; onClose: () => void }) {
  const cls = (s: string) => (s === "fail" ? "text-ansi-9" : s === "warn" ? "text-accent" : "text-fg");
  return (
    <Panel
      title="checkhealth"
      onClose={onClose}
      footer={
        <Button variant="primary" autoFocus onClick={onClose}>
          [ Close ]
        </Button>
      }
    >
      <div className="text-dim">rein doctor --scope install --json</div>
      <div className={cls(health?.status ?? "")}>{healthSegment(health)}</div>
      {health?.reason && <div className="text-dim">{health.reason}</div>}
      {health?.checks.map((c, i) => (
        <div key={i}>
          {/* A wrapped line lines up under the check text, not at column 0. */}
          <div className="pl-[5ch] -indent-[5ch]">
            <span className={cls(c.status)}>{c.status === "fail" ? "FAIL" : c.status.padEnd(4)}</span> {c.id}: {c.line}
          </div>
          {c.fix && <div className="pl-6 text-dim">fix: {c.fix}</div>}
        </div>
      ))}
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
      footer={
        phase.at === "confirm" ? (
          <>
            <Button autoFocus onClick={onClose}>
              [ Cancel ]
            </Button>
            <Button variant="primary" onClick={run}>
              {kind === "update" ? `[ Update to ${phase.check?.newest ?? "the newest"} ]` : "[ Undo last update ]"}
            </Button>
          </>
        ) : phase.at === "done" ? (
          <Button variant="primary" autoFocus onClick={onClose}>
            [ Close ]
          </Button>
        ) : null
      }
    >
      {phase.at === "checking" && <div className="text-dim">checking for a newer release…</div>}
      {phase.at === "confirm" && kind === "update" && phase.check && (
        <>
          <div>
            installed: {phase.check.installed ?? "none"} → newest: {phase.check.newest ?? "?"}
          </div>
          <div className="text-dim">{phase.check.lines.slice(2).join("\n")}</div>
          <div className="text-dim">y install · n cancel — runs: rein-app update --yes</div>
        </>
      )}
      {phase.at === "confirm" && kind === "rollback" && (
        <>
          <div>Restore the previous Rein version from its snapshot (your data is kept), then check it.</div>
          <div className="text-dim">y roll back · n cancel — runs: rein-app rollback</div>
        </>
      )}
      {(phase.at === "running" || phase.at === "done") && <div>{phase.lines.join("\n")}</div>}
      {phase.at === "done" && <div className={phase.code === 0 || phase.code === 10 ? "text-accent" : "text-ansi-9"}>{phase.note}</div>}
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
      footer={
        plan && !result && !error ? (
          <>
            <Button autoFocus onClick={onClose}>
              [ Cancel ]
            </Button>
            <Button variant="primary" onClick={apply} disabled={running}>
              [ Apply to {plan.profiles.length} profile{plan.profiles.length === 1 ? "" : "s"} ]
            </Button>
          </>
        ) : (
          <Button variant="primary" autoFocus onClick={onClose}>
            [ Close ]
          </Button>
        )
      }
    >
      {!shown && !error && <div className="text-dim">reading the profiles…</div>}
      {shown && <div>{shown.lines.join("\n")}</div>}
      {result && (
        <div className={result.code === 0 ? "text-accent" : "text-ansi-9"}>
          {result.code === 0 ? "applied" : `ended with exit ${result.code ?? "?"}`}
        </div>
      )}
      {error && <div className="text-ansi-9">{error}</div>}
    </Panel>
  );
}
