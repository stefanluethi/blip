#!/bin/sh
# cargo calls this as `rustc-wrapper.sh <rustc> <arguments>` (see config.toml).
# With sccache installed the call goes through its cache, which all checkouts and
# worktrees share; without it rustc runs directly.
if command -v sccache >/dev/null 2>&1; then
    exec sccache "$@"
fi
exec "$@"
