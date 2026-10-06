"""Public command-line tests for the Cargo rustc-wrapper launcher."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().with_name("rustc-cache.py")
CAPTURE_KEYS = (
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_BUILD_DIR",
    "CARGO_PKG_NAME",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_HOME",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "PATH",
    "CACHE_TEST_SENTINEL",
)


@unittest.skipUnless(
    os.name == "posix", "fixture executable requires POSIX PATH lookup"
)
class RustcCacheCliTests(unittest.TestCase):
    """Check compiler forwarding and child environment at the process seam."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(
            prefix="rustc-cache-test-"
        )
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.tool_bin = self.root / "tools"
        self.tool_bin.mkdir()
        (self.tool_bin / "python3").symlink_to(sys.executable)
        self.capture = self.root / "sccache-invocation.json"
        self._write_sccache_fixture()

    def _write_sccache_fixture(self) -> None:
        self._write_capture_fixture("sccache")

    def _write_workspace_wrapper_fixture(self) -> Path:
        return self._write_capture_fixture("workspace-wrapper")

    def _write_capture_fixture(self, name: str) -> Path:
        executable = self.tool_bin / name
        executable.write_text(
            f"#!{sys.executable}\n"
            "import json\n"
            "import os\n"
            "import sys\n"
            "keys = json.loads(os.environ['CACHE_TEST_CAPTURE_KEYS'])\n"
            "payload = {'tool': os.path.basename(sys.argv[0]), "
            "'argv': sys.argv[1:], "
            "'env': {key: os.environ[key] for key in keys "
            "if key in os.environ}}\n"
            "with open(os.environ['CACHE_TEST_CAPTURE'], 'w', "
            "encoding='utf-8') as output:\n"
            "    json.dump(payload, output)\n"
            "raise SystemExit(int(os.environ.get('CACHE_TEST_EXIT', '0')))\n",
            encoding="utf-8",
        )
        executable.chmod(0o755)
        return executable

    def _environment(
        self,
        exit_status: int = 0,
        workspace_wrapper: Path | None = None,
    ) -> dict[str, str]:
        environment = os.environ.copy()
        environment.update(
            {
                "PATH": str(self.tool_bin),
                "CARGO_TARGET_DIR": str(self.root / "target"),
                "CARGO_BUILD_BUILD_DIR": str(self.root / "build"),
                "CARGO_PKG_NAME": "fixture-package",
                "CARGO_ENCODED_RUSTFLAGS": "-Copt-level=1",
                "CARGO_HOME": str(self.root / "cargo-home"),
                "RUSTC_WRAPPER": "outer-wrapper-value",
                "CACHE_TEST_SENTINEL": "preserve-this-value",
                "CACHE_TEST_CAPTURE": str(self.capture),
                "CACHE_TEST_CAPTURE_KEYS": json.dumps(CAPTURE_KEYS),
                "CACHE_TEST_EXIT": str(exit_status),
            }
        )
        if workspace_wrapper is not None:
            environment["RUSTC_WORKSPACE_WRAPPER"] = str(workspace_wrapper)
        return environment

    def _run(
        self,
        arguments: list[str],
        environment: dict[str, str],
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [str(SCRIPT), *arguments],
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )

    def _invoke(
        self,
        compiler: str,
        *arguments: str,
        exit_status: int = 0,
        workspace_wrapper: Path | None = None,
    ) -> subprocess.CompletedProcess[str]:
        return self._run(
            [compiler, *arguments],
            self._environment(exit_status, workspace_wrapper),
        )

    def _invocation(self) -> dict[str, object]:
        return json.loads(self.capture.read_text(encoding="utf-8"))

    def test_forwarding_and_child_environment(self) -> None:
        workspace_wrapper = self._write_workspace_wrapper_fixture()
        compiler = str(self.root / "rustc with spaces")
        arguments = (
            "--crate-name",
            "two words",
            "--edition=2024",
            "src/main.rs",
        )
        result = self._invoke(
            compiler, *arguments, workspace_wrapper=workspace_wrapper
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        invocation = self._invocation()
        self.assertEqual(invocation["argv"], [compiler, *arguments])
        child_environment = invocation["env"]
        self.assertNotIn("CARGO_TARGET_DIR", child_environment)
        self.assertNotIn("CARGO_BUILD_BUILD_DIR", child_environment)
        self.assertEqual(
            child_environment["CARGO_PKG_NAME"], "fixture-package"
        )
        self.assertEqual(
            child_environment["CARGO_ENCODED_RUSTFLAGS"], "-Copt-level=1"
        )
        self.assertEqual(
            child_environment["CARGO_HOME"], str(self.root / "cargo-home")
        )
        self.assertEqual(
            child_environment["RUSTC_WRAPPER"], "outer-wrapper-value"
        )
        self.assertEqual(
            child_environment["RUSTC_WORKSPACE_WRAPPER"],
            str(workspace_wrapper),
        )
        self.assertEqual(
            child_environment["CACHE_TEST_SENTINEL"], "preserve-this-value"
        )
        self.assertEqual(child_environment["PATH"], str(self.tool_bin))

    def test_compiler_exit_status_propagates(self) -> None:
        result = self._invoke("rustc", "--version", exit_status=43)

        self.assertEqual(result.returncode, 43, result.stderr)
        self.assertEqual(self._invocation()["argv"], ["rustc", "--version"])

    def test_workspace_wrapper_receives_nested_compiler_and_exit_status(
        self,
    ) -> None:
        workspace_wrapper = self._write_workspace_wrapper_fixture()
        compiler = str(self.root / "workspace rustc")
        arguments = [
            compiler,
            "--crate-name",
            "workspace member",
            "-C",
            "incremental",
        ]
        result = self._run(
            [str(workspace_wrapper), *arguments],
            self._environment(47, workspace_wrapper),
        )

        self.assertEqual(result.returncode, 47, result.stderr)
        invocation = self._invocation()
        self.assertEqual(invocation["tool"], "workspace-wrapper")
        self.assertEqual(invocation["argv"], arguments)
        child_environment = invocation["env"]
        self.assertNotIn("CARGO_TARGET_DIR", child_environment)
        self.assertNotIn("CARGO_BUILD_BUILD_DIR", child_environment)
        self.assertEqual(
            child_environment["RUSTC_WORKSPACE_WRAPPER"],
            str(workspace_wrapper),
        )
        self.assertEqual(
            child_environment["CARGO_PKG_NAME"], "fixture-package"
        )
        self.assertEqual(
            child_environment["CACHE_TEST_SENTINEL"], "preserve-this-value"
        )


if __name__ == "__main__":
    unittest.main()
