---
name: orchestrate
description: Lead a feature or change in blip from idea to reviewed code. Clarify with the user, get design options from the designer for UI changes, have the requirements engineer write the requirements, get them approved, hand the work to the specialist agents (blip-dev, sonde-dev, outpost-dev, docs-writer) and loop with the reviewer and, for blip UI changes, the design-reviewer until requirements, code and look agree. Runs only when the user types /orchestrate.
argument-hint: <change>
disable-model-invocation: true
---

# Orchestrate a change

You are the lead. You talk to the user; the specialists are subagents that only see the
brief you give them. They cannot ask the user, and they do not see your memory or this
conversation. Put everything they need in the brief: requirement IDs, files, decisions
the user made, the design notes from your memory that apply, and what done looks like.

The change to build: $ARGUMENTS

## 1. Understand

1. Read what the change touches: the code, the docs pages, and the requirements in
   `doc/source/requirements`. Check your memory for design decisions about this area.
2. Ask the user until these are clear, with AskUserQuestion, a few questions at a time,
   each with a recommended option:
   - what should happen, from the user's view, and why;
   - which parts change (blip, proto, outpost, sonde, docs);
   - how we know it works: the acceptance criteria;
   - what is out of scope.
   Do not ask what you can find in the code or docs.
3. Sum up the agreed change in a few lines and get the user's OK.

## 2. Design (when the user will see the change)

Skip this step if the change does not alter the blip UI.

1. Spawn `designer` with the agreed summary, the views and panels it touches, and the
   user's decisions. Ask for a visual study in `design/` if the user wants one or the
   options are hard to judge from wireframes.
2. Show the user the options (name, wireframe, trade-offs) and the designer's
   recommendation, and let them choose with AskUserQuestion. Send follow-up questions
   to the same designer with SendMessage.
3. The chosen option goes into the brief of the requirements engineer and of `blip-dev`.
   If it sets a new design rule, add it to `design/DESIGN.md`.

## 3. Requirements

1. Spawn `requirements-engineer` with the agreed summary, the decisions, and the existing
   requirement IDs you found.
2. If it reports open points, ask the user and send the answers back with SendMessage to
   the same agent.
3. Show the user the new and changed requirements (ID, title, text) and ask them to
   approve them. When approved, set the `:status:` of the new and changed `REQ_*` and
   `STK_*` items to `approved`.

Do not start implementation before the requirements are approved.

## 4. Plan the work

Split the work by part and decide the order:

- A proto change goes first (`blip-dev`), then the outpost and sonde follow it.
- Parts that do not touch the same files may run in parallel: spawn them in one message.
  Two specialists never edit the same file at the same time.
- `doc/source/requirements/tests.rst` is shared: each developer only edits the `TEST_*`
  items of its own part.

Tell the user the plan in a few lines (who does what, in which order) before you start.

## 5. Implement

Spawn each specialist (`blip-dev`, `sonde-dev`, `outpost-dev`) with:

- the requirement IDs it implements and the `TEST_*` items it must fill in;
- the files and modules to work in, and what not to touch;
- the user's decisions and the acceptance criteria;
- what other parts are changing at the same time.

Read each report. If a specialist reports work needed in another part, add it to the
plan. Look at the diff yourself; a report is not proof.

## 6. Document

Spawn `docs-writer` with the requirement IDs, a summary of what was built, and the pages
likely affected. It updates the guides, internals, reference, `ARCH_*` items and
`changes.rst`, and regenerates diagrams and screenshots if needed.

## 7. Review

Every turn of an agent rereads all it has seen so far, so a long-running agent gets
more expensive with each message. Keep review rounds small: fresh agents with short
briefs, and checks of what changed, not the whole change again.

Before each round, save a snapshot of the working tree (it catches new untracked
files and does not touch the index):

```bash
GIT_INDEX_FILE="$(mktemp -u)" sh -c 'git read-tree HEAD && git add -A && git write-tree'
```

It prints a tree id; note it, since shell variables do not last between commands.
Later, run the same command again and `git diff <old id> <new id>` shows what changed
since that round. Put the old id in the briefs of the next round.

1. **First round.** Spawn `reviewer` with the requirement IDs in scope and a summary of
   the change. If the change touches `blip/src`, spawn `design-reviewer` in the same
   message, with the requirement IDs, the chosen design option and the views and panels
   that changed. The design reviewer renders its screenshots into a scratch directory,
   so the two do not get in each other's way. (The specialists have no Agent tool, so
   the reviewer cannot start the design reviewer; you run both.)
2. **Sort the findings.** Blocking and must-fix findings go to a fix round. Polish and
   Info findings do not start a round: collect them and give them to the user at the
   end, who decides.
3. **Fixes.** Spawn a fresh specialist for the fixes, not the agent that built the
   change. Its brief lists the findings, the files and functions they point to, the
   user's decisions and the requirement IDs. Design findings go to `blip-dev`, missing
   screenshots to `blip-dev` (`screenshots.rs`) and then `docs-writer`. Run fixes for
   different parts in parallel. Use SendMessage only for a short follow-up right after
   an agent's report, never to start a new round in a long-running agent.
4. **Re-check.** Spawn fresh reviewers on Sonnet (the Agent tool's `model: "sonnet"`).
   Brief them with the previous findings and the snapshot id to diff against. They
   check that those findings are fixed and that the fix broke nothing it touched, not
   the whole change again. The design reviewer looks at only the screenshots of the
   fixed findings.
5. **Last pass.** The final round runs the full checks once: the `reviewer` runs every
   check the change touches, and the `design-reviewer` renders all screenshots to see
   that nothing else changed. If the previous round's checks passed and only small
   fixes followed, fold this into the re-check.
6. Stop after two rounds that still have must-fix findings, and ask the user how to go
   on.

When the reviewer passes and the design reviewer says it looks good, set the `:status:`
of the requirements and test cases in scope to `implemented`. Leave `verified` to the
user.

## 8. Finish

The reviewer's last pass ran the checks on the finished change; trust its results and
do not run them again. Setting the statuses changed only the requirements, so run
`cd doc && make html` once more. If anything else changed after the reviewer's last
pass, send it back to the reviewer instead.

Look back at the review rounds. If the reviewers flagged the same kind of problem more
than once, in this change or one before it, propose a rule that would have prevented it
and where it goes: the agent file of the specialist that made it (`.claude/agents`),
`design/DESIGN.md` for a design rule, or `CLAUDE.md` for one every agent needs. Show the
user the exact text and add it only when they agree. The agents have no memory of their
own, so these files are where their lessons live.

Report to the user: what was built, per requirement ID, the reviewer's coverage table,
the design reviewer's verdict, the reviewer's check results, the polish and Info
findings you collected, and anything left open (e.g. diagrams that need the draw.io
app). Do not commit unless the user asks. If they do, use a short imperative subject
and a body that explains what changed and why, as `CLAUDE.md` says.
