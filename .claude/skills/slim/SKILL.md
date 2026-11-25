---
name: slim
description: Shrink the code base of blip while keeping what it does. Surveys one part (blip, kit, proto, outpost, sonde) or one module for repeated, unused, speculative and long-hand code and for repeated tests, lets the user decide on what changes behaviour or the public API, then removes and merges in small batches, each checked, measured with tokei and committed. Runs only when the user types /slim.
argument-hint: "[part or path] (default: every part, largest first)"
disable-model-invocation: true
---

# Slim the code base

You lead a clean-up whose result is fewer lines of code with the same functionality.
Read `.claude/skills/lean-review/RULES.md` first: its rules, its patterns and above all
"What is not bloat" decide what counts. Small changes in behaviour are allowed where
they let code be shared; each one is listed for the user.

What to slim: $ARGUMENTS

You are the lead, as in `/orchestrate`: agents read the code and write to files, you
read their short reports. Do not load modules or diffs into your own context.

## 0. Start

1. Work on a branch of its own (a worktree when other sessions work on blip). The tree
   is clean, or you stop and ask.
2. Pick the scope. Without an argument: `.claude/skills/lean-review/loc.sh`, and take
   the parts largest first. One part per session is plenty: `kit` alone is several
   sessions, so split it by module (`store`, `widgets/view`, `widgets` without `view`,
   `workbench`).
3. The ledger is `.claude/specs/slim-<scope>.md`. If it exists, an earlier session
   started this scope: read it and go on where it stopped. Otherwise create it with the
   `loc.sh` table and the commit as the baseline.
4. Run the checks of the scope once (`./check.sh`, `cd outpost && uv run pytest`, or the
   sonde build and ctest). They must pass before anything changes: a clean-up on a red
   tree proves nothing. If the scope touches a path that runs all the time, run the
   baseline as well (`cargo test --release -p blip-kit --test baseline -- --ignored
   --nocapture`), and if it touches the UI render the screenshots into a scratch
   directory (`BLIP_SCREENSHOTS_DIR=<dir>`) to compare at the end. Note the results in
   the ledger.

## 1. Survey

Find before you change. Spawn the scope's specialist (`blip-dev`, `outpost-dev`,
`sonde-dev`) as finders, several in one message, each told to edit no code and to
append its candidates to the ledger. Give each one cut of at most about 4000 lines, and
`RULES.md`:

- **Per module:** the non-test files of one module. Dead code, one-user abstractions,
  speculative options, defensive checks, long-hand code, comments.
- **Across siblings:** the files that do the same job for different things, read side
  by side: every view under `kit/src/widgets/view`, every source, every `Viewable` and
  `AsValue` implementation, the encoders of the protocol in each part. This is where
  the most lines are: code written once and then written again for the next sibling.
  This finder runs on the specialist's default model; the per-module finders on Sonnet
  at medium effort.
- **Public surface** (`kit`, `proto`): for each `pub` item, where it is used outside
  its file: in `blip/`, `kit/examples`, `kit/tests`, `doc/source/library`. Used only by
  its own tests, or nowhere, is a candidate.
- **Tests:** the `tests.rs` files and test directories. Set-up copied between tests,
  tests that differ in one input, the same behaviour asserted at several layers,
  helpers of the same name in several files.

A candidate in the ledger is one block:

```
### <n> <short title>
- where: <file:line>, <file:line>
- pattern: <from the table in RULES.md>
- change: <what replaces it, in one or two lines>
- saves: about <n> lines of code, <n> of tests
- kind: same behaviour | behaviour changes: <what the user would notice> | public API: <item> | file format
- proven by: <the tests that cover it today, or "none">
```

A finder reports ten lines: how many candidates, the lines they save, the largest three.
A candidate needs a concrete smaller form. "Could be simpler" is not one.

## 2. Decide

Read the ledger (it is the one thing you read in full). Drop candidates that break
"What is not bloat", and those that save under about five lines unless they remove a
whole item. Mark doubles. Then show the user, with AskUserQuestion:

- the total per pattern and per module, and the estimate against the baseline;
- every candidate of kind **behaviour changes**, **public API** or **file format**,
  one line each, with your recommendation. These are the user's call, one by one or in
  groups. Do not take silence for a yes;
- candidates **proven by: none**. They get a test first (in the batch before), or are
  left out.

Candidates of kind **same behaviour** with tests need no approval. Write the decisions
into the ledger.

## 3. Shrink, batch by batch

Order the batches: dead code first (it makes the rest smaller), then merges inside a
module, then merges across siblings, then long-hand code and comments, the tests last.
A batch is one theme in one module, about 400 changed lines at most, so that it can be
reviewed and reverted on its own.

Two rules keep a batch honest:

- **A batch changes the code or the tests, not both.** While the code changes, the
  tests stay as they are and prove it; only a signature they call may follow. While the
  tests are merged, the code stays as it is. Tests of code that was removed go with it.
- **One writer per file.** Batches that share no file may run in parallel.

Per batch, spawn a fresh specialist on Sonnet at medium effort (a merge across siblings
that needs a new shared signature goes to its default model). The brief: the ledger
file and the candidate numbers, `RULES.md`, the files it may touch, the checks to run
(the tests and builds of what it touched; the last batch of a part runs the full
checks), and to report the tests it changed and why, in ten lines.

After each batch:

1. `git diff --stat` against the commit before: only the batch's files.
2. `loc.sh HEAD`: the code is smaller. A batch that does not shrink the code, or that
   moves lines from the code into the tests, is reverted (`git restore` its files,
   yourself, while no agent is writing) and noted in the ledger with the reason.
3. The checks the agent ran passed. A batch that fails twice is reverted and noted.
4. Commit it on the branch (subject: what was merged or removed; body: why it is safe,
   the lines saved, any change in behaviour). Do not push. Mark the candidates done in
   the ledger with the lines they saved in fact.

When a test batch merges or renames tests, the `TEST_*` items in
`doc/source/requirements/tests` that name them follow in the same batch, and every
assertion is kept.

## 4. Docs

Spawn `docs-writer` once per part with the list of what was removed, renamed or
changed in behaviour: the internals and library pages that name it, the code samples,
and a `changes.rst` entry for each change the user of blip or of the library notices.
`cd doc && make html` passes.

## 5. Close

1. Spawn `reviewer` on its default model with the scope **correctness**, the baseline
   commit to diff against, the ledger, and the instruction that no requirement changed:
   every test removed or loosened is a finding unless the ledger names what still
   checks the behaviour. On Sonnet, a second `reviewer` with the scope **coverage**
   runs the full checks.
2. Run the baseline and render the screenshots again if you did in step 0. A result
   more than about 10 % worse, or a screenshot that differs without a decided change
   in behaviour behind it, goes back to the specialist.
3. Fix as in `/orchestrate` step 8, then report to the user:
   - the `loc.sh <baseline commit>` table;
   - what was merged and removed, by pattern, with the lines saved;
   - every change in behaviour and in the public API, one by one;
   - the candidates left out or reverted, and why;
   - what the survey shows as the next scope, and whether another pass on this one is
     worth it (a survey that finds less than about 2 % is not).
4. If one pattern made up much of the findings, propose the rule that would have
   prevented it and where it goes (`RULES.md`, an agent file, `CLAUDE.md`), and add it
   when the user agrees.

Keep the ledger until the branch is merged; it lets a later session go on.

## Do not

- Set a number of lines to reach. A target makes agents golf. Report what was found.
- Shrink by formatting, by shorter names, or by moving code into macros.
- Remove what a requirement names, or change a file format or the protocol, without
  the user's decision.
- Refactor what no candidate names. A clean-up that grows is the thing it set out to
  remove.
