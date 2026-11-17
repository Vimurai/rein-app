// Structured text (status.md §Components 7, D-063; E-74): the blocks of `panel-text.ts` as
// elements with text children only. The element names are a closed set (STRUCTURED_TAGS, a
// test checks the rendered DOM against it): no anchor, no image, no `href` / `src`, no raw HTML.

import type { ReactNode } from "react";
import { blocks, inline, type Inline } from "./panel-text.ts";

/** Every element this component can make. */
export const STRUCTURED_TAGS = ["div", "p", "h4", "ul", "ol", "li", "pre", "code", "strong", "span"] as const;

function Inl({ parts }: { parts: Inline[] }): ReactNode {
  return parts.map((p, i) =>
    p.kind === "code" ? (
      <code key={i} className="rounded bg-muted px-1 py-px font-mono text-[12px]">
        {p.text}
      </code>
    ) : p.kind === "bold" ? (
      <strong key={i} className="font-semibold">
        {p.text}
      </strong>
    ) : (
      <span key={i}>{p.text}</span>
    ),
  );
}

/** One line (a title, a trigger) with its `code` and **bold**, no blocks. */
export function InlineText({ text }: { text: string }) {
  return <Inl parts={inline(text)} />;
}

/** `text` drawn as paragraphs, lists, heading lines and code blocks. */
export function StructuredText({ text, className = "" }: { text: string; className?: string }) {
  return (
    <div className={`grid gap-2 [overflow-wrap:anywhere] ${className}`}>
      {blocks(text).map((b, i) => {
        switch (b.kind) {
          case "p":
            return (
              <p key={i} className="m-0">
                <Inl parts={b.inl} />
              </p>
            );
          case "heading":
            return (
              <h4 key={i} className="m-0 font-semibold">
                <Inl parts={b.inl} />
              </h4>
            );
          case "ul":
            return (
              <ul key={i} className="m-0 grid list-disc gap-1 pl-5">
                {b.items.map((it, k) => (
                  <li key={k}>
                    <Inl parts={it} />
                  </li>
                ))}
              </ul>
            );
          case "ol":
            return (
              <ol key={i} start={b.start} className="m-0 grid list-decimal gap-1 pl-6">
                {b.items.map((it, k) => (
                  <li key={k}>
                    <Inl parts={it} />
                  </li>
                ))}
              </ol>
            );
          case "pre":
            return (
              <pre key={i} className="m-0 overflow-x-auto rounded-md bg-muted p-2 font-mono text-[12px] leading-normal [overflow-wrap:normal]">
                <code>{b.text}</code>
              </pre>
            );
        }
      })}
    </div>
  );
}
