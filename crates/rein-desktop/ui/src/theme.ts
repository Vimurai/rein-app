// Themes: sixteen terminal colours plus foreground, background and accent (desktop.md "For
// tiling window managers"). E-34 ships `gruvbox-dark`; E-37 adds the other five and config.

export interface Theme {
  name: string;
  fg: string;
  bg: string;
  accent: string;
  /** Terminal colours 0–15, also given to xterm.js (E-35), so chrome and terminal match. */
  ansi: readonly string[];
  /** Chrome roles derived from the palette. */
  barBg: string;
  barFg: string;
  dim: string;
  /** Dim text on `barBg`. */
  barDim: string;
  border: string;
}

export const GRUVBOX_DARK: Theme = {
  name: "gruvbox-dark",
  fg: "#ebdbb2",
  bg: "#282828",
  accent: "#fabd2f",
  ansi: [
    "#282828", "#cc241d", "#98971a", "#d79921", "#458588", "#b16286", "#689d6a", "#a89984",
    "#928374", "#fb4934", "#b8bb26", "#fabd2f", "#83a598", "#d3869b", "#8ec07c", "#ebdbb2",
  ],
  barBg: "#3c3836",
  barFg: "#ebdbb2",
  dim: "#a89984",
  barDim: "#bdae93",
  border: "#7c6f64",
};

/** The CSS variables the stylesheet reads, for one theme. */
export function cssVariables(t: Theme): Record<string, string> {
  return {
    "--fg": t.fg,
    "--bg": t.bg,
    "--accent": t.accent,
    "--bar-bg": t.barBg,
    "--bar-fg": t.barFg,
    "--dim": t.dim,
    "--bar-dim": t.barDim,
    "--border": t.border,
    "--ok": t.ansi[10] ?? t.fg,
    "--green": t.ansi[10] ?? t.fg,
  };
}

/** WCAG relative luminance of `#rrggbb`. */
export function luminance(hex: string): number {
  const ch = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const lin = ch.map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * (lin[0] ?? 0) + 0.7152 * (lin[1] ?? 0) + 0.0722 * (lin[2] ?? 0);
}

/** WCAG contrast ratio between two `#rrggbb` colours (1–21). */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}
