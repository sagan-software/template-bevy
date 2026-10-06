#!/usr/bin/env python3
"""Keep Cargo artifact locations out of the compiler-cache environment."""

import os
import subprocess
import sys


def main() -> int:
    """Forward the compiler command and retain other environment values."""
    environment = os.environ.copy()
    # These Cargo-only locations prevent compiler-cache reuse across worktrees.
    for name in ("CARGO_TARGET_DIR", "CARGO_BUILD_BUILD_DIR"):
        environment.pop(name, None)
    arguments = sys.argv[1:]
    workspace = os.environ.get("RUSTC_WORKSPACE_WRAPPER")
    # Workspace and Clippy wrappers own incremental compilation and linting.
    if arguments and workspace and arguments[0] == workspace:
        os.execvpe(workspace, arguments, environment)
    if os.name == "nt":
        return subprocess.run(
            ["sccache", *sys.argv[1:]], env=environment, check=False
        ).returncode
    os.execvpe("sccache", ["sccache", *sys.argv[1:]], environment)


if __name__ == "__main__":
    sys.exit(main())
