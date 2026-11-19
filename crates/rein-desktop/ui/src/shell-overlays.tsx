// The toast and every overlay of the window (moved out of shell.tsx by E-97, no change): the
// command line, the menu, the paste question, the close and stop confirms, the Update project
// sheet, which-key, health / release / profiles, the update card, the team sheet and the
// project picker.

import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import { Exit } from "../components/dialog.tsx";
import { CloseConfirm, StopConfirm } from "./agent-bar.tsx";
import { CommandPalette, Toast } from "./frame.tsx";
import { projectsForget, updatesLater, type HealthView } from "./ipc.ts";
import type { ActionCall, KeyTable } from "./keymap.ts";
import { closeTabText } from "./lifecycle.ts";
import { menuEntries } from "./menu.ts";
import { continuations, type ModeState } from "./modes.ts";
import type { OpenTab, UpdateSheetState } from "./open-tab.ts";
import { MenuOverlay, PasteConfirm, WhichKey } from "./overlays.tsx";
import { ProjectPicker } from "./project-picker.tsx";
import { UpdateSheet } from "./project-ui.tsx";
import { HealthOverlay, ProfilesOverlay, ReleaseOverlay } from "./release.tsx";
import type { ShellProps } from "./shell.tsx";
import type { RecentView } from "./start.ts";
import { TeamSheet } from "./team-sheet.tsx";
import type { TermTab } from "./term.ts";
import { UpdateCard } from "./update-ui.tsx";
import { reinAsk, updateRows, type UpdatesState } from "./updates.ts";

export type OverlayKind = "health" | "update" | "rollback" | "profiles" | "menu" | "team" | null;

export interface ShellOverlaysProps {
  props: ShellProps;
  message: { text: string; error: boolean; seq: number; leaving?: boolean } | null;
  modeView: ModeState;
  setMode: (next: ModeState) => void;
  cmdText: string;
  completing: boolean;
  completion: MutableRefObject<{ base: string; items: string[]; index: number } | null>;
  overlay: OverlayKind;
  closeOverlay: () => void;
  menuQuery: string;
  keys: KeyTable;
  otherTable: KeyTable;
  whichKey: boolean;
  tabs: MutableRefObject<OpenTab[]>;
  cur: MutableRefObject<number>;
  tab: { readonly current: TermTab | null };
  run: (call: ActionCall) => void;
  open: (project: string, restore?: boolean) => (() => Promise<void>) | null;
  say: (text: string, error?: boolean) => void;
  rerender: () => void;
  pasteText: string | null;
  setPasteText: Dispatch<SetStateAction<string | null>>;
  confirmClose: { entry: OpenTab; running: string[] } | null;
  setConfirmClose: Dispatch<SetStateAction<{ entry: OpenTab; running: string[] } | null>>;
  closeProject: (entry: OpenTab, confirmed: boolean, auto?: boolean) => void;
  stop: { entry: OpenTab; roles: string[] } | null;
  setStop: Dispatch<SetStateAction<{ entry: OpenTab; roles: string[] } | null>>;
  closeTab: (i?: number, confirmed?: boolean) => void;
  onCloseDontAsk: () => void;
  updateSheet: UpdateSheetState;
  setUpdateSheet: Dispatch<SetStateAction<UpdateSheetState>>;
  showCommand: (cmd: string, note?: string) => void;
  restoreChip: () => void;
  health: HealthView | null;
  updates: UpdatesState | null;
  picker: boolean;
  setPicker: Dispatch<SetStateAction<boolean>>;
  recent: RecentView[];
  setRecent: Dispatch<SetStateAction<RecentView[]>>;
  recentLoaded: boolean;
}

export function ShellOverlays(p: ShellOverlaysProps) {
  const { props, message, modeView, setMode, cmdText, completing, completion, overlay, closeOverlay, menuQuery, keys, otherTable, whichKey } = p;
  const { tabs, cur, tab, run, open, say, rerender, pasteText, setPasteText, confirmClose, setConfirmClose, closeProject, stop, setStop } = p;
  const { closeTab, onCloseDontAsk, updateSheet, setUpdateSheet, showCommand, restoreChip, health, updates, picker, setPicker, recent, setRecent, recentLoaded } = p;
  return (
    <>
      <Toast message={message} />
      {/* Each overlay leaves with its exit animation (V7): `Exit` keeps it for --motion-fast. */}
      <Exit
        show={modeView.mode === "COMMAND"}
        render={() => <CommandPalette text={cmdText} items={completion.current?.items ?? []} pending={completing} />}
      />
      <Exit
        show={overlay === "menu"}
        render={() => (
          <MenuOverlay
            entries={menuEntries(keys, otherTable, { tabs: tabs.current.length, panels: props.panelsEnabled })}
            initialQuery={menuQuery}
            onRun={(c) => {
              closeOverlay();
              run(c);
            }}
            onClose={closeOverlay}
          />
        )}
      />
      <Exit
        show={pasteText !== null}
        render={() => (
          <PasteConfirm
            text={pasteText ?? ""}
            onPaste={() => {
              if (pasteText !== null) tab.current?.paste(pasteText);
              setPasteText(null);
              tab.current?.focus();
            }}
            onClose={() => {
              setPasteText(null);
              tab.current?.focus();
            }}
          />
        )}
      />
      <Exit
        show={confirmClose !== null}
        render={() => (
          <CloseConfirm
            running={confirmClose?.running ?? []}
            onYes={() => {
              const c = confirmClose;
              setConfirmClose(null);
              if (c) closeProject(c.entry, true);
            }}
            onNo={() => {
              setConfirmClose(null);
              tab.current?.focus();
            }}
          />
        )}
      />
      <Exit
        show={updateSheet !== null}
        render={() =>
          updateSheet && (
            <UpdateSheet
              tab={updateSheet.id}
              rows={updateSheet.rows}
              title={updateSheet.run ? "Set up this project" : "Update project"}
              onRun={updateSheet.run}
              onCommand={showCommand}
              onDone={(outcome) => {
                if (!outcome.keepBar) updateSheet.entry.update = undefined;
                rerender();
              }}
              onClose={() => {
                setUpdateSheet(null);
                restoreChip();
                tab.current?.focus();
              }}
            />
          )
        }
      />
      <Exit
        show={stop !== null}
        render={() =>
          stop && (
            <StopConfirm
              title="Close project?"
              text={closeTabText(stop.roles, stop.entry.title)}
              yes="Close"
              onYes={(dontAsk) => {
                const s = stop;
                setStop(null);
                if (dontAsk) onCloseDontAsk();
                const i = tabs.current.indexOf(s.entry);
                if (i >= 0) closeTab(i, true);
              }}
              onNo={() => {
                setStop(null);
                tab.current?.focus();
              }}
            />
          )
        }
      />
      {whichKey && <WhichKey leader={keys.leader} next={continuations(keys, modeView.pending)} />}
      <Exit show={overlay === "health"} render={() => <HealthOverlay health={health} onClose={closeOverlay} />} />
      <Exit
        show={overlay === "update" || overlay === "rollback"}
        render={() => (
          <ReleaseOverlay kind={overlay === "rollback" ? "rollback" : "update"} rows={updateRows(updates)} onClose={closeOverlay} onCommand={showCommand} />
        )}
      />
      <Exit show={reinAsk(updates) !== null && overlay !== "update"} render={() => <UpdateCard version={reinAsk(updates) ?? ""} onLater={() => void updatesLater("rein").catch(() => {})} onUpdate={() => run({ name: "release.update" })} />} />
      <Exit show={overlay === "profiles"} render={() => <ProfilesOverlay onClose={closeOverlay} onCommand={showCommand} />} />
      <Exit
        show={overlay === "team"}
        render={() => {
          const t = tabs.current[cur.current];
          const id = t?.term.tab?.id;
          return id === undefined || !t ? null : <TeamSheet tab={id} project={t.project} onCommand={showCommand} onClose={closeOverlay} />;
        }}
      />
      <Exit
        show={picker}
        render={() => (
          <ProjectPicker
            recent={recent}
            loading={!recentLoaded}
            openPaths={tabs.current.map((t) => t.project)}
            onPick={(path) => { setPicker(false); open(path); }}
            onChoose={() => { setPicker(false); run({ name: "project.open" }); }}
            onForget={(path) => {
              projectsForget(path).then(setRecent, () => say(`E: cannot forget ${path}`, true));
            }}
            onClose={() => { setPicker(false); if (tabs.current.length) setMode({ mode: "TERMINAL", pending: [] }); }}
          />
        )}
      />
    </>
  );
}
