// Rendered to HTML by test/components.test.mjs (E-42, E-50): one Dialog, one Tooltip and the Command
// list, each open, without a portal so the server render includes them.
import { renderToString } from "react-dom/server";
import { Command, CommandGroup, CommandInput, CommandItem, CommandList, CommandShortcut } from "../../components/command.tsx";
import { Dialog, DialogContent, DialogDescription, DialogFooter } from "../../components/dialog.tsx";
import { Button } from "../../components/button.tsx";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "../../components/tooltip.tsx";

export function dialog(): string {
  return renderToString(
    <Dialog open>
      <DialogContent portal={false} title="Update Rein">
        <DialogDescription>rein-app update --yes</DialogDescription>
        <DialogFooter>
          <Button>Cancel</Button>
          <Button variant="primary">Update to 0.5.1</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>,
  );
}

export function tooltip(): string {
  return renderToString(
    <TooltipProvider>
      <Tooltip open>
        <TooltipTrigger>[≡]</TooltipTrigger>
        <TooltipContent portal={false}>Menu  Ctrl+Shift+P</TooltipContent>
      </Tooltip>
    </TooltipProvider>,
  );
}

export function command(): string {
  return renderToString(
    <Command>
      <CommandInput placeholder="Type an action" />
      <CommandList>
        <CommandGroup heading="Actions">
          <CommandItem>Open a project…<CommandShortcut>Ctrl+Shift+O</CommandShortcut></CommandItem>
          <CommandItem>Check installation health<CommandShortcut>Space h</CommandShortcut></CommandItem>
        </CommandGroup>
      </CommandList>
    </Command>,
  );
}

// E-54 (desktop.md V7 "Tests (V7)"): the start screen and a project tab's frame, idle and busy,
// for the "an idle window runs no animation" check.
import { StartScreen } from "../../src/start-screen.tsx";
import { StatusBar, TitleBar } from "../../src/frame.tsx";
import { startScreen } from "../../src/start.ts";
import { statusBar, titleBar } from "../../src/chrome.ts";

const runner = { run: () => {}, tip: (c: { name: string }) => c.name };
const recent = [{ path: "/home/me/Dev/p", name: "p", last_opened: 1_800_000_000 - 7200, exists: true }];
const ok = { status: "ok" as const, checks: [], reason: null, setup: null, taken_at: 0 };

export function start(busy: boolean): string {
  const model = startScreen(busy ? [] : recent, null, 1_800_000_000, { loading: busy });
  return renderToString(
    <TooltipProvider>
      <StartScreen model={model} r={runner} />
    </TooltipProvider>,
  );
}

export function tab(busy: boolean): string {
  const bar = titleBar([{ title: "p", current: true, exited: false, starting: busy }], "0.5.0");
  const status = statusBar("/home/me/Dev/p", ok, "cd ~/Dev/p && rein start", null, { starting: busy, health: busy });
  return renderToString(
    <TooltipProvider>
      <TitleBar bar={bar} chrome="linux" r={runner} />
      <StatusBar bar={status} r={runner} />
    </TooltipProvider>,
  );
}
