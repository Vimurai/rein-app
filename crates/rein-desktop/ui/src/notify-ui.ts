// The `notify` event as the toast (status.md §Components 6 channel 1; E-78). What to show is
// notify.ts; this file only listens for the life of the window.

import { useEffect, useRef } from "react";
import { listen } from "./ipc.ts";
import { toastText, type Notification } from "./notify.ts";
import { coreId } from "./status.ts";
import type { StatusTab } from "./tab-status.tsx";

/** Shows each notification with `say` unless it is about the current tab (`current`). */
export function useNotifyToast(say: (text: string) => void, current: () => StatusTab | undefined): void {
  const ref = useRef({ say, current });
  ref.current = { say, current };
  useEffect(() => {
    let live = true;
    void listen<Notification>("notify", (n) => {
      if (!live) return;
      const text = toastText(n, coreId(ref.current.current()));
      if (text !== null) ref.current.say(text);
    });
    return () => {
      live = false;
    };
  }, []);
}
