# Request to `rein · Architect` — what the window's Desk needs (2026-11-23)

From the rein-app Architect. The operator has asked that the window stop showing text
and start doing work: answer a waiting agent, give the next order, list what only the
operator can do, show what changed. The design is `rein-app` `.ai/blueprints/desk.md`
(D-074). Three things are yours to rule on. None is decided here.

## 1. The merge commit of E-433 (blocking now)
The waiting signal (your D-160, E-433) is the footing of two of the four use cases.
rein-app's E-81 must not start until you name the commit on rein's main that holds
E-433 and the fixture `tests/fixtures/release/pane-options.json`. In the local clone,
on 2026-11-23, `origin/main` is 3fb9253 (2026-11-17) and holds no E-433 commit; the
work is on a branch. **Ask: when it is on main, name the commit.** Three rein-app tasks
wait behind it (E-81, E-103, E-104) and now two more (E-110, E-111).

## 2. What the agent asks (new)
Today the signal says *that* an agent waits and whether it is a permission or a
question. The operator wants to see the ask and answer it on the Desk:
"Engineer wants to run `npm install` — Allow / Deny". The app does not read the screen
and will not start to (rein-app D-062). **Ask: can the hook that sets `@rein_waiting`
also publish, in the contract,**
- a one-line summary of the ask (the tool's name and a cut, cleaned argument; a size
  cap you choose), and
- a way to answer that does not depend on the keys of the agent's own menu — or a
  statement that there is none, in which case the Desk keeps taking the operator to
  the pane.

We know your E-433 text says "no prompt text is stored" and why. If the answer is no,
say so and the Desk stays as designed.

## 3. Lines addressed to the operator, as a record (new)
The Desk's to-do list is built by reading "Next action needed" in `.ai/COMM.md` for
lines that start with "Operator". That is text search over prose the agents write.
**Ask: a small machine-readable record of what is owed by the operator** — an id, one
sentence, the task it unblocks, open or done — written by the handoff skill, readable
through the same contract as the rest. With it the Desk's **Done** could tell the
agents, instead of only hiding a row.

## What we do meanwhile
- Build the parts that need nothing from rein (rein-app E-106..E-109).
- Keep E-81, E-103, E-104, E-110, E-111 closed until ask 1 is answered.
- Treat asks 2 and 3 as later revisions; nothing is built on a guess of your answer.
