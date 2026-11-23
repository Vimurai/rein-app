// Rendered to HTML by test/updates.test.mjs: the window's update card in each state, the
// "Updated to" toast and the window's row of the Update dialog (updates.md §Components 4, E-87).
import { renderToString } from "react-dom/server";
import { AppRow, AppUpdateCard, UpdatedToast } from "../../src/update-ui.tsx";
import { appCard, type UpdatesState } from "../../src/updates.ts";

const none = () => {};

/** The card for a state, or "" when the state shows none. */
export function card(s: UpdatesState): string {
  const c = appCard(s);
  return c ? renderToString(<AppUpdateCard card={c} onLater={none} onStart={none} onCancel={none} />) : "";
}

export function toast(version: string, notes: boolean): string {
  return renderToString(<UpdatedToast version={version} notes={notes} onNotes={none} onClose={none} />);
}

export function row(s: UpdatesState): string {
  return renderToString(<AppRow updates={s} onStart={none} />);
}
