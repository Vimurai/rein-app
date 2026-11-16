// The rail (panels.md §Components 1, D-056; E-60): a 40 px column at the window's left edge,
// below the title bar — one icon button per enabled panel, in descriptor order, its tooltip
// the panel's title and key. A click opens that panel; the same click again closes it. At
// the foot, the Project sheet (V10, E-64). Every button runs an action: the menu and the keys
// reach the same things (pointer parity, D-034).

import type { ComponentType, ReactNode } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "../components/tooltip.tsx";
import type { Runner } from "./frame.tsx";
import { LayoutDashboard, ListChecks, MessagesSquare, Users } from "./icons.ts";
import type { ActionCall } from "./keymap.ts";
import type { Descriptor, PanelIcon } from "./panels.ts";

/** A descriptor's icon name → the bundled icon (panels.ts `PANEL_ICONS` is the closed list). */
export const ICON: Record<PanelIcon, ComponentType<{ size?: number; "aria-hidden"?: boolean | "true" }>> = {
  "layout-dashboard": LayoutDashboard,
  "messages-square": MessagesSquare,
  "list-checks": ListChecks,
};

const BUTTON =
  "flex size-8 items-center justify-center rounded-md text-muted-foreground transition-colors duration-[var(--motion-fast)] hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none aria-pressed:bg-muted aria-pressed:text-foreground";

export function Rail({ panels, open, r }: { panels: readonly Descriptor[]; open: string | null; r: Runner }) {
  const button = (call: ActionCall, tip: string, icon: ReactNode, pressed?: boolean) => (
    <Tooltip key={`${call.name} ${call.arg ?? ""}`}>
      <TooltipTrigger asChild>
        <button type="button" aria-label={tip} aria-pressed={pressed} className={BUTTON} onClick={() => r.run(call)}>
          {icon}
        </button>
      </TooltipTrigger>
      <TooltipContent side="right">{tip}</TooltipContent>
    </Tooltip>
  );
  return (
    <nav aria-label="Panels" className="flex w-10 shrink-0 flex-col items-center gap-1 border-r border-border bg-background py-2">
      {panels.map((d) => {
        const call: ActionCall = { name: "panel.toggle", arg: d.id };
        const Icon = ICON[d.icon];
        return button(call, r.tip(call), <Icon size={16} aria-hidden="true" />, open === d.id);
      })}
      <div className="flex-1" />
      {button({ name: "project.team" }, r.tip({ name: "project.team" }), <Users size={16} aria-hidden="true" />)}
    </nav>
  );
}
