// Themes: sixteen terminal colours plus foreground, background and accent (desktop.md "For
// tiling window managers"). E-34 ships `gruvbox-dark`; E-37 adds the other five and config.
// The default is `catppuccin-mocha` (D-049 addendum 1, E-48). The terminal's background is
// the theme's `bg`, the window's background too, in every theme.

import type { ITheme } from "@xterm/xterm";

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
  /** Error lines on `bg`, and the statusline's mode segments (text in `bg` on them): the
   *  terminal palette's red / green / blue / yellow unless a theme needs a readable tone. */
  error: string;
  modeTerminal: string;
  modeNormal: string;
  modeCommand: string;
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
  error: "#fe6352",
  modeTerminal: "#b8bb26",
  modeNormal: "#83a598",
  modeCommand: "#fabd2f",
};

const theme = (
  name: string,
  fg: string,
  bg: string,
  accent: string,
  ansi: string[],
  chrome: { barBg: string; barFg: string; dim: string; barDim: string; border: string },
  roles: Partial<Pick<Theme, "error" | "modeTerminal" | "modeNormal" | "modeCommand">> = {},
): Theme => ({
  name,
  fg,
  bg,
  accent,
  ansi,
  ...chrome,
  error: roles.error ?? ansi[9] ?? fg,
  modeTerminal: roles.modeTerminal ?? ansi[10] ?? fg,
  modeNormal: roles.modeNormal ?? ansi[12] ?? fg,
  modeCommand: roles.modeCommand ?? ansi[11] ?? fg,
});

export const TOKYONIGHT = theme("tokyonight", "#c0caf5", "#1a1b26", "#7aa2f7", [
  "#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#a9b1d6",
  "#414868", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#c0caf5",
], { barBg: "#24283b", barFg: "#c0caf5", dim: "#a9b1d6", barDim: "#a9b1d6", border: "#565f89" });

export const CATPPUCCIN_MOCHA = theme("catppuccin-mocha", "#cdd6f4", "#1e1e2e", "#cba6f7", [
  "#45475a", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#bac2de",
  "#585b70", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#a6adc8",
], { barBg: "#313244", barFg: "#cdd6f4", dim: "#a6adc8", barDim: "#bac2de", border: "#6c7086" });

export const NORD = theme("nord", "#d8dee9", "#2e3440", "#88c0d0", [
  "#3b4252", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead", "#88c0d0", "#e5e9f0",
  "#4c566a", "#d08770", "#a3be8c", "#ebcb8b", "#88c0d0", "#b48ead", "#8fbcbb", "#eceff4",
], { barBg: "#3b4252", barFg: "#e5e9f0", dim: "#aeb6c3", barDim: "#c2c9d4", border: "#616e88" }, { error: "#e0957d" });

export const SOLARIZED_DARK = theme("solarized-dark", "#93a1a1", "#002b36", "#b58900", [
  "#073642", "#dc322f", "#859900", "#b58900", "#268bd2", "#d33682", "#2aa198", "#eee8d5",
  "#586e75", "#cb4b16", "#859900", "#b58900", "#268bd2", "#6c71c4", "#93a1a1", "#fdf6e3",
], { barBg: "#073642", barFg: "#eee8d5", dim: "#8c9a9b", barDim: "#a3afaf", border: "#586e75" }, { error: "#f2643f", modeNormal: "#3a9be2" });

export const GITHUB_LIGHT = theme("github-light", "#1f2328", "#ffffff", "#0969da", [
  "#24292f", "#cf222e", "#116329", "#4d2d00", "#0969da", "#8250df", "#1b7c83", "#6e7781",
  "#57606a", "#a40e26", "#1a7f37", "#633c01", "#0550ae", "#6639ba", "#136061", "#8c959f",
], { barBg: "#eaeef2", barFg: "#1f2328", dim: "#59636e", barDim: "#4d5761", border: "#8c959f" });

/** The theme when config.toml and Settings name none (D-049 addendum 1). */
export const DEFAULT_THEME_NAME = "catppuccin-mocha";

export const THEMES: ReadonlyMap<string, Theme> = new Map(
  [GRUVBOX_DARK, TOKYONIGHT, CATPPUCCIN_MOCHA, NORD, SOLARIZED_DARK, GITHUB_LIGHT].map((t) => [t.name, t]),
);

const DEFAULT_THEME: Theme = THEMES.get(DEFAULT_THEME_NAME) ?? GRUVBOX_DARK;

/** A built-in theme with `[theme.custom]` colours laid over it (`fg`, `bg`, `accent`,
 *  `color0` … `color15`); the core has checked every value is `#rrggbb`. */
export function resolveTheme(name: string, custom: Readonly<Record<string, string>> = {}): Theme {
  const base = THEMES.get(name) ?? DEFAULT_THEME;
  const ansi = [...base.ansi];
  for (const [k, v] of Object.entries(custom)) {
    const n = /^color(\d+)$/.exec(k)?.[1];
    if (n !== undefined && Number(n) < 16) ansi[Number(n)] = v;
  }
  return {
    ...base,
    fg: custom["fg"] ?? base.fg,
    bg: custom["bg"] ?? base.bg,
    accent: custom["accent"] ?? base.accent,
    ansi,
  };
}

/** The six built-in themes (desktop.md "For tiling window managers"); E-37 ships the five
 *  besides `gruvbox-dark`. `:theme` completes these names. */
export const THEME_NAMES: readonly string[] = [
  "gruvbox-dark",
  "tokyonight",
  "catppuccin-mocha",
  "nord",
  "solarized-dark",
  "github-light",
];

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
    "--error": t.error,
    "--mode-terminal": t.modeTerminal,
    "--mode-normal": t.modeNormal,
    "--mode-command": t.modeCommand,
    // The sixteen terminal colours, for the Tailwind theme tokens (`text-ansi-1`, …).
    ...Object.fromEntries(t.ansi.map((c, i) => [`--ansi-${i}`, c])),
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

const ANSI_KEYS = [
  "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
  "brightBlack", "brightRed", "brightGreen", "brightYellow",
  "brightBlue", "brightMagenta", "brightCyan", "brightWhite",
] as const;

/** The colours the tmux overlay paints (desktop.md §Look and feel v2 V6): borders, the accent
 *  for the active border, selection and message line, and the optional status line. The
 *  core accepts `#rrggbb` in lower case only; a `[theme.custom]` colour may be upper case. */
export function tmuxColours(t: Theme): { bg: string; fg: string; muted: string; border: string; accent: string } {
  const c = (s: string) => s.toLowerCase();
  return { bg: c(t.bg), fg: c(t.fg), muted: c(t.dim), border: c(t.border), accent: c(t.accent) };
}

/** The theme's sixteen colours, given to xterm so chrome and terminal match. The
 *  terminal's background is the theme's `bg`: the window's background (E-48). */
export function xtermTheme(t: Theme): ITheme {
  const out: ITheme = { foreground: t.fg, background: t.bg, cursor: t.fg, selectionBackground: t.barBg };
  ANSI_KEYS.forEach((k, i) => {
    const c = t.ansi[i];
    if (c) out[k] = c;
  });
  return out;
}
