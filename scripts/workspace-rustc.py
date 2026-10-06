#!/usr/bin/env python3
"""Give Cargo a worktree-specific compiler namespace without changing rustc."""

import os
import sys


def main() -> None:
    """Forward the compiler and every argument without a shell."""
    os.execvpe(sys.argv[1], sys.argv[1:], os.environ)


if __name__ == "__main__":
    main()
