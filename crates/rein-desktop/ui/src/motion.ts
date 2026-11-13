// The V7 motion tokens (desktop.md §Look and feel v2 V7, D-054) for the timers and Web
// Animations that live in script: the same values as the stylesheet's --motion-* tokens
// (src/style.css), which a test holds equal. Everything in CSS names the tokens themselves.

export const MOTION = {
  fast: 120,
  base: 150,
  slow: 220,
  spin: 800,
  sweep: 1100,
  ease: "cubic-bezier(0.2, 0, 0, 1)",
} as const;

/** Whether the system asks for reduced motion: then nothing moves and nothing waits for an
 *  exit animation. */
export function reducedMotion(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
}
