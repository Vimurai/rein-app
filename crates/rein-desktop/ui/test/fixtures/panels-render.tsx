// Rendered to HTML by test/panels.test.mjs (E-60; "Needs you" added by E-61): the panel views with text a project's
// `.ai/` files could hold — a script tag, an image with an onerror, a Markdown link — which
// must come out as text, never as markup.
import { renderToString } from "react-dom/server";
import { PanelBody } from "../../src/panel-host.tsx";
import { Reader } from "../../src/panel-reader.tsx";
import { DESCRIPTORS } from "../../src/panels.ts";
import { StructuredText } from "../../src/structured-text.tsx";
export { STRUCTURED_TAGS } from "../../src/structured-text.tsx";
export { taskChips } from "../../src/panel-reader.tsx";

const HOSTILE = "<script>alert(1)</script> [the blueprint](https://example.com/x) <img src=x onerror=alert(2)>";
// Eight lines: the entry shows six, then "…".
const NEXT = ["- **Engineer:** E-74 next.", "- one", "- two", "- three", "- four", "- five", "- six (left out)", "- seven"].join("\n");
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
              sections: [
                { name: HOSTILE, text: HOSTILE },
                { name: "Next action needed", text: NEXT },
              ],
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

type Lists = import("../../src/attention.ts").Lists;
type Item = import("../../src/attention.ts").Item;
const feed = (lists: Lists) => ({ lists, title: (i: number) => ["alpha", "beta"][i] ?? "?", readAt: NOW / 1000, goTo: () => {}, refresh: () => {}, dismiss: () => {} });
const it = (over: Partial<Item>): Item => ({ key: "k", kind: "bell", index: 0, tab: 10, text: "The terminal rang its bell.", quote: null, list: [], dismissable: false, ...over });

export function attention(): string {
  return renderToString(
    <PanelBody
      d={d("attention")}
      app={app}
      nowMs={NOW}
      reply={null}
      attention={feed({
        now: [
          it({ key: "health", kind: "health_fail", index: null, tab: null, text: "The installation health check is failing." }),
          it({ key: "bell", dismissable: true }),
          it({ key: "ended", kind: "ended", index: 1, tab: 11, text: "rein start has ended in this tab." }),
          it({ key: "mention:0", kind: "mention", index: 1, tab: 11, text: "The last handoff mentions the operator:", quote: HOSTILE, dismissable: true }),
        ],
        info: [
          it({ key: "blocked", kind: "blocked", text: "12 tasks are blocked", list: [{ id: "E-1", description: HOSTILE }] }),
          it({ key: "update", kind: "update", index: 1, tab: 11, text: "rein says the project is out of date." }),
        ],
      })}
    />,
  );
}

export function attentionInfoOnly(): string {
  return renderToString(
    <PanelBody d={d("attention")} app={app} nowMs={NOW} reply={null} attention={feed({ now: [], info: [it({ key: "blocked", kind: "blocked", text: "1 task is blocked" })] })} />,
  );
}

export function attentionEmpty(): string {
  return renderToString(<PanelBody d={d("attention")} app={app} nowMs={NOW} reply={null} attention={feed({ now: [], info: [] })} />);
}

export function attentionLoading(): string {
  return renderToString(<PanelBody d={d("attention")} app={app} nowMs={NOW} reply={null} attention={{ ...feed({ now: [], info: [] }), readAt: null }} />);
}

// E-74: every construct of the structured text, with the hostile strings inside each one.
export const RICH = [
  "# A heading " + HOSTILE,
  "",
  "A paragraph with `code " + HOSTILE + "` and **bold " + HOSTILE + "** and an unclosed ` and ** marker.",
  "It goes on here. https://example.com/raw stays text. ![img](https://example.com/i.png)",
  "",
  "- a bullet " + HOSTILE,
  "  continued under it",
  "- another",
  "",
  "3. third",
  "4. fourth " + HOSTILE,
  "",
  "```",
  "fenced " + HOSTILE,
  "  kept as written",
  "```",
  "```",
  "an unclosed fence is a line of text",
].join("\n");

export function structured(): string {
  return renderToString(<StructuredText text={RICH} />);
}

const longTask = task("E-74", "OPEN", "Make the side panels readable, per status.md §Components 7 (D-063). " + RICH);

export function readerTask(): string {
  return renderToString(
    <Reader
      items={[{ key: "task:E-74", kind: "task", task: { ...longTask, blocked_by: ["E-69"] } }]}
      index={0}
      modal={false}
      nowMs={NOW}
      left={400}
      onIndex={() => {}}
      onClose={() => {}}
    />,
  );
}

export function readerHandoff(): string {
  return renderToString(
    <Reader
      items={[
        { key: "handoff:0", kind: "task", task: longTask },
        {
          key: "handoff:1",
          kind: "handoff",
          entry: { key: "1", title: "Architect → Engineer", when: "2026-11-16 11:45 UTC · 15 min ago", trigger: HOSTILE, cut: true, sections: [{ name: "Next action needed", text: RICH }] },
        },
      ]}
      index={1}
      modal={true}
      nowMs={NOW}
      left={400}
      onIndex={() => {}}
      onClose={() => {}}
    />,
  );
}
