---
name: lean-review
description: Review a change to blip for size. Measures what the change adds with tokei, compares it with what was asked for, and finds code and tests in it that are repeated, unused, speculative, defensive or longer than needed, each with the smaller form. Use it on a branch or the working tree before a change is committed or merged, and from /orchestrate after the build. With "fix" it applies the findings and measures again. It does not look for bugs; the reviewer does.
argument-hint: "[fix] [base ref, default main]"
---

# Review a change for size

You check one thing: is this change as small as what it does allows? Read
`RULES.md` in this skill's directory (`.claude/skills/lean-review/RULES.md`) first. Its
rules and patterns are what you check; its "What is not bloat" is what you must not
ask for.

Arguments: $ARGUMENTS

Review with fresh eyes: the session that wrote the change defends it. If you wrote this
change in this conversation, spawn a fresh agent with this skill instead, or tell the
user to run it in a new session.

## 1. Measure

The base is the ref in the arguments, or `main`.

```bash
base=$(git merge-base HEAD main)
.claude/skills/lean-review/loc.sh "$base"
git diff --stat "$base"
```

The diff is the working tree against the base, new files included (`git status`).
Note the net lines of code and of tests per part, and the files added.

## 2. What was asked

Find the size the change should have, from the first of these that exists: the spec or
the **Agreed** file in `.claude/specs` (its budget and its list of functions to reuse),
the requirement IDs named in the brief, the commit messages, the user's request. One
sentence: what the change does for the user. If you cannot say it, ask.

Compare with the budget in `RULES.md`: code against the estimate, tests against code,
a replacement near zero, a fix small.

## 3. Read the added code

Go through the added lines (`git diff "$base"`, non-test files first), and for each new
function, type, file, option and test ask the questions of "Before you write" in
`RULES.md`. Do the searches; do not guess:

- **Is it there already?** For each new helper, `grep` the part for the types it takes
  and the verb in its name, and open the sibling modules (the other views, sources,
  sinks). The usual finding is a new copy of what the neighbour has.
- **Who uses it?** `grep` each new item's name. One caller for an abstraction, no
  caller outside tests, or an argument every caller passes alike is a finding.
- **What did it replace?** `grep` for the old names. Code the change made obsolete and
  left in place is a finding, with its tests and docs.
- **Can the state happen?** For each new check, fallback and `Option`, find where the
  value comes from. If the edge has checked it, the inner check is a finding.
- **Is it in scope?** Hunks that the one sentence of step 2 does not need.
- **Tests:** per new test, the behaviour it checks. Two tests for one behaviour, copied
  set-up, a table written as separate tests.

Code that was there before is not your subject, with one exception: when the change
copies it or adds a sibling to it, the finding is to share the two.

## 4. Report

Only findings with a concrete smaller form that you have checked against the code.
Leave out style, naming, bugs, and anything under about five lines unless it removes
a whole item. Most severe first:

```
<file:line> <pattern>: <what is there>
  smaller: <what to do instead, naming the existing code to use>
  saves: about <n> lines   kind: same behaviour | behaviour changes: <what>
```

Then:

- the table of `loc.sh`, and code and tests added against the budget;
- the lines the findings would save together;
- the verdict: **lean** (nothing worth a round), **shrink** (findings to apply), or
  **rethink** (the approach is larger than the problem: say which simpler approach,
  and stop; do not list details of code that should not exist).

An honest "lean" is a good result. Do not invent findings to have some.

## 5. Fix (only with `fix`, or when the user says so)

Apply the findings of kind **same behaviour**, code first with the tests untouched, then
the tests. Findings that change behaviour or the public API wait for the user. Run the
checks of the parts you touched (see `CLAUDE.md`), then `loc.sh "$base"` again, and
report both tables and what you left. A fix that does not make the change smaller is
taken back.
