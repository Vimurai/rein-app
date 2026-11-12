// Rendered to HTML by test/components.test.mjs (E-42): one Dialog, one Tooltip and the Command
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
          <Button>[ Cancel ]</Button>
          <Button variant="primary">[ Update to 0.5.1 ]</Button>
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
