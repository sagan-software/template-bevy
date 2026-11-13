"""Tests for evicting idle and oversized entries from the build cache."""

from __future__ import annotations

import importlib.util
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parent / "cargo-fast.py"
specification = importlib.util.spec_from_file_location("cargo_fast", SCRIPT)
assert specification is not None and specification.loader is not None
cargo_fast = importlib.util.module_from_spec(specification)
specification.loader.exec_module(cargo_fast)

NOW = 1_800_000_000.0
HOUR = 3600.0
MIB = 1024**2


class EvictionTests(unittest.TestCase):
    """Run eviction over fake cache entries."""

    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory(prefix="cargo-fast-evict-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.cache = self.root / "cache"
        self.proc = self.root / "proc"
        self.proc.mkdir()
        self.environment = patch.dict(
            os.environ,
            {
                "BEVY_BUILD_MAX_IDLE_HOURS": "",
                "BEVY_BUILD_MAX_GIB": "",
                "BEVY_BUILD_EVICT_INTERVAL_SECONDS": "",
            },
        )
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def entry(
        self, name: str, idle_hours: float, mebibytes: int = 1
    ) -> Path:
        """Make an entry with a target file, last used idle_hours ago."""
        key, _, part = name.partition("/")
        entry = self.cache / "repositories" / key / part
        (entry / "target" / "debug").mkdir(parents=True)
        with open(entry / "target" / "debug" / "artifact", "wb") as file:
            file.write(os.urandom(mebibytes * MIB))
        used = NOW - idle_hours * HOUR
        for path in (
            entry / "target" / "debug",
            entry / "target",
            entry,
        ):
            os.utime(path, (used, used))
        return entry

    def evict(self, keep: list[Path] | None = None) -> list[Path]:
        return cargo_fast.evict(self.cache, keep or [], NOW, self.proc)

    def test_idle_entries_go_and_in_use_ones_stay(self) -> None:
        idle = self.entry("a/idle", 7)
        locked = self.entry("a/locked", 30)
        named = self.entry("b/named", 30)
        current = self.entry("b/current", 30)
        recent = self.entry("c/recent", 1)
        dependencies = self.entry("c/dependencies", 9)
        launchers = self.cache / "repositories" / "a" / "launchers"
        launchers.mkdir()
        descriptor = cargo_fast.lock_entry(self.cache, locked, shared=True)
        self.addCleanup(os.close, descriptor)
        process = self.proc / "4242"
        process.mkdir()
        (process / "environ").write_bytes(
            b"HOME=/x\0CARGO_TARGET_DIR="
            + os.fsencode(named / "target")
            + b"\0"
        )

        removed = self.evict([current / "target"])

        self.assertEqual(sorted(removed), sorted([idle, dependencies]))
        for kept in (locked, named, current, recent, launchers):
            self.assertTrue(kept.is_dir(), kept)
        self.assertFalse(idle.exists())

    def test_least_recently_used_go_until_under_the_cap(self) -> None:
        oldest = self.entry("a/oldest", 3, 4)
        older = self.entry("a/older", 2, 4)
        newer = self.entry("b/newer", 1, 4)
        current = self.entry("b/current", 5, 4)
        # 16 MiB in the cache; a 9 MiB cap removes the two oldest others.
        with patch.dict(os.environ, {"BEVY_BUILD_MAX_GIB": str(9 / 1024)}):
            removed = self.evict([current])

        self.assertEqual(removed, [oldest, older])
        self.assertTrue(newer.is_dir())
        self.assertTrue(current.is_dir())

    def test_locked_entry_counts_but_is_skipped_under_the_cap(self) -> None:
        locked = self.entry("a/locked", 3, 4)
        older = self.entry("a/older", 2, 4)
        descriptor = cargo_fast.lock_entry(self.cache, locked, shared=True)
        self.addCleanup(os.close, descriptor)
        with patch.dict(os.environ, {"BEVY_BUILD_MAX_GIB": str(5 / 1024)}):
            removed = self.evict()

        self.assertEqual(removed, [older])
        self.assertTrue(locked.is_dir())

    def test_stamp_marks_an_entry_as_recently_used(self) -> None:
        entry = self.entry("a/stamped", 30)
        (entry / cargo_fast.STAMP).touch()
        os.utime(entry / cargo_fast.STAMP, (NOW, NOW))

        self.assertEqual(self.evict(), [])
        self.assertTrue(entry.is_dir())

    def test_zero_turns_limits_off(self) -> None:
        entry = self.entry("a/idle", 1000, 2)
        with patch.dict(
            os.environ,
            {"BEVY_BUILD_MAX_IDLE_HOURS": "0", "BEVY_BUILD_MAX_GIB": "0"},
        ):
            self.assertEqual(self.evict(), [])
        self.assertTrue(entry.is_dir())

    def test_interval_skips_checks_made_too_soon(self) -> None:
        self.cache.mkdir()
        marker = self.cache / "last-eviction"
        marker.touch()
        os.utime(marker, (NOW - 60, NOW - 60))
        entry = self.entry("a/idle", 30)

        self.assertEqual(self.evict(), [])
        self.assertTrue(entry.is_dir())
        os.utime(marker, (NOW - 601, NOW - 601))
        self.assertEqual(self.evict(), [entry])

    def test_missing_cache_and_proc_evict_nothing(self) -> None:
        self.assertEqual(
            cargo_fast.evict(self.cache, [], NOW, self.root / "no-proc"), []
        )

    def test_invalid_limits_fail_clearly(self) -> None:
        for value in ("many", "-1", "nan"):
            with (
                self.subTest(value=value),
                patch.dict(os.environ, {"BEVY_BUILD_MAX_GIB": value}),
                self.assertRaisesRegex(ValueError, "BEVY_BUILD_MAX_GIB"),
            ):
                self.evict()

    def test_hard_links_count_once(self) -> None:
        entry = self.entry("a/linked", 0, 2)
        artifact = entry / "target" / "debug" / "artifact"
        os.link(artifact, entry / "target" / "copy")
        self.assertLess(cargo_fast.disk_usage(entry), 3 * MIB)


if __name__ == "__main__":
    unittest.main()
