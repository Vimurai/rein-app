// Rendered to HTML by test/home.test.mjs: one Home card with an in-progress, an open and a
// blocked task (status.md "Home, second revision" R4, R5, D-071) — what the card lists, its
// two tab stops in order, and project text drawn as text.
import { renderToString } from "react-dom/server";
import { HomeView } from "../../src/home.tsx";
import { startScreen } from "../../src/start.ts";
import type { ProjectStatus } from "../../src/status.ts";

const status: ProjectStatus = {
  tab: 4,
  state: "working",
  quiet_s: 2,
  worked_s: 60,
  waiting: null,
  now: [],
  info: [],
  in_progress: { id: "E-80", title: "Build Home." },
  turn: "Engineer",
  last_handoff_s: 60,
  counts: { open: 1, in_progress: 1, blocked: 1 },
  trigger: "Review",
  agents: [],
  active: [
    { id: "E-80", title: "Build Home.", status: "in_progress" },
    { id: "E-79", title: "<b>System notifications.</b>", status: "open" },
    { id: "E-81", title: "Read rein's waiting signal.", status: "blocked" },
  ],
};

export function card(): string {
  return renderToString(
    <HomeView
      tabs={[{ id: 1, index: 0, title: "rein-app", status }]}
      now={[]}
      dismiss={null}
      goTo={() => {}}
      openTasks={() => {}}
      start={startScreen([], null, 0, { loading: false, checking: false })}
      r={{ run: () => {}, tip: () => "" }}
    />,
  );
}
