#!/usr/bin/env bash
# Lines of code per part, counted by tokei and split into code and tests.
#
#   loc.sh            the working tree
#   loc.sh <git ref>  the ref, the working tree and the difference
#
# Counts Rust, Python and C++ only (no docs, no generated files). Tests are the
# `tests.rs` files, the `tests` and `benches` directories and `test_*.py`.
set -euo pipefail

parts=(blip kit proto outpost sonde)
skip=(-e target -e build -e dist -e .venv -e __pycache__)
tests=(-e tests.rs -e tests -e benches -e 'test_*.py')

# code lines of the languages we count, from tokei's table
sum() {
    tokei "$@" 2>/dev/null | awk '
        /^ (Rust|Python|C\+\+|C\+\+ Header|C Header|C) +[0-9]/ { n += $(NF-2) }
        END { print n + 0 }'
}

# prints "<part> <code> <tests>" per part of the tree in $1
measure() {
    for part in "${parts[@]}"; do
        [ -d "$1/$part" ] || { echo "$part 0 0"; continue; }
        all=$(sum "$1/$part" "${skip[@]}")
        code=$(sum "$1/$part" "${skip[@]}" "${tests[@]}")
        echo "$part $code $((all - code))"
    done
}

root=$(git rev-parse --show-toplevel)
now=$(measure "$root")

if [ $# -eq 0 ]; then
    echo "$now" | awk '
        BEGIN { printf "%-8s %8s %8s\n", "part", "code", "tests" }
        { printf "%-8s %8d %8d\n", $1, $2, $3; c += $2; t += $3 }
        END { printf "%-8s %8d %8d\n", "total", c, t }'
    exit
fi

old_tree=$(mktemp -d)
trap 'rm -rf "$old_tree"' EXIT
git -C "$root" archive "$1" | tar -x -C "$old_tree"
old=$(measure "$old_tree")

paste -d' ' <(echo "$old") <(echo "$now") | awk -v ref="$1" '
    BEGIN { printf "against %s\n%-8s %8s %8s %8s   %8s %8s %8s\n", ref,
            "part", "code", "now", "diff", "tests", "now", "diff" }
    function row(name, c0, c1, t0, t1) {
        printf "%-8s %8d %8d %+8d   %8d %8d %+8d\n", name, c0, c1, c1 - c0, t0, t1, t1 - t0
    }
    { row($1, $2, $5, $3, $6); c0 += $2; c1 += $5; t0 += $3; t1 += $6 }
    END { row("total", c0, c1, t0, t1) }'
