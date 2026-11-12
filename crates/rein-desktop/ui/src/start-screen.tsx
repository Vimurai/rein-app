// The start screen in the modern look (desktop.md §Look and feel v2 V2, E-50): a centred column
// — the wordmark, a primary `Open a project…`, the recent projects as rows (number, folder
// icon, name, dim path, last opened), a secondary `Check installation`, the WSL setup card on
// Windows when needed, and the key hints. What it shows comes from the pure `startScreen`.

import { Button, Kbd } from "../components/button.tsx";
import { Skeleton, Spinner } from "../components/loaders.tsx";
import { Tooltip, TooltipContent, TooltipTrigger } from "../components/tooltip.tsx";
import type { Runner } from "./frame.tsx";
import { TruncStart } from "./frame.tsx";
import { Folder, FolderOpen, ShieldCheck, TriangleAlert } from "./icons.ts";
import type { StartScreen as Model } from "./start.ts";

export function StartScreen({ model, r }: { model: Model; r: Runner }) {
  return (
    <div id="start" className="flex h-full justify-center overflow-y-auto px-6 font-ui text-[13px] text-foreground">
      <div className="my-auto flex w-full max-w-[560px] flex-col gap-6 py-10">
        <div className="flex items-center gap-3">
          <span aria-hidden="true" className="flex size-10 items-center justify-center rounded-lg bg-primary text-[20px] font-bold text-primary-foreground">
            r
          </span>
          <span className="grid">
            <span className="text-[22px] leading-7 font-semibold tracking-tight">rein</span>
            <span className="text-[12px] text-muted-foreground">Open a project to start its Architect and Engineer.</span>
          </span>
        </div>

        {model.setup && (
          <div role="alert" className="grid gap-3 rounded-lg border border-destructive bg-card p-4 text-card-foreground">
            <div className="flex items-start gap-2">
              <TriangleAlert size={16} aria-hidden="true" className="mt-px shrink-0 text-destructive" />
              <span className="font-medium">{model.setup.reason}</span>
            </div>
            <div className="grid gap-1">
              <span className="text-[12px] text-muted-foreground">Run in an administrator terminal:</span>
              <code className="rounded-md border border-border bg-background px-2 py-1.5 font-mono text-[12px] text-foreground">
                {model.setup.command}
              </code>
            </div>
            <div aria-busy={model.setup.checking || undefined}>
              {model.setup.checking ? (
                <span className="flex h-7 items-center gap-2 text-[12px] text-muted-foreground">
                  <Spinner size={12} />
                  <span role="status">Checking WSL…</span>
                </span>
              ) : (
                <Button size="sm" aria-label={r.tip(model.setup.again)} onClick={() => r.run(model.setup!.again)}>
                  Check again
                  <Kbd>c</Kbd>
                </Button>
              )}
            </div>
          </div>
        )}

        <div className="flex flex-wrap gap-2">
          <Button variant="primary" aria-label={r.tip(model.open.action)} onClick={() => r.run(model.open.action)}>
            <FolderOpen size={15} aria-hidden="true" />
            {model.open.label}
          </Button>
          <Button aria-label={r.tip(model.check.action)} onClick={() => r.run(model.check.action)}>
            <ShieldCheck size={15} aria-hidden="true" />
            {model.check.label}
          </Button>
        </div>

        <section aria-label="Recent projects" aria-busy={model.loading || undefined} className="grid gap-2">
          <h2 className="m-0 text-[12px] font-medium text-muted-foreground">Recent projects</h2>
          {model.loading ? (
            <div className="grid gap-0.5">
              {[0, 1, 2].map((i) => (
                <div key={i} className="flex h-10 items-center gap-3 px-2">
                  <Skeleton className="size-5" />
                  <Skeleton className="h-3.5 w-28" />
                  <Skeleton className="h-3 flex-1" />
                  <Skeleton className="h-3 w-14" />
                </div>
              ))}
              <span role="status" className="sr-only">
                Loading recent projects…
              </span>
            </div>
          ) : model.recent.length === 0 ? (
            <p className="m-0 rounded-lg border border-dashed border-border p-4 text-muted-foreground">
              No recent projects yet. Open one and it appears here.
            </p>
          ) : (
            <ul className="m-0 grid list-none gap-0.5 p-0">
              {model.recent.map((row) => {
                const tip = r.tip(row.action);
                return (
                  <li key={row.path}>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <button
                          type="button"
                          aria-label={tip}
                          onClick={() => r.run(row.action)}
                          className={
                            "flex h-10 w-full cursor-pointer items-center gap-3 rounded-md px-2 text-left outline-none transition-colors duration-[var(--motion-base)] hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring " +
                            (row.missing ? "opacity-60" : "")
                          }
                        >
                          <Kbd className="shrink-0">{row.n}</Kbd>
                          <Folder size={15} aria-hidden="true" className="shrink-0 text-muted-foreground" />
                          <span className="shrink-0 font-medium">{row.name}</span>
                          <TruncStart text={row.path} className="flex-1 font-mono text-[11px] text-muted-foreground" />
                          <span className="shrink-0 text-[12px] text-muted-foreground tabular-nums">{row.missing ? "missing" : row.ago}</span>
                        </button>
                      </TooltipTrigger>
                      <TooltipContent>{tip}</TooltipContent>
                    </Tooltip>
                  </li>
                );
              })}
            </ul>
          )}
        </section>

        <div className="flex flex-wrap gap-x-4 gap-y-2 text-[12px] text-muted-foreground">
          {model.hints.map(([k, what]) => (
            <span key={k} className="flex items-center gap-1.5">
              <Kbd>{k}</Kbd>
              {what}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}
