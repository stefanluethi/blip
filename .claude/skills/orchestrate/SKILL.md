---
name: orchestrate
description: Lead a feature or change in blip from idea to reviewed code. Small changes take a light path (one specialist, one Sonnet review). For the rest, clarify with the user, get design options from the designer for UI changes and from the architect for changes to an API or the structure, have the requirements engineer write the requirements, get them approved, have a technical spec written on Opus, hand the slices of work on Sonnet to the specialist agents (blip-dev, sonde-dev, outpost-dev, docs-writer) and loop with the reviewer and, for blip UI changes, the design-reviewer until requirements, code and look agree. Runs only when the user types /orchestrate.
argument-hint: <change>
disable-model-invocation: true
---

# Orchestrate a change

You are the lead. You talk to the user; the specialists are subagents that only see the
brief you give them. They cannot ask the user, and they do not see your memory or this
conversation. Put everything they need in the brief: requirement IDs, files, decisions
the user made, the design notes from your memory that apply, and what done looks like.

**Keep your own context small.** Every turn of yours rereads this whole conversation,
and you take more turns than any specialist, so what you load costs the most:

- Do not read code, diffs or long reports that an agent can read instead. What a later
  agent needs goes into a file in `.claude/specs`, not through you.
- Ask every agent for a short report (ten lines at most, see step 6); the details go to
  the log file.
- SendMessage wakes an agent with all it has read. Use it only within a few minutes of
  the agent's report and only for a short follow-up. Later, or when the agent has read
  a lot (an architect, a spec author, a reviewer), spawn a fresh agent and name the
  files the first one wrote.

The change to build: $ARGUMENTS

## 1. Understand

1. Read what the change touches: the code, the docs pages, and the requirements in
   `doc/source/requirements`. Check your memory for design decisions about this area,
   and the repository's list of open points (`OPEN_POINTS.md`), if there is one, for
   what an earlier change left open here. If `.claude/specs/<change>.md` exists and
   says the requirements are approved, an earlier session started this change: read
   that file and go on from step 4.
2. Ask the user until these are clear, with AskUserQuestion, a few questions at a time,
   each with a recommended option:
   - what should happen, from the user's view, and why;
   - which parts change (blip, proto, outpost, sonde, docs);
   - how we know it works: the acceptance criteria;
   - what is out of scope.
   Do not ask what you can find in the code or docs.
3. Sum up the agreed change in a few lines and get the user's OK. On the full path,
   write it to `.claude/specs/<change>.md` under **Agreed**, as numbered points: what
   happens, the acceptance criteria, what is out of scope. Add the user's later
   decisions (the chosen design, answers to open points) to it as they come. Every
   brief names this file, the reviewer checks the change against its points, and your
   final report goes through them one by one. What was agreed and is not in a
   requirement gets lost otherwise.
4. Pick the path (see "Light path" below) and tell the user which one and why, in the
   same message as the summary. The user can ask for the full path.

## Light path

Most of the cost of a change is in the agents that run for many turns, so a small change
does not get the whole team. Take the light path when all of these hold:

- one part changes (blip, proto, outpost or sonde), in a few files, about 150 lines of
  code or less;
- no new view, panel or control, and no design choice to make with the user;
- no change to the protocol, the recording format or a saved file format;
- no change to the public API of `blip-kit` or `blip-proto`, and no code that moves
  between modules or layers;
- at most a few requirements are added or changed.

Then:

1. **Requirements.** If the change adds or changes a requirement, spawn
   `requirements-engineer` and get the user's approval as in step 3. If it only fixes the
   code of existing requirements, name their IDs and skip this.
2. **Build.** Spawn the part's specialist once on its default model. Its brief is the
   spec in short (the sections of step 5), and names the docs pages it updates as well:
   the guide or reference page the change touches and the `changes.rst` entry. Skip the
   designer, the spec file, the simplify pass and the `docs-writer`.
3. **Review.** Spawn `reviewer` on Sonnet (`model: "sonnet"`, `effort: "medium"`) as the
   last pass, so it runs the checks once. If the change touches the look of `blip/src` or `kit/src`,
   spawn `design-reviewer` in the same message.
4. **Fix and finish** as in steps 8 and 9.

If the change turns out larger while you work (the specialist reports work in another
part, or the diff grows past the limits above), tell the user and go on with the full
path from the step you are at.

## 2. Design

A change may need one of the two, both or none.

### The look (when the user will see the change)

Skip this if the change does not alter the blip UI.

1. Spawn `designer` with the agreed summary, the views and panels it touches, and the
   user's decisions. Ask for a visual study in `design/` if the user wants one or the
   options are hard to judge from wireframes.
2. Show the user the options (name, wireframe, trade-offs) and the designer's
   recommendation, and let them choose with AskUserQuestion. A short follow-up right
   after the report goes to the same designer with SendMessage; a later one goes to a
   fresh designer.
3. The chosen option goes into the brief of the requirements engineer and of `blip-dev`.
   If it sets a new design rule, add it to `design/DESIGN.md`.

### The architecture (when the structure changes)

Do this when the change alters the public API of `blip-kit` or `blip-proto`, moves a
boundary between modules or layers, or changes the protocol or a file format. Skip it
when the change only fills in behind the interfaces that exist.

1. Spawn `architect` with the agreed summary, the modules it touches, what the user
   said about the direction, and the file to write:
   `.claude/specs/<change>-architecture.md`. If the user has chosen the direction
   already, say so: the options are then the ways to build it.
2. Show the user the options (name, the signatures, the consequences for speed, kept
   files and the other parts) and the architect's recommendation, and let them choose
   with AskUserQuestion. When the user's answer changes the proposal, spawn a fresh
   architect with the architecture file and the answer, and have it update the file.
   Do not wake the first one: it has read half the code base, and the talk with the
   user took longer than its cache lasts.
3. Add the chosen option to **Agreed**. It goes into the brief of the requirements
   engineer, who writes what an application or a user sees of it and updates the
   `ARCH_*` elements, and it is the start of the specs in step 5: they build the
   chosen structure and do not choose it again. If it sets a rule for the structure
   (what a layer may use), add it to `CLAUDE.md`.

## 3. Requirements

1. Spawn `requirements-engineer` with the agreed summary, the decisions, and the existing
   requirement IDs you found.
2. If it reports open points, ask the user and send the answers back with SendMessage to
   the same agent.
3. Show the user the new and changed requirements (ID, title, text) and ask them to
   approve them. When approved, set the `:status:` of the new and changed `REQ_*` and
   `STK_*` items to `approved`.

Do not start implementation before the requirements are approved. The approval is the
user's: do not take silence, or their OK to the summary, for it.

Once they are approved, write the requirement IDs in scope into the **Agreed** file and
that they are approved. If the way here was long (a design or an architecture to
discuss), offer the user to build in a fresh session with `/orchestrate <change>`: the
files hold all the build needs, and a fresh lead does not reread the discussion on
every turn of the build.

## 4. Plan the work

Split the work by part and decide the order:

- A proto change goes first (`blip-dev`), then the outpost and sonde follow it.
- Parts that do not touch the same files may run in parallel: spawn them in one message.
  Two specialists never edit the same file at the same time.
- The test pages in `doc/source/requirements/tests/` are shared: each developer only
  edits the `TEST_*` items of its own part.

- Build on a branch, in a worktree of its own when other sessions work on blip
  (`git worktree list`, `git status`). A change built in the main checkout while
  another lands on `main` ends in a merge that costs more than a slice.

Tell the user the plan in a few lines (who does what, in which order) before you start.

## 5. Spec

The requirements say what to build, not how. A spec settles the how, so the code can be
written on Sonnet in small slices, and Opus spends its tokens once on the decisions
instead of on every line.

1. For each part, spawn its specialist on its default model (Opus) to write the spec,
   not code, to `.claude/specs/<change>-<part>.md` and one file per slice (see
   below). Its brief: the requirement IDs, the
   chosen design option, the user's decisions, and the outline below. It reads the code
   it will change and may run commands to work out test values, but edits no other
   file.
2. Read the spec. Check that every requirement and `TEST_*` item in scope is in a slice,
   and that it decides rather than lists options. Ask the user only about decisions that
   change what they see or what loads from old files. Write an answer of a line or two
   into the spec yourself; for more, spawn a fresh specialist with the spec files and
   the answers.

Keep the spec short: it decides and names, it does not hold the code. No function
bodies, only signatures and, where the logic is not plain, the steps in words. The
shared sections (Decisions, Files, Types and functions, Not to touch) go to
`.claude/specs/<change>-<part>.md`, about 300 lines at most. Each slice gets its own
file, `.claude/specs/<change>-<part>-slice-<n>.md`, with its part of Slices, Context
and Tests, about 100 lines. The author writes these files one at a time: one long
output takes minutes, the prompt cache runs out meanwhile, and all it has read is paid
for again.

The spec has these sections:

- **Decisions:** each choice the code needs that the requirements leave open (names,
  compatibility of saved files, the drawing approach, which existing code to reuse), with
  the choice and one line of why.
- **Files:** new and changed files and modules, and what each holds.
- **Types and functions:** new and changed signatures, with a line each on what they
  do; existing functions to reuse, named by file and function.
- **Slices:** in order, each one buildable and checked on its own, about 600 lines of
  code at most, and as few as that allows (a slice pays for the start of its agent,
  its builds and your turns around it): the requirement IDs, the files, the steps, and what done looks like.
  The workspace builds and its tests pass after every slice: a slice that changes a
  signature also changes its callers, or keeps the old one beside the new until the
  slice that removes it. Per slice, the checks its agent runs (the tests and the
  builds of the crates it touches); the last slice of a part runs the part's full
  checks.
- **Context:** per slice, what its agent reads before it starts: the sections of the docs
  (`internals.rst`, `sonde/README.md`, by heading) and the code (file and function or line
  range). A slice agent reads these instead of whole files, so keep the list short.
- **Tests:** per slice, the test names, the `TEST_*` item each fills, and the inputs
  and expected values worked out (by running the numbers), not guessed. Also the
  existing tests that change, each with the reason: the behaviour changed by a
  requirement, or only the signature it calls. No other test is changed or removed.
- **Not to touch:** what stays as it is.

Skip the file for a small change, one slice in a few files: then the brief is the spec,
with the same sections in short.

## 6. Implement

Spawn a fresh specialist per slice, in the spec's order, on Sonnet at medium effort (the
Agent tool's `model: "sonnet"` and `effort: "medium"`): the spec has made the decisions,
so the slice needs no deep reasoning. Slices of different parts that do not share files
may run in parallel. Each brief has:

- the shared spec file and the file of its slice; the slices before it are done. Do
  not copy the spec into the brief;
- the log file, `.claude/specs/<change>-log.md`: the agent appends its full report
  there and returns ten lines at most (built or not, the result of each check, where
  it left the spec, what is open);
- the checks to run, from the spec: a slice in the middle runs the tests and builds of
  what it touched, not `./check.sh`, which builds everything and takes minutes; the
  last slice of a part runs the part's full checks;
- the requirement IDs it implements and the `TEST_*` items it must fill in;
- what other parts are changing at the same time.

A slice the spec leaves open, or one that failed twice on Sonnet, goes to the
specialist on its default model. If a specialist reports that the spec does not fit the
code, have the spec fixed before the next slice.

The developers stop after 80 turns (`maxTurns`) and return what they have. A slice that
hits the limit was too large: resume it once with SendMessage to finish it, and split the
slices that follow smaller. Do not lift the limit.

Read each report. If a specialist reports work needed in another part, add it to the
plan. A report is not proof, but do not read the diff yourself: take a snapshot (step
8) before the first slice, and after each slice look at `git diff --stat` against the
snapshot before it. Files outside the slice's list, or a slice far larger or smaller
than the spec says, are what to look into; the reviewers read the code.

**Measure.** If the change touches a path that runs all the time (how the store takes
in samples, the queries of the widgets, the drawing of a plot), run the baseline
yourself before the first slice and after the last, on the same machine with nothing
else building, and keep both results for the reviewer and the report:

```bash
cargo test --release -p blip-kit --test baseline -- --ignored --nocapture
```

If the baseline itself has to change with an API, keep what it measures the same. A
result more than about 10 % worse goes to the specialist on its default model before
the review, with both numbers.

**Simplify.** When a part's change adds more than about 200 lines of code, spawn a
fresh specialist of that part on Sonnet at medium effort to simplify it before the
docs and the review: smaller code is cheaper for every agent that reads it later. The
`simplify` skill spreads its work over agents, which a specialist cannot start, so the
brief asks for one pass instead: repeated code to merge, code the change left unused,
and work done twice on a path that runs all the time. Its brief: the files of the
change (`git diff --stat` against the first snapshot), keep the behaviour and the
tests the same, run the part's checks (`./check.sh` for blip and proto), and report
what it removed or merged. Skip this for smaller changes.

## 7. Document

Spawn `docs-writer` with the requirement IDs, a summary of what was built, and the pages
likely affected. It updates the guides, internals, reference, `ARCH_*` items and
`changes.rst`, and regenerates diagrams and screenshots if needed. For a change of the
structure, name the diagrams in `doc/source/diagrams` that show the modules that moved
and the pages of `doc/source/library` that show the changed signatures.

The docs follow the code to the end: if a fix round in step 8 changes an interface or
behaviour that the docs describe, the `docs-writer` runs again in that round with the
fix's findings, before the re-check.

## 8. Review

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

1. **First round.** If `main` moved since the branch started
   (`git log --oneline HEAD..main`), bring it in first (rebase or merge, the part's
   specialist resolves conflicts), so the review sees what will land. Then spawn two
   reviewers in one message, each with the requirement IDs in scope, a summary of the
   change, the file with the **Agreed** points and the log file:
   - `reviewer` on its default model with the scope **correctness** (its sections 1
     and 2) and, if you measured, the two baseline results. It runs no checks.
   - `reviewer` on Sonnet at medium effort (`model: "sonnet"`, `effort: "medium"`)
     with the scope **coverage** (its sections 3 and 4): the coverage table, the
     traceability, the docs and the checks. This is looking up and running, not
     judging, and only this one builds.

   If the change touches `blip/src` or `kit/src`, spawn `design-reviewer` in the same
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
   different parts in parallel. Fixes the findings spell out run on Sonnet at medium
effort (`model: "sonnet"`, `effort: "medium"`); a finding
   that needs a design decision goes to the specialist on its default model. Use
   SendMessage only for a short follow-up right after an agent's report, never to start
   a new round in a long-running agent.
4. **Re-check.** Spawn fresh reviewers on Sonnet at medium effort (the Agent tool's
   `model: "sonnet"` and `effort: "medium"`).
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
of the requirements and test cases in scope to `implemented`. A `STK_*` item becomes
`implemented` only when every requirement that satisfies it is. Leave `verified` to the
user.

## 9. Finish

The reviewer's last pass ran the checks on the finished change; trust its results and
do not run them again. Setting the statuses changed only the requirements, so run
`cd doc && make html` once more. If anything else changed after the reviewer's last
pass, send it back to the reviewer instead.

Look back at the review rounds. If the reviewers flagged the same kind of problem more
than once, in this change or one before it, propose a rule that would have prevented it
and where it goes: the agent file of the specialist that made it (`.claude/agents`),
`design/DESIGN.md` for a design rule, or `CLAUDE.md` for one every agent needs. Show the
user the exact text and add it only when they agree. The agents have no memory of their
own, so these files are where their lessons live. A new or changed agent file is
loaded when a session starts: tell the user that it takes effect in the next session,
and do not spawn an agent you added in this one.

Report to the user: what was built, per requirement ID, the **Agreed** points one by
one (built, built in part, or not built, and why), the reviewer's coverage table,
the design reviewer's verdict, the reviewer's check results, the baseline before and
after if you measured, the polish and Info findings you collected, what nobody has
looked at or run (a window, the browser, a target), and anything left open (e.g.
diagrams that need the draw.io app). Do not commit unless the user asks. If they do, use a short imperative subject
and a body that explains what changed and why, as `CLAUDE.md` says. The specs in
`.claude/specs` are working files: once the code and docs describe the change, delete
them unless the user wants to keep them. If the repository has a list of open points
(`OPEN_POINTS.md`), tell the user which of its points the change closed and which it
adds, and change the file only when they agree.
