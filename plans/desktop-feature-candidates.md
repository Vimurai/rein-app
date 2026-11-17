# Window feature candidates (2026-11-17) — NOT decided, nothing registered

Operator asked on 2026-11-17 for "more features our app will benefit a lot from". These
are the Architect's candidates in the order recommended. Each becomes a blueprint
revision and tasks only when the operator picks it. Already planned and not listed:
project status, notifications, readable panels (`status.md`, E-74..E-79); sessions that
outlive the app (V12, E-69..E-71).

| # | Feature | What the operator gets | Needs |
|---|---|---|---|
| 1 | **PLANNED 2026-11-17 → D-062 addendum 1, E-81 (rein said yes: rein D-160, E-433; waits on rein's merge).** **"Waiting for you" as a fact** | The tab and the notification say an agent is at a permission prompt or asked a question, per role, also for a session running while the app was closed | rein: the waiting signal (request sent 2026-11-17). Small on our side once it exists |
| 2 | **CHOSEN 2026-11-17 → D-064, E-80 ("Home").** **All-projects overview** (the start screen becomes it) | One screen: every open and recent project with status, task in progress, whose turn, last handoff, what needs you; open or go to a project from it | Nothing new — reuses the status of D-062 |
| 3 | **"While you were away"** | On returning to a project: what happened since you last looked — handoffs, tasks finished, tasks blocked, agents ended — as one short list | A "last seen" time per project in the app's state; reuses the sources |
| 4 | **Quick switcher** (`Ctrl+K`) | Type to jump to a project, a panel, a task or an action | Nothing new |
| 5 | **Reviews and decisions panels** | Read critic stamps (`REVIEWS.md`) and decisions (`DECISIONS.md`) in the reader, with a filter | Two new read-only sources (Tier 3 each: new file reads) |
| 6 | **Cost and context per agent** | Tokens / cost of the session and how full each agent's context is | rein: a source for it (not in the contract) |
| 7 | **CI status per task** | The Dashboard and the tab show red / green for the last commit | rein: `get_ci_status` in the read interface (already asked, item 3) |
| 8 | **Reply from the panel** | Answer a waiting agent or send it a line without finding the pane | Reverses the rule "no panel action types into a terminal" — the operator's decision, Tier 3 |
| 9 | **Notifications on a phone** | A push when a project needs you and you are away | Network from the app (none today) — Tier 3, a service to choose |
| 10 | **Signed installers and self-update of the window** | No "unknown publisher" warning; the window updates itself | Operator: signing certificates (pending since D-048) |

Recommendation: 2 and 3 next (no dependency on anyone, and they are the same question the
operator asked today — "what needs me" — answered across projects and across time), then
4. Item 1 as soon as rein answers. Items 8 and 9 only on an explicit operator decision.
