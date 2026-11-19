// The rail (panels.md §Components 1, D-056; E-60): a 40 px column at the window's left edge,
// below the title bar — one icon button per enabled panel, in descriptor order, its tooltip
// the panel's title and key. A click opens that panel; the same click again closes it. At
// the foot, the Project sheet (V10, E-64). Every button runs an action: the menu and the keys
// reach the same things (pointer parity, D-034).

import type { ComponentType, ReactNode } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "../components/tooltip.tsx";
import type { Runner } from "./frame.tsx";
import { Bell, LayoutDashboard, ListChecks, MessagesSquare, Users } from "./icons.ts";
import type { ActionCall } from "./keymap.ts";
import type { Descriptor, PanelIcon } from "./panels.ts";
import { badgeText } from "./attention.ts";

/** A descriptor's icon name → the bundled icon (panels.ts `PANEL_ICONS` is the closed list). */
export const ICON: Record<PanelIcon, ComponentType<{ size?: number; "aria-hidden"?: boolean | "true" }>> = {
  "layout-dashboard": LayoutDashboard,
  "messages-square": MessagesSquare,
  "list-checks": ListChecks,
  bell: Bell,
};

const BUTTON =
  "flex size-8 items-center justify-center rounded-md text-muted-foreground transition-colors duration-[var(--motion-fast)] hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none aria-pressed:bg-muted aria-pressed:text-foreground";

/** `home` (status.md §Home H4, E-80): Home is shown — only the panels it names have something
 *  to show; the rest, and Project: team, are disabled with `home.tip`. */
export function Rail({
  panels,
  open,
  r,
  badge,
  home = null,
}: {
  panels: readonly Descriptor[];
  open: string | null;
  r: Runner;
  badge: number;
  home?: { panels: readonly string[]; tip: string } | null;
}) {
  const button = (call: ActionCall, tip: string, icon: ReactNode, pressed?: boolean, count = 0, off = false) => (
    <Tooltip key={`${call.name} ${call.arg ?? ""}`}>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={count > 0 ? `${tip} — ${count} waiting` : tip}
          aria-pressed={pressed}
          aria-disabled={off || undefined}
          className={`relative ${BUTTON} ${off ? "cursor-default opacity-40 hover:bg-transparent hover:text-muted-foreground" : ""}`}
          onClick={() => !off && r.run(call)}
        >
          {icon}
          {count > 0 && (
            <span
              data-badge=""
              aria-hidden="true"
              className="absolute -top-0.5 -right-0.5 min-w-4 rounded-full bg-primary px-1 text-center text-[10px] leading-4 font-medium text-primary-foreground tabular-nums"
            >
              {badgeText(count)}
            </span>
          )}
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
        const off = home !== null && !home.panels.includes(d.id);
        return button(call, off ? home.tip : r.tip(call), <Icon size={16} aria-hidden="true" />, open === d.id && !off, d.source === "attention" ? badge : 0, off);
      })}
      <div className="flex-1" />
      {button({ name: "project.team" }, home ? home.tip : r.tip({ name: "project.team" }), <Users size={16} aria-hidden="true" />, undefined, 0, home !== null)}
    </nav>
  );
}
