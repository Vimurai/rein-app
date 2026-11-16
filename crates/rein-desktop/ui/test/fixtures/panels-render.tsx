// Rendered to HTML by test/panels.test.mjs (E-60): the three panel views with text a project's
// `.ai/` files could hold — a script tag, an image with an onerror, a Markdown link — which
// must come out as text, never as markup.
import { renderToString } from "react-dom/server";
import { PanelBody } from "../../src/panel-host.tsx";
import { DESCRIPTORS } from "../../src/panels.ts";

const HOSTILE = "<script>alert(1)</script> [the blueprint](https://example.com/x) <img src=x onerror=alert(2)>";
const d = (id: string) => DESCRIPTORS.find((x) => x.id === id)!;
const app = { health: "OK", server: "rein-app's own" };
const NOW = Date.UTC(2026, 10, 16, 12, 0);
const task = (id: string, status: string, description: string) => ({
  id,
  status,
  owner: "engineer",
  tier: "T2",
  description,
  blocked_by: [],
  updated_at: "2026-11-16T10:00:00Z",
});

export function communication(): string {
  return renderToString(
    <PanelBody
      d={d("communication")}
      app={app}
      nowMs={NOW}
      reply={{
        read_at: NOW / 1000,
        ok: {
          cut: false,
          handoffs: [
            {
              timestamp: "2026-11-16 11:45 UTC",
              from: "Architect",
              to: HOSTILE,
              trigger: HOSTILE,
              cut: false,
              sections: [{ name: HOSTILE, text: HOSTILE }],
            },
          ],
        },
      }}
    />,
  );
}

export function tasks(): string {
  return renderToString(
    <PanelBody
      d={d("tasks")}
      app={app}
      nowMs={NOW}
      reply={{ read_at: NOW / 1000, ok: { cut: false, focus: "", tasks: [task("E-1", "OPEN", HOSTILE), task("E-2", "WAITING", "odd status")] } }}
    />,
  );
}

export function dashboard(): string {
  return renderToString(
    <PanelBody
      d={d("dashboard")}
      app={app}
      nowMs={NOW}
      reply={{
        read_at: NOW / 1000,
        ok: {
          counts: [{ status: "OPEN", n: 1 }],
          in_progress: task("E-3", "IN_PROGRESS", HOSTILE),
          last_handoff: { timestamp: "2026-11-16 11:45 UTC", from: "Architect", to: "Engineer", trigger: HOSTILE },
          whose_turn: HOSTILE,
          focus: HOSTILE,
        },
      }}
    />,
  );
}

export function unavailable(): string {
  return renderToString(<PanelBody d={d("tasks")} app={app} nowMs={NOW} reply={{ unavailable: "this project keeps tasks in a form this version does not read" }} />);
}

export function loading(): string {
  return renderToString(<PanelBody d={d("tasks")} app={app} nowMs={NOW} reply={null} />);
}
