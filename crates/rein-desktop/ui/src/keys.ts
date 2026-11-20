// Key names, vim style (desktop.md "Default keymap"): a KeyboardEvent becomes one token —
// `a`, `:`, `Space`, `Enter`, `Esc`, `Tab`, `S-Tab`, `C-n`, `C-\`, `C-S-p`, `M-1`, `F1`.
// Pure: no DOM beyond the event's fields, so it tests under Node.

export interface KeyLike {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
  /** The physical key (`Digit1`); read only for Ctrl+Shift+digit, whose `key` differs by
   *  layout (`!` on US, `+` on some) — the panel keys (E-60) must not — and for Tab when
   *  `key` is "Unidentified" (Shift+Tab on Linux; D-070 addendum 1). */
  code?: string;
}

const NAMED: Record<string, string> = {
  " ": "Space",
  Enter: "Enter",
  Escape: "Esc",
  Tab: "Tab",
  Backspace: "BS",
  Delete: "Del",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  Insert: "Insert",
};

const MODIFIER_ONLY = new Set(["Shift", "Control", "Alt", "Meta", "CapsLock", "NumLock", "Dead", "Unidentified", "Process"]);

/** The token for a key press, or `null` for a lone modifier. */
export function keyName(e: KeyLike): string | null {
  // WebKitGTK reports Shift+Tab (ISO_Left_Tab) as key "Unidentified" with code "Tab"
  // (E-99, measured on Xvfb): that one case is named from the physical key; every other
  // "Unidentified" key stays without a token (desktop.md "Shift+Tab on Linux").
  const key = e.key === "Unidentified" && e.code === "Tab" ? "Tab" : e.key;
  if (MODIFIER_ONLY.has(key)) return null;
  let base = NAMED[key] ?? key;
  const printable = base.length === 1;
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("C");
  if (e.altKey) mods.push("M");
  if (e.metaKey) mods.push("D");
  // A printable key already carries Shift in its character (`A`, `:`), except with Ctrl,
  // where browsers still report the shifted letter: `C-S-p` for Ctrl+Shift+P.
  if (e.shiftKey && (!printable || (e.ctrlKey && /^[a-z]$/i.test(base)))) mods.push("S");
  if (printable && e.ctrlKey) base = base.toLowerCase();
  // Ctrl+Shift+digit is `C-S-<digit>` on every layout (panels.md §API, E-60).
  const digit = e.code ? /^Digit([0-9])$/.exec(e.code)?.[1] : undefined;
  if (digit !== undefined && e.ctrlKey && e.shiftKey) {
    base = digit;
    if (!mods.includes("S")) mods.push("S");
  }
  return mods.length ? `${mods.join("-")}-${base}` : base;
}

/** The bytes a terminal sends for `token`, for the keys a held chord prefix can be (a Ctrl
 *  key or a printable character); `null` when the token is not one of those. */
export function keyBytes(token: string): string | null {
  if (token.length === 1) return token;
  if (token === "Space") return " ";
  const ctrl = /^C-(.)$/.exec(token);
  if (ctrl?.[1]) {
    const c = ctrl[1].toUpperCase().charCodeAt(0);
    if (c >= 0x40 && c <= 0x5f) return String.fromCharCode(c & 0x1f); // C-a … C-_, C-\ C-] C-^
  }
  return null;
}
