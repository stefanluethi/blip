#!/usr/bin/env python3
"""Blocks Bash commands that would make a debug build (see "Release builds only" in
CLAUDE.md): cargo build, run, check, clippy, test, doc and rustc without --release,
trunk build, serve and watch without --release, and a CMake configure without
CMAKE_BUILD_TYPE. A debug build fills target/debug with tens of GB.

Runs as a PreToolUse hook on Bash. It reads the hook input as JSON on stdin and exits
with 2 and a reason on stderr to block the command, or 0 to let it run.
"""

import json
import shlex
import sys

CARGO_BUILDS = {"build", "b", "run", "r", "check", "c", "clippy", "test", "t", "doc", "d", "rustc"}
TRUNK_BUILDS = {"build", "serve", "watch"}
CMAKE_TYPES = {"Release", "MinSizeRel", "RelWithDebInfo"}
SEPARATORS = {";", "&&", "||", "|", "&", "(", ")", "|&", ";;"}
WRAPPERS = {"env", "time", "nice", "command", "exec", "nohup"}


def commands(line: str) -> list[list[str]]:
    """The simple commands in a shell line, each as its words."""
    lexer = shlex.shlex(line.replace("\\\n", " ").replace("\n", " ; "), posix=True, punctuation_chars=True)
    lexer.whitespace_split = True
    result: list[list[str]] = [[]]
    for token in lexer:
        if token in SEPARATORS:
            result.append([])
        else:
            result[-1].append(token)
    return [words for words in result if words]


def program(words: list[str]) -> list[str]:
    """The words from the program on, without variable assignments and wrappers."""
    i = 0
    while i < len(words) and ("=" in words[i] and not words[i].startswith("-") or words[i] in WRAPPERS):
        i += 1
    return words[i:]


def options(args: list[str]) -> list[str]:
    """The arguments before `--`, which belong to cargo or trunk, not to the program."""
    return args[: args.index("--")] if "--" in args else args


def release_profile(args: list[str]) -> bool:
    if "--release" in args or "-r" in args:
        return True
    for i, arg in enumerate(args):
        if arg in ("--profile=release", "--profile=minsize") or (arg == "--profile" and i + 1 < len(args)):
            return arg != "--profile" or args[i + 1] != "dev"
    return False


def problem(words: list[str]) -> str | None:
    words = program(words)
    if not words:
        return None
    name = words[0].rsplit("/", 1)[-1]
    args = words[1:]
    if name == "cargo":
        if args and args[0].startswith("+"):
            args = args[1:]
        if args and args[0] in CARGO_BUILDS and not release_profile(options(args[1:])):
            return f"`cargo {args[0]}` without --release"
    elif name == "trunk":
        if args and args[0] in TRUNK_BUILDS and not release_profile(options(args[1:])):
            return f"`trunk {args[0]}` without --release"
    elif name == "cmake":
        configures = any(a in ("-S", "-B") or a.startswith(("-S", "-B")) for a in args)
        other_mode = any(a in ("--build", "--install", "-E", "-P", "--preset") or a.startswith("--preset=") for a in args)
        if configures and not other_mode:
            joined = [a + b if a == "-D" else a for a, b in zip(args, args[1:] + [""])]
            types = [a.split("=", 1)[1] for a in joined if a.startswith("-DCMAKE_BUILD_TYPE=")]
            if not any(t in CMAKE_TYPES for t in types):
                return "a CMake configure without -DCMAKE_BUILD_TYPE=Release (or MinSizeRel)"
    return None


def main() -> int:
    try:
        command = json.load(sys.stdin).get("tool_input", {}).get("command", "")
        found = [p for words in commands(command) if (p := problem(words))]
    except (ValueError, AttributeError):
        return 0  # a line shlex cannot split, e.g. an unclosed quote in a heredoc
    if found:
        print(
            f"Blocked: {found[0]}. This project builds in release mode only, a debug build "
            "fills target/debug with tens of GB (see CLAUDE.md). Add --release, or "
            "-DCMAKE_BUILD_TYPE=Release for CMake.",
            file=sys.stderr,
        )
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
