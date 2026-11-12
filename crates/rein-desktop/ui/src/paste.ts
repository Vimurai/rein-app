// Paste (desktop.md §Security 6, E-40): a paste holding a newline asks once before it is sent.

/** The lines a paste would type. */
export function pasteLines(text: string): number {
  return text.split(/\r\n|\r|\n/).length;
}

/** Whether to ask before sending `text` to the terminal. */
export function pasteNeedsQuestion(text: string): boolean {
  return pasteLines(text) > 1;
}
