// The Project sheet, Team section (desktop.md §Look and feel v2 V10, E-64), drawn from the pure
// `team.ts`. rein's lines go in as text nodes only. The mode switch asks here first (default
// No); the core then asks again in a native dialog of its own before rein runs.

import { useCallback, useEffect, useState } from "react";
import { Button } from "../components/button.tsx";
import { Dialog, DialogContent, DialogFooter } from "../components/dialog.tsx";
import { Input } from "../components/input.tsx";
import { Spinner } from "../components/loaders.tsx";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../components/select.tsx";
import { teamAdd, teamGet, teamMode } from "./ipc.ts";
import { Output, Panel, StatusLine } from "./release.tsx";
import {
  MODE_BUTTON,
  MODE_CONFIRM,
  NOT_OFFERED,
  formHint,
  formReady,
  heldAfter,
  offer,
  runNote,
  type Mode,
  type Prefix,
  type Run,
  type TeamView,
} from "./team.ts";

const errText = (e: unknown) => (e as { message?: string }).message ?? String(e);

export function TeamSheet({ tab, project, onCommand, onClose }: { tab: number; project: string; onCommand: (cmd: string) => void; onClose: () => void }) {
  const [view, setView] = useState<TeamView | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  /** The run in flight, by the button that started it: its spinner shows there, and every
   *  other button is only disabled (E-60, from the E-67 review). */
  const [busyOn, setBusyOn] = useState<"add" | "mode" | null>(null);
  const busy = busyOn !== null;
  const [email, setEmail] = useState("");
  const [prefix, setPrefix] = useState<Prefix>("E");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  /** The last add or mode run, shown under the form as rein printed it. */
  const [result, setResult] = useState<{ title: string; run: Run } | { title: string; error: string } | null>(null);
  const [confirm, setConfirm] = useState<Mode | null>(null);
  /** rein may still be running in WSL: Add and the mode button stay off until reopened. */
  const [held, setHeld] = useState(false);

  const load = useCallback(() => {
    setView(null);
    setFailed(null);
    onCommand("rein team show && rein trunk");
    teamGet(tab).then(setView, (e) => setFailed(errText(e)));
  }, [tab, onCommand]);
  useEffect(load, [load]);

  const add = () => {
    if (!formReady(email, from, to) || busy || held) return;
    setBusyOn("add");
    onCommand(`rein team add ${email} --range ${prefix}:${from}-${to}`);
    teamAdd(tab, email, prefix, Number(from), Number(to))
      .then(
        (run) => {
          setResult({ title: "rein team add", run });
          setHeld((h) => heldAfter(h, run));
          if (run.code === 0) load();
        },
        (e) => setResult({ title: "rein team add", error: errText(e) }),
      )
      .finally(() => setBusyOn(null));
  };

  const runMode = (mode: Mode) => {
    setConfirm(null);
    setBusyOn("mode");
    onCommand(`rein state migrate --${mode}`);
    teamMode(tab, mode)
      .then(
        (end) => {
          if ("declined" in end) return;
          setResult({ title: `rein state migrate --${mode}`, run: end });
          setHeld((h) => heldAfter(h, end));
          if (!end.stuck) load();
        },
        (e) => setResult({ title: `rein state migrate --${mode}`, error: errText(e) }),
      )
      .finally(() => setBusyOn(null));
  };

  const o = view ? offer(view.shape) : null;
  const hint = formHint(email, from, to);
  return (
    <>
      <Panel
        title="Project: team"
        onClose={() => !busy && onClose()}
        footer={
          <Button variant="primary" autoFocus onClick={onClose} disabled={busy}>
            Close
          </Button>
        }
      >
        <code className="font-mono text-[12px] [overflow-wrap:anywhere] text-muted-foreground">{project}</code>
        {!view && !failed && <StatusLine status="busy">Asking rein for the team…</StatusLine>}
        {failed && <StatusLine status="fail">{failed}</StatusLine>}
        {view && (
          <>
            <div className="grid gap-1">
              <span className="text-[12px] text-muted-foreground">rein team show</span>
              <Output lines={view.show.lines} />
            </div>
            <div className="flex items-center gap-2 text-[12px]">
              <span className="text-muted-foreground">rein trunk</span>
              <code className="font-mono [overflow-wrap:anywhere] text-foreground">{view.trunk.lines.join(" ") || "—"}</code>
            </div>
            {o?.note && <StatusLine status="warn">{o.note}</StatusLine>}
            {o?.form && (
              <form
                className="grid gap-2 rounded-md border border-border p-3"
                onSubmit={(e) => {
                  e.preventDefault();
                  add();
                }}
              >
                <span className="font-medium">Add a developer</span>
                <div className="flex flex-wrap items-center gap-2">
                  <Input aria-label="E-mail" placeholder="dev@example.com" className="w-64" value={email} onChange={(e) => setEmail(e.target.value.trim())} />
                  <Select value={prefix} onValueChange={(v) => setPrefix(v as Prefix)}>
                    <SelectTrigger aria-label="Task prefix" className="w-20 min-w-0">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="E">E</SelectItem>
                      <SelectItem value="P">P</SelectItem>
                    </SelectContent>
                  </Select>
                  <Input aria-label="From" placeholder="from" inputMode="numeric" className="w-24" value={from} onChange={(e) => setFrom(e.target.value.trim())} />
                  <span className="text-muted-foreground">–</span>
                  <Input aria-label="To" placeholder="to" inputMode="numeric" className="w-24" value={to} onChange={(e) => setTo(e.target.value.trim())} />
                  <Button type="submit" disabled={busy || held || !formReady(email, from, to)}>
                    {busyOn === "add" && <Spinner size={12} />}
                    Add
                  </Button>
                </div>
                {hint && <span className="text-[12px] text-destructive">{hint}</span>}
              </form>
            )}
            {result && (
              <div className="grid gap-1">
                <span className="text-[12px] text-muted-foreground">{result.title}</span>
                {"run" in result ? (
                  <>
                    <Output lines={result.run.lines} />
                    <StatusLine status={runNote(result.run).status}>{runNote(result.run).text}</StatusLine>
                  </>
                ) : (
                  <StatusLine status="fail">{result.error}</StatusLine>
                )}
              </div>
            )}
            {o?.mode && (
              <div className="flex items-center justify-between gap-2">
                <span className="text-[12px] text-muted-foreground">{NOT_OFFERED}</span>
                <Button disabled={busy || held} onClick={() => setConfirm(o.mode)}>
                  {busyOn === "mode" && <Spinner size={12} />}
                  {MODE_BUTTON[o.mode]}
                </Button>
              </div>
            )}
          </>
        )}
      </Panel>
      {confirm && (
        <Dialog open onOpenChange={(open) => !open && setConfirm(null)}>
          <DialogContent title={MODE_CONFIRM[confirm].title} aria-describedby={undefined}>
            <p className="m-0 text-[13px] text-foreground">{MODE_CONFIRM[confirm].text}</p>
            <DialogFooter>
              <Button autoFocus onClick={() => setConfirm(null)}>
                No, keep it
              </Button>
              <Button variant="primary" onClick={() => runMode(confirm)}>
                {MODE_BUTTON[confirm]}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </>
  );
}
