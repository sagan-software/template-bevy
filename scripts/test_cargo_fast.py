"""Public command-line tests for the shared Cargo build-cache wrapper."""

from __future__ import annotations

import importlib.util
import json
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

REPOSITORY = Path(__file__).resolve().parents[1]
SCRIPT = REPOSITORY / "scripts" / "cargo-fast.py"


class CargoFastCliTests(unittest.TestCase):
    """Exercise the wrapper through its public command-line interface."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="cargo-fast-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.repository = self.root / "project"
        self._create_git_repository(self.repository)
        self.worktree = self.root / "worktree"
        self._git(
            self.repository,
            "worktree",
            "add",
            "--quiet",
            "--detach",
            str(self.worktree),
            "HEAD",
        )
        self.clone = self.root / "clone"
        self._run_git(
            "clone", "--quiet", str(self.repository), str(self.clone)
        )
        self.tool_bin = self.root / "tools"
        self._create_tool_fixtures(self.tool_bin)
        self.cache_root = self.root / "external-cache"

    def _run_git(self, *arguments: str) -> None:
        subprocess.run(
            ["git", *arguments],
            check=True,
            capture_output=True,
            text=True,
        )

    def _git(self, directory: Path, *arguments: str) -> None:
        subprocess.run(
            ["git", "-C", str(directory), *arguments],
            check=True,
            capture_output=True,
            text=True,
        )

    def _create_git_repository(self, directory: Path) -> None:
        directory.mkdir(parents=True)
        (directory / "Cargo.toml").write_text(
            '[package]\nname = "cache-test"\n'
            'version = "0.1.0"\nedition = "2024"\n',
            encoding="utf-8",
        )
        (directory / "src").mkdir()
        (directory / "src" / "lib.rs").write_text(
            "pub fn value() {}\n", encoding="utf-8"
        )
        subprocess.run(
            [
                "git",
                "init",
                "--quiet",
                "--initial-branch=main",
                str(directory),
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        self._git(directory, "config", "user.name", "Cache Test")
        self._git(
            directory, "config", "user.email", "cache-test@example.invalid"
        )
        self._git(directory, "add", "Cargo.toml", "src/lib.rs")
        self._git(directory, "commit", "--quiet", "-m", "initial")

    def _create_tool_fixtures(self, directory: Path) -> None:
        directory.mkdir()
        git = shutil.which("git")
        if git is None:
            self.fail(
                "Git is required to create the worktree and clone fixtures"
            )
        (directory / "git").symlink_to(git)

        rustc = directory / "rustc"
        rustc.write_text(
            f"#!{sys.executable}\n"
            "import os\n"
            "import sys\n"
            "if sys.argv[1:] == ['-vV']:\n"
            "    print(os.environ.get('MOCK_RUSTC_IDENTITY', "
            "'rustc 1.99.0\\nhost: fixture-target\\n"
            "release: 1.99.0\\nLLVM version: fixture'))\n"
            "    raise SystemExit(0)\n"
            "raise SystemExit(0)\n",
            encoding="utf-8",
        )
        self._make_executable(rustc)

        cargo = directory / "cargo"
        cargo.write_text(
            f"#!{sys.executable}\n"
            "import json\n"
            "import os\n"
            "import sys\n"
            "if sys.argv[1:] == ['--version']:\n"
            "    print(os.environ.get('MOCK_CARGO_IDENTITY', "
            "'cargo 1.99.0'))\n"
            "    raise SystemExit(0)\n"
            "if sys.argv[1:2] == ['locate-project']:\n"
            "    locate_exit = int(os.getenv('MOCK_LOCATE_EXIT', '0'))\n"
            "    if locate_exit:\n"
            "        print('fixture locate-project failure',\n"
            "              file=sys.stderr)\n"
            "        raise SystemExit(locate_exit)\n"
            "    manifest_index = sys.argv.index('--manifest-path') + 1\n"
            "    manifest = sys.argv[manifest_index]\n"
            "    print(os.environ.get('MOCK_WORKSPACE_ROOT', manifest))\n"
            "    raise SystemExit(0)\n"
            "if sys.argv[1:2] == ['metadata']:\n"
            "    capture_path = os.getenv('MOCK_CARGO_METADATA_CAPTURE')\n"
            "    if capture_path:\n"
            "        with open(capture_path, 'w') as output:\n"
            "            payload = {\n"
            "                'argv': sys.argv[1:], 'cwd': os.getcwd()\n"
            "            }\n"
            "            json.dump(payload, output)\n"
            "    metadata_exit = int(\n"
            "        os.getenv('MOCK_CARGO_METADATA_EXIT', '0')\n"
            "    )\n"
            "    if metadata_exit:\n"
            "        raise SystemExit(metadata_exit)\n"
            "    metadata = os.environ.get('MOCK_CARGO_METADATA')\n"
            "    if metadata is None:\n"
            "        manifest = os.path.abspath('Cargo.toml')\n"
            "        package_id = (\n"
            "            'path+file://' + manifest +\n"
            "            '#cache-test@0.1.0'\n"
            "        )\n"
            "        packages = [{'id': package_id, 'source': None,\n"
            "                     'manifest_path': manifest}]\n"
            "        if '--all-features' in sys.argv:\n"
            "            optional_manifest = os.path.join(\n"
            "                os.path.dirname(manifest),\n"
            "                'optional-game-assets', 'Cargo.toml'\n"
            "            )\n"
            "            optional_id = (\n"
            "                'path+file://' + optional_manifest\n"
            "                + '#game-assets@0.1.0'\n"
            "            )\n"
            "            packages.append({\n"
            "                'id': optional_id, 'source': None,\n"
            "                'manifest_path': optional_manifest\n"
            "            })\n"
            "        metadata = json.dumps({\n"
            "            'version': 1,\n"
            "            'packages': packages,\n"
            "            'workspace_members': [package_id],\n"
            "            'workspace_root': os.path.dirname(manifest),\n"
            "        })\n"
            "    print(metadata)\n"
            "    raise SystemExit(0)\n"
            "keys = json.loads(os.environ.get('CARGO_FAST_ENV_KEYS', '[]'))\n"
            "payload = {'argv': sys.argv[1:], 'cwd': os.getcwd(), "
            "'env': {key: os.environ[key] for key in keys "
            "if key in os.environ}}\n"
            "with open(os.environ['CARGO_FAST_CAPTURE'], 'w', "
            "encoding='utf-8') as output:\n"
            "    json.dump(payload, output)\n"
            "raise SystemExit(int(os.environ.get('CARGO_MOCK_EXIT', '0')))\n",
            encoding="utf-8",
        )
        self._make_executable(cargo)

        for name in (
            "sccache",
            "ccache",
            "cc",
            "c++",
            "fixture-clang",
            "fixture-clang++",
        ):
            executable = directory / name
            if name in ("cc", "c++", "fixture-clang", "fixture-clang++"):
                version_name = (
                    "MOCK_CXX_VERSION" if "+" in name else "MOCK_CC_VERSION"
                )
                source = (
                    f"#!{sys.executable}\n"
                    "import json\n"
                    "import os\n"
                    "import sys\n"
                    "import time\n"
                    "if sys.argv[1:] and sys.argv[-1] == '--version':\n"
                    "    delay = float(\n"
                    "        os.getenv('MOCK_CC_SLEEP_SECONDS', '0')\n"
                    "    )\n"
                    "    if delay:\n"
                    "        time.sleep(delay)\n"
                    "    exit_code = int(os.getenv('MOCK_CC_EXIT', '0'))\n"
                    "    if exit_code:\n"
                    "        raise SystemExit(exit_code)\n"
                    "    capture = os.getenv('MOCK_COMPILER_CAPTURE')\n"
                    f"    version = os.getenv('{version_name}', "
                    f"'fixture {name} compiler 1')\n"
                    "    if capture:\n"
                    "        with open(capture, 'a') as output:\n"
                    "            json.dump({\n"
                    "                'path': sys.argv[0],\n"
                    "                'argv': sys.argv[1:],\n"
                    "                'cwd': os.getcwd(),\n"
                    "                'version': version\n"
                    "            }, output)\n"
                    "            output.write('\\n')\n"
                    "    print(version)\n"
                    "    raise SystemExit(0)\n"
                    "raise SystemExit(0)\n"
                )
            else:
                source = f"#!{sys.executable}\nraise SystemExit(0)\n"
            executable.write_text(source, encoding="utf-8")
            self._make_executable(executable)

    def _make_executable(self, path: Path) -> None:
        path.chmod(0o755)

    def _environment(
        self,
        *,
        tool_bin: Path | None = None,
        cache_root: Path | None = None,
        extra: dict[str, str] | None = None,
    ) -> dict[str, str]:
        environment = os.environ.copy()
        environment["PATH"] = str(tool_bin or self.tool_bin)
        environment["BEVY_BUILD_CACHE"] = str(cache_root or self.cache_root)
        environment["CARGO_HOME"] = str(self.root / "cargo-home")
        # Keep native compiler probes inside the mock PATH under Nix.
        environment["CC"] = "cc"
        environment["CXX"] = "c++"
        if extra:
            environment.update(extra)
        return environment

    def _invoke(
        self,
        cwd: Path,
        *arguments: str,
        environment: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(SCRIPT), *arguments],
            cwd=cwd,
            env=environment or self._environment(),
            capture_output=True,
            text=True,
            check=False,
        )

    def _print_environment(
        self,
        cwd: Path,
        *,
        lane: str | None = None,
        command: tuple[str, ...] = (),
        cache_mode: str | None = None,
        environment: dict[str, str] | None = None,
    ) -> dict[str, str]:
        arguments = ["--print-env"]
        if cache_mode is not None:
            arguments.extend(["--cache-mode", cache_mode])
        if lane is not None:
            arguments.extend(["--lane", lane])
        arguments.extend(command)
        result = self._invoke(cwd, *arguments, environment=environment)
        self.assertEqual(result.returncode, 0, result.stderr)
        selected = json.loads(result.stdout)
        self.assertIsInstance(selected, dict)
        return selected

    def _assert_cache_paths(
        self, selected: dict[str, str], base: Path
    ) -> None:
        for key in (
            "CARGO_TARGET_DIR",
            "CARGO_BUILD_BUILD_DIR",
            "SCCACHE_DIR",
            "CCACHE_DIR",
        ):
            self.assertIn(key, selected)
            self.assertTrue(
                Path(selected[key]).is_relative_to(base),
                (key, selected[key], base),
            )

    def _workspace_package(self) -> dict[str, object]:
        manifest = self.repository / "Cargo.toml"
        return {
            "id": "path+file:///fixture/cache-test@0.1.0",
            "source": None,
            "manifest_path": str(manifest),
        }

    def _metadata(
        self,
        packages: list[dict[str, object]],
        workspace_members: object,
        *,
        version: object = 1,
        workspace_root: Path | None = None,
    ) -> str:
        return json.dumps(
            {
                "version": version,
                "packages": packages,
                "workspace_members": workspace_members,
                "workspace_root": str(workspace_root or self.repository),
            }
        )

    def _assert_isolated(
        self, selected: dict[str, str], auto: dict[str, str]
    ) -> None:
        self.assertNotEqual(
            selected["CARGO_BUILD_BUILD_DIR"],
            auto["CARGO_BUILD_BUILD_DIR"],
        )
        self.assertEqual(selected.get("RUSTC_WORKSPACE_WRAPPER"), "")

    def test_default_auto_build_isolates_targets_and_shares_build_directory(
        self,
    ) -> None:
        project_environment = self._print_environment(self.repository)
        worktree_environment = self._print_environment(self.worktree)

        self.assertNotEqual(
            project_environment["CARGO_TARGET_DIR"],
            worktree_environment["CARGO_TARGET_DIR"],
        )
        self.assertEqual(
            project_environment["CARGO_BUILD_BUILD_DIR"],
            worktree_environment["CARGO_BUILD_BUILD_DIR"],
        )

        for key in ("SCCACHE_DIR", "CCACHE_DIR"):
            with self.subTest(key=key):
                self.assertEqual(
                    project_environment[key], worktree_environment[key]
                )

        self.assertEqual(
            project_environment["RUSTC_WRAPPER"],
            worktree_environment["RUSTC_WRAPPER"],
        )
        self.assertNotEqual(
            project_environment["RUSTC_WRAPPER"],
            str(SCRIPT.with_name("rustc-cache.py").resolve()),
        )
        for environment in (project_environment, worktree_environment):
            for key in ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
                wrapper = Path(environment[key])
                self.assertTrue(wrapper.is_file(), (key, wrapper))
                self.assertTrue(os.access(wrapper, os.X_OK), (key, wrapper))

        self.assertNotEqual(
            project_environment["RUSTC_WORKSPACE_WRAPPER"],
            worktree_environment["RUSTC_WORKSPACE_WRAPPER"],
        )

        self._assert_cache_paths(project_environment, self.cache_root)
        self._assert_cache_paths(worktree_environment, self.cache_root)

    def test_eligible_auto_build_shares_build_but_not_targets_or_wrappers(
        self,
    ) -> None:
        project_environment = self._print_environment(self.repository)
        worktree_environment = self._print_environment(self.worktree)

        self.assertEqual(
            project_environment["CARGO_BUILD_BUILD_DIR"],
            worktree_environment["CARGO_BUILD_BUILD_DIR"],
        )
        self.assertNotEqual(
            project_environment["CARGO_TARGET_DIR"],
            worktree_environment["CARGO_TARGET_DIR"],
        )
        self.assertNotEqual(
            project_environment["RUSTC_WORKSPACE_WRAPPER"],
            worktree_environment["RUSTC_WORKSPACE_WRAPPER"],
        )

    def test_supported_commands_use_auto_workspace_build_cache(self) -> None:
        default = self._print_environment(self.repository)

        for command in ("build", "test", "run", "check"):
            with self.subTest(command=command):
                selected = self._print_environment(
                    self.repository, command=(command,)
                )
                self.assertEqual(
                    selected["CARGO_BUILD_BUILD_DIR"],
                    default["CARGO_BUILD_BUILD_DIR"],
                )
                self.assertEqual(
                    selected["RUSTC_WORKSPACE_WRAPPER"],
                    default["RUSTC_WORKSPACE_WRAPPER"],
                )

    def test_clippy_xtask_and_unknown_commands_use_isolated_cache(
        self,
    ) -> None:
        auto = self._print_environment(self.repository)

        for command in ("clippy", "xtask", "future-subcommand"):
            with self.subTest(command=command):
                selected = self._print_environment(
                    self.repository, command=(command,)
                )
                self._assert_isolated(selected, auto)

    def test_known_external_sources_remain_eligible(self) -> None:
        workspace_package = self._workspace_package()
        packages = [workspace_package]
        external_sources = (
            (
                "registry+https://github.com/rust-lang/crates.io-index",
                self.root
                / "cargo-home"
                / "registry"
                / "src"
                / "index.crates.io-fixture"
                / "registry-dependency"
                / "Cargo.toml",
            ),
            (
                "git+https://github.com/example/dependency",
                self.root
                / "cargo-home"
                / "git"
                / "checkouts"
                / "dependency-fixture"
                / "revision"
                / "Cargo.toml",
            ),
            (
                "sparse+https://index.example.invalid/",
                self.root
                / "cargo-home"
                / "registry"
                / "src"
                / "sparse-index-fixture"
                / "sparse-dependency"
                / "Cargo.toml",
            ),
            (
                "registry+https://github.com/rust-lang/crates.io-index",
                Path("/nix/store/fixture-registry-source/Cargo.toml"),
            ),
        )
        for index, (source, manifest) in enumerate(external_sources):
            packages.append(
                {
                    "id": f"{source}#dependency-{index}@1.0.0",
                    "source": source,
                    "manifest_path": str(manifest),
                }
            )
        environment = self._environment(
            extra={
                "MOCK_CARGO_METADATA": self._metadata(
                    packages, [str(workspace_package["id"])]
                )
            }
        )
        default = self._print_environment(self.repository, command=("build",))
        selected = self._print_environment(
            self.repository, command=("build",), environment=environment
        )

        self.assertEqual(
            selected["CARGO_BUILD_BUILD_DIR"],
            default["CARGO_BUILD_BUILD_DIR"],
        )
        self.assertIn("RUSTC_WORKSPACE_WRAPPER", selected)

    def test_malformed_or_unsafe_package_metadata_disables_shared_build(
        self,
    ) -> None:
        workspace_package = self._workspace_package()
        workspace_member = str(workspace_package["id"])
        outside = self.root / "external-path" / "Cargo.toml"
        metadata_cases = (
            ("malformed", "{not-json"),
            (
                "unsupported metadata version",
                self._metadata(
                    [workspace_package], [workspace_member], version="1"
                ),
            ),
            (
                "boolean metadata version",
                self._metadata(
                    [workspace_package], [workspace_member], version=True
                ),
            ),
            (
                "missing metadata version key",
                json.dumps(
                    {
                        "packages": [workspace_package],
                        "workspace_members": [workspace_member],
                        "workspace_root": str(self.repository),
                    }
                ),
            ),
            (
                "missing package manifest key",
                self._metadata(
                    [{"id": workspace_member, "source": None}],
                    [workspace_member],
                ),
            ),
            (
                "invalid package manifest type",
                self._metadata(
                    [
                        {
                            "id": workspace_member,
                            "source": None,
                            "manifest_path": 1,
                        }
                    ],
                    [workspace_member],
                ),
            ),
            (
                "non-list workspace members",
                json.dumps(
                    {
                        "version": 1,
                        "packages": [workspace_package],
                        "workspace_members": workspace_member,
                        "workspace_root": str(self.repository),
                    }
                ),
            ),
            (
                "non-string workspace member",
                self._metadata([workspace_package], [None]),
            ),
            (
                "missing workspace package",
                self._metadata([workspace_package], ["missing-package-id"]),
            ),
            (
                "duplicate package identity",
                self._metadata(
                    [workspace_package, workspace_package], [workspace_member]
                ),
            ),
            ("empty package graph", self._metadata([], [])),
            (
                "unknown source",
                self._metadata(
                    [
                        workspace_package,
                        {
                            "id": (
                                "unknown+https://example.invalid"
                                "#package@1.0"
                            ),
                            "source": "unknown+https://example.invalid",
                            "manifest_path": str(outside),
                        },
                    ],
                    [workspace_member],
                ),
            ),
            (
                "vendored under workspace root",
                self._metadata(
                    [
                        workspace_package,
                        {
                            "id": "path+file:///fixture/vendor#vendored@1.0",
                            "source": None,
                            "manifest_path": str(
                                self.repository
                                / "vendor"
                                / "vendored"
                                / "Cargo.toml"
                            ),
                        },
                    ],
                    [workspace_member],
                ),
            ),
            (
                "nonmember path dependency",
                self._metadata(
                    [
                        workspace_package,
                        {
                            "id": "path+file:///external/path#dependency@1.0",
                            "source": None,
                            "manifest_path": str(outside),
                        },
                    ],
                    [workspace_member],
                ),
            ),
            (
                "registry package outside immutable cache",
                self._metadata(
                    [
                        workspace_package,
                        {
                            "id": (
                                "registry+https://example.invalid"
                                "#external@1.0"
                            ),
                            "source": "registry+https://example.invalid",
                            "manifest_path": str(
                                self.root / "external-registry" / "Cargo.toml"
                            ),
                        },
                    ],
                    [workspace_member],
                ),
            ),
            (
                "metadata names another workspace root",
                self._metadata(
                    [workspace_package],
                    [workspace_member],
                    workspace_root=self.root / "different-workspace",
                ),
            ),
        )
        auto = self._print_environment(self.repository, command=("build",))

        for label, metadata in metadata_cases:
            with self.subTest(case=label):
                environment = self._environment(
                    extra={"MOCK_CARGO_METADATA": metadata}
                )
                selected = self._print_environment(
                    self.repository,
                    command=("build",),
                    environment=environment,
                )
                self._assert_isolated(selected, auto)

    def test_failed_metadata_command_disables_shared_build(self) -> None:
        auto = self._print_environment(self.repository, command=("build",))
        environment = self._environment(
            extra={"MOCK_CARGO_METADATA_EXIT": "23"}
        )
        selected = self._print_environment(
            self.repository,
            command=("build",),
            environment=environment,
        )

        self._assert_isolated(selected, auto)

    def test_default_metadata_uses_only_requested_workspace_features(
        self,
    ) -> None:
        capture = self.root / "metadata-command.json"
        selected = self._print_environment(
            self.repository,
            command=("check",),
            environment=self._environment(
                extra={"MOCK_CARGO_METADATA_CAPTURE": str(capture)}
            ),
        )

        invocation = json.loads(capture.read_text(encoding="utf-8"))
        self.assertEqual(
            invocation["argv"],
            [
                "metadata",
                "--locked",
                "--format-version",
                "1",
            ],
        )
        self.assertEqual(invocation["cwd"], str(self.repository))
        self.assertIn("RUSTC_WORKSPACE_WRAPPER", selected)
        self.assertNotEqual(selected["RUSTC_WORKSPACE_WRAPPER"], "")

    def test_requested_features_only_reach_metadata_before_runtime_args(
        self,
    ) -> None:
        capture = self.root / "requested-features-metadata.json"
        cargo_arguments = (
            "test",
            "--package",
            "cache-test",
            "-p",
            "another-package",
            "--features",
            "root-feature",
            "-F",
            "short-feature",
            "--features=joined-one,joined-two",
            "-Fjoined-short",
            "--no-default-features",
            "--",
            "--all-features",
            "--features",
            "runtime-feature",
        )
        selected = self._print_environment(
            self.repository,
            command=cargo_arguments,
            environment=self._environment(
                extra={"MOCK_CARGO_METADATA_CAPTURE": str(capture)}
            ),
        )

        invocation = json.loads(capture.read_text(encoding="utf-8"))
        self.assertEqual(
            invocation["argv"],
            [
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--features",
                "root-feature",
                "-F",
                "short-feature",
                "--features=joined-one,joined-two",
                "-Fjoined-short",
                "--no-default-features",
            ],
        )
        self.assertEqual(invocation["cwd"], str(self.repository))
        self.assertNotEqual(selected["RUSTC_WORKSPACE_WRAPPER"], "")

    def test_requested_all_features_reaches_metadata(self) -> None:
        capture = self.root / "all-features-metadata.json"
        selected = self._print_environment(
            self.repository,
            command=("build", "--all-features"),
            environment=self._environment(
                extra={"MOCK_CARGO_METADATA_CAPTURE": str(capture)}
            ),
        )

        invocation = json.loads(capture.read_text(encoding="utf-8"))
        self.assertEqual(
            invocation["argv"],
            [
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--all-features",
            ],
        )
        self.assertEqual(selected["RUSTC_WORKSPACE_WRAPPER"], "")

    def test_eligible_build_directory_is_shared_across_lane_values(
        self,
    ) -> None:
        project_environment = self._print_environment(
            self.repository, lane="agent-a", command=("build",)
        )
        worktree_environment = self._print_environment(
            self.worktree, lane="agent-b", command=("test",)
        )

        self.assertEqual(
            project_environment["CARGO_BUILD_BUILD_DIR"],
            worktree_environment["CARGO_BUILD_BUILD_DIR"],
        )
        self.assertNotEqual(
            project_environment["CARGO_TARGET_DIR"],
            worktree_environment["CARGO_TARGET_DIR"],
        )
        self.assertNotEqual(
            project_environment["RUSTC_WORKSPACE_WRAPPER"],
            worktree_environment["RUSTC_WORKSPACE_WRAPPER"],
        )

    def test_explicit_shared_lane_shares_target_and_build_but_not_wrapper(
        self,
    ) -> None:
        project_environment = self._print_environment(
            self.repository, lane="shared"
        )
        worktree_environment = self._print_environment(
            self.worktree, lane="shared"
        )

        self.assertEqual(
            project_environment["CARGO_TARGET_DIR"],
            worktree_environment["CARGO_TARGET_DIR"],
        )
        self.assertEqual(
            project_environment["CARGO_BUILD_BUILD_DIR"],
            worktree_environment["CARGO_BUILD_BUILD_DIR"],
        )
        self.assertNotEqual(
            project_environment["RUSTC_WORKSPACE_WRAPPER"],
            worktree_environment["RUSTC_WORKSPACE_WRAPPER"],
        )

        for key in ("SCCACHE_DIR", "CCACHE_DIR"):
            with self.subTest(key=key):
                self.assertEqual(
                    project_environment[key], worktree_environment[key]
                )

    def test_separate_clone_gets_distinct_cache_directories(self) -> None:
        project_environment = self._print_environment(self.repository)
        clone_environment = self._print_environment(self.clone)

        for key in ("CARGO_TARGET_DIR", "CARGO_BUILD_BUILD_DIR"):
            with self.subTest(key=key):
                self.assertNotEqual(
                    project_environment[key], clone_environment[key]
                )

    def test_compiler_identity_changes_cache_directories(self) -> None:
        first = self._print_environment(self.repository)
        second = self._print_environment(
            self.repository,
            environment=self._environment(
                extra={
                    "MOCK_RUSTC_IDENTITY": "rustc 1.99.0\\nhost: other-target"
                }
            ),
        )

        for key in ("CARGO_TARGET_DIR", "CARGO_BUILD_BUILD_DIR"):
            with self.subTest(key=key):
                self.assertNotEqual(first[key], second[key])

    def test_cargo_identity_changes_cache_directories(self) -> None:
        first = self._print_environment(self.repository)
        second = self._print_environment(
            self.repository,
            environment=self._environment(
                extra={"MOCK_CARGO_IDENTITY": "cargo 1.100.0"}
            ),
        )

        for key in ("CARGO_TARGET_DIR", "CARGO_BUILD_BUILD_DIR"):
            with self.subTest(key=key):
                self.assertNotEqual(first[key], second[key])

    def test_lane_isolates_target_and_namespace_but_reuses_build_directory(
        self,
    ) -> None:
        default = self._print_environment(self.repository)
        lane = self._print_environment(self.repository, lane="agent-17")

        self.assertNotEqual(
            default["CARGO_TARGET_DIR"], lane["CARGO_TARGET_DIR"]
        )
        self.assertEqual(
            default["CARGO_BUILD_BUILD_DIR"], lane["CARGO_BUILD_BUILD_DIR"]
        )
        self.assertNotEqual(
            default["RUSTC_WORKSPACE_WRAPPER"],
            lane["RUSTC_WORKSPACE_WRAPPER"],
        )
        repeated = self._print_environment(self.repository, lane="agent-17")
        for key in (
            "CARGO_TARGET_DIR",
            "CARGO_BUILD_BUILD_DIR",
            "RUSTC_WORKSPACE_WRAPPER",
        ):
            self.assertEqual(lane[key], repeated[key])
        wrapper = Path(lane["RUSTC_WORKSPACE_WRAPPER"])
        self.assertTrue(wrapper.is_file())
        self.assertTrue(os.access(wrapper, os.X_OK))

    def test_cached_launchers_are_copied_and_keep_mtime_when_unchanged(
        self,
    ) -> None:
        selected = self._print_environment(self.repository)
        wrapper_paths = (
            Path(selected["RUSTC_WRAPPER"]),
            Path(selected["RUSTC_WORKSPACE_WRAPPER"]),
        )
        for wrapper in wrapper_paths:
            self.assertFalse(wrapper.is_symlink())
            sentinel = 1_600_000_000_000_000_000
            os.utime(wrapper, ns=(sentinel, sentinel))

        self._print_environment(self.repository)

        for wrapper in wrapper_paths:
            with self.subTest(wrapper=wrapper.name):
                self.assertEqual(wrapper.stat().st_mtime_ns, sentinel)

    def test_changed_cached_launcher_is_replaced_and_then_stable(self) -> None:
        selected = self._print_environment(self.repository)
        wrapper = Path(selected["RUSTC_WRAPPER"])
        wrapper.write_bytes(b"#!/usr/bin/env python3\n# stale helper\n")
        wrapper.chmod(0o700)
        stale_time = 1_600_000_000_000_000_000
        os.utime(wrapper, ns=(stale_time, stale_time))
        stale_inode = wrapper.stat().st_ino

        refreshed = self._print_environment(self.repository)

        current_source = SCRIPT.with_name("rustc-cache.py").read_bytes()
        expected = current_source.replace(
            b"#!/usr/bin/env python3",
            f"#!{Path(sys.executable).resolve()}".encode(),
            1,
        )
        self.assertEqual(refreshed["RUSTC_WRAPPER"], str(wrapper))
        self.assertEqual(wrapper.read_bytes(), expected)
        self.assertNotEqual(wrapper.stat().st_ino, stale_inode)
        self.assertNotEqual(wrapper.stat().st_mtime_ns, stale_time)
        self.assertTrue(os.access(wrapper, os.X_OK))
        refreshed_time = wrapper.stat().st_mtime_ns

        self._print_environment(self.repository)

        self.assertEqual(wrapper.stat().st_mtime_ns, refreshed_time)

    def test_native_compiler_settings_change_cache_identity(self) -> None:
        baseline = self._print_environment(self.repository)
        alternatives = (
            ("CC", "fixture-clang"),
            ("CXX", "fixture-clang++"),
            ("RUSTFLAGS", "-C target-cpu=fixture-cpu"),
            ("CARGO_ENCODED_RUSTFLAGS", "-Ctarget-cpu=fixture-cpu"),
            ("NIX_LDFLAGS", "-Wl,--as-needed"),
            ("NIX_CFLAGS_COMPILE", "-DFIXTURE_NATIVE_FLAG=1"),
        )
        for name, value in alternatives:
            with self.subTest(setting=name):
                selected = self._print_environment(
                    self.repository,
                    environment=self._environment(extra={name: value}),
                )
                for cache_name in (
                    "CARGO_TARGET_DIR",
                    "CARGO_BUILD_BUILD_DIR",
                ):
                    self.assertNotEqual(
                        baseline[cache_name], selected[cache_name]
                    )

    def test_resolved_compiler_paths_and_versions_change_cache_identity(
        self,
    ) -> None:
        baseline_capture = self.root / "baseline-compiler-versions.jsonl"
        baseline_environment = self._environment(
            extra={
                "CC": "cc --target=fixture-target -pthread",
                "CXX": "c++ --target=fixture-cxx -pthread",
                "MOCK_COMPILER_CAPTURE": str(baseline_capture),
            }
        )
        baseline = self._print_environment(
            self.repository, environment=baseline_environment
        )

        alternate_bin = self.root / "alternate-compiler-bin"
        alternate_bin.mkdir()
        alternate_cc_path = alternate_bin / "cc"
        shutil.copy2(self.tool_bin / "cc", alternate_cc_path)
        alternate_cxx_path = alternate_bin / "c++"
        shutil.copy2(self.tool_bin / "c++", alternate_cxx_path)
        alternate_capture = self.root / "alternate-compiler-versions.jsonl"
        alternate_environment = self._environment(
            extra={
                "PATH": f"{alternate_bin}{os.pathsep}{self.tool_bin}",
                "CC": "cc --target=fixture-target -pthread",
                "CXX": "c++ --target=fixture-cxx -pthread",
                "MOCK_CC_VERSION": "fixture cc compiler 2",
                "MOCK_CXX_VERSION": "fixture cxx compiler 2",
                "MOCK_COMPILER_CAPTURE": str(alternate_capture),
            }
        )
        alternate = self._print_environment(
            self.repository, environment=alternate_environment
        )

        for key in ("CARGO_TARGET_DIR", "CARGO_BUILD_BUILD_DIR"):
            with self.subTest(key=key):
                self.assertNotEqual(baseline[key], alternate[key])
        baseline_compilers = [
            json.loads(line)
            for line in baseline_capture.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
        alternate_compilers = [
            json.loads(line)
            for line in alternate_capture.read_text(
                encoding="utf-8"
            ).splitlines()
        ]
        baseline_cc = next(
            item
            for item in baseline_compilers
            if Path(item["path"]).name == "cc"
        )
        alternate_compiler = next(
            item
            for item in alternate_compilers
            if Path(item["path"]).name == "cc"
        )
        baseline_cxx = next(
            item
            for item in baseline_compilers
            if Path(item["path"]).name == "c++"
        )
        alternate_cxx = next(
            item
            for item in alternate_compilers
            if Path(item["path"]).name == "c++"
        )
        self.assertEqual(baseline_cc["path"], str(self.tool_bin / "cc"))
        self.assertEqual(alternate_compiler["path"], str(alternate_cc_path))
        self.assertEqual(baseline_cxx["path"], str(self.tool_bin / "c++"))
        self.assertEqual(alternate_cxx["path"], str(alternate_cxx_path))
        for name, first, second, target in (
            ("CC", baseline_cc, alternate_compiler, "fixture-target"),
            ("CXX", baseline_cxx, alternate_cxx, "fixture-cxx"),
        ):
            with self.subTest(compiler=name):
                self.assertEqual(
                    first["argv"],
                    [f"--target={target}", "-pthread", "--version"],
                )
                self.assertEqual(
                    second["argv"],
                    [f"--target={target}", "-pthread", "--version"],
                )
                self.assertEqual(first["cwd"], str(self.repository))
                self.assertEqual(second["cwd"], str(self.repository))
                self.assertNotEqual(first["version"], second["version"])

    def test_missing_native_compiler_fails_before_cache_files(self) -> None:
        isolated_cache = self.root / "missing-native-compiler-cache"
        result = self._invoke(
            self.repository,
            "--print-env",
            environment=self._environment(
                cache_root=isolated_cache,
                extra={"CC": "missing-cc"},
            ),
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("native compiler is unavailable", result.stderr.lower())
        self.assertFalse(isolated_cache.exists())

    def test_native_compiler_version_failure_precedes_cache_files(
        self,
    ) -> None:
        isolated_cache = self.root / "compiler-version-failure-cache"
        result = self._invoke(
            self.repository,
            "--print-env",
            environment=self._environment(
                cache_root=isolated_cache,
                extra={"MOCK_CC_EXIT": "7"},
            ),
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("native compiler identity failed", result.stderr.lower())
        self.assertFalse(isolated_cache.exists())

    def test_native_compiler_version_timeout_precedes_cache_files(
        self,
    ) -> None:
        isolated_cache = self.root / "compiler-version-timeout-cache"
        environment = self._environment(
            cache_root=isolated_cache,
            extra={"MOCK_CC_SLEEP_SECONDS": "11"},
        )
        started = time.monotonic()
        result = self._invoke(
            self.repository, "--print-env", environment=environment
        )
        elapsed = time.monotonic() - started

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "native compiler identity timed out", result.stderr.lower()
        )
        self.assertGreaterEqual(elapsed, 9.0)
        self.assertFalse(isolated_cache.exists())

    def test_explicit_isolated_cache_mode_disables_workspace_wrapper(
        self,
    ) -> None:
        auto = self._print_environment(self.repository)
        isolated = self._print_environment(
            self.repository, cache_mode="isolated"
        )

        self._assert_isolated(isolated, auto)

    def test_cargo_configuration_overrides_disable_shared_build(self) -> None:
        auto = self._print_environment(self.repository)
        overrides = (
            ("--config", "net.offline=true", "check"),
            ("--manifest-path", str(self.repository / "Cargo.toml"), "check"),
            ("--target-dir", str(self.root / "target-override"), "check"),
            ("-Z", "unstable-options", "check"),
            ("-C", "codegen-units=1", "check"),
            ("+nightly", "check"),
        )

        for command in overrides:
            with self.subTest(command=command):
                selected = self._print_environment(
                    self.repository, command=command
                )
                self._assert_isolated(selected, auto)

    def test_invalid_lane_fails_before_creating_cache_directories(
        self,
    ) -> None:
        isolated_cache = self.root / "invalid-lane-cache"
        result = self._invoke(
            self.repository,
            "--lane",
            "../escape",
            "test",
            environment=self._environment(cache_root=isolated_cache),
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("lane", result.stderr.lower())
        self.assertFalse(isolated_cache.exists())

    def test_external_cache_override_contains_selected_paths(self) -> None:
        external_cache = self.root / "chosen-cache"
        selected = self._print_environment(
            self.repository,
            environment=self._environment(cache_root=external_cache),
        )

        self._assert_cache_paths(selected, external_cache)
        for key in (
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
        ):
            self.assertTrue(Path(selected[key]).is_relative_to(external_cache))
            self.assertTrue(Path(selected[key]).is_file())

    def test_long_sccache_socket_path_fails_clearly(self) -> None:
        long_cache = self.root / f"cache-{'c' * 110}"
        environment = self._environment(cache_root=long_cache)
        socket_path = long_cache / "sccache.sock"
        self.assertGreater(len(os.fsencode(socket_path)), 100)
        result = self._invoke(
            self.repository, "--print-env", environment=environment
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("socket", result.stderr.lower())
        self.assertTrue(
            any(
                word in result.stderr.lower()
                for word in ("long", "100", "length", "limit")
            ),
            result.stderr,
        )
        self.assertFalse(long_cache.exists())

    def test_xdg_cache_home_supplies_default_cache_root(self) -> None:
        xdg_cache = self.root / "xdg-cache"
        environment = self._environment()
        environment.pop("BEVY_BUILD_CACHE", None)
        environment["XDG_CACHE_HOME"] = str(xdg_cache)
        selected = self._print_environment(
            self.repository, environment=environment
        )

        self._assert_cache_paths(selected, xdg_cache / "bevy-build")

    def test_absent_nix_output_paths_are_removed_and_other_flags_remain(
        self,
    ) -> None:
        absent_output = self.root / "nix-output"
        existing = self.root / "native-sdk"
        (existing / "lib").mkdir(parents=True)
        (existing / "include").mkdir(parents=True)
        environment = self._environment(
            extra={
                "out": str(absent_output),
                "NIX_LDFLAGS": (
                    f"-rpath {absent_output}/lib -L {absent_output}/lib "
                    f"-L {existing}/lib -Wl,--as-needed"
                ),
                "NIX_CFLAGS_COMPILE": (
                    f"-I {absent_output}/include "
                    f"-isystem {absent_output}/include "
                    f"-I {existing}/include -DKEEP=1"
                ),
            }
        )
        selected = self._print_environment(
            self.repository, environment=environment
        )

        self.assertEqual(
            shlex.split(selected["NIX_LDFLAGS"]),
            ["-L", str(existing / "lib"), "-Wl,--as-needed"],
        )
        self.assertEqual(
            shlex.split(selected["NIX_CFLAGS_COMPILE"]),
            ["-I", str(existing / "include"), "-DKEEP=1"],
        )

    def test_missing_nix_flags_stay_absent(self) -> None:
        environment = self._environment()
        environment.pop("NIX_LDFLAGS", None)
        environment.pop("NIX_CFLAGS_COMPILE", None)

        selected = self._print_environment(
            self.repository, environment=environment
        )

        self.assertNotIn("NIX_LDFLAGS", selected)
        self.assertNotIn("NIX_CFLAGS_COMPILE", selected)

    def test_missing_required_tools_fail_with_tool_name(self) -> None:
        for missing in ("cargo", "rustc", "sccache", "ccache"):
            with self.subTest(missing=missing):
                fixture_path = self.root / f"tools-without-{missing}"
                shutil.copytree(self.tool_bin, fixture_path, symlinks=True)
                (fixture_path / missing).unlink()
                result = self._invoke(
                    self.repository,
                    "--print-env",
                    environment=self._environment(tool_bin=fixture_path),
                )

                self.assertNotEqual(result.returncode, 0)
                self.assertIn(missing, result.stderr.lower())
                if missing in ("sccache", "ccache"):
                    self.assertTrue(
                        any(
                            word in result.stderr.lower()
                            for word in ("install", "path", "nix")
                        ),
                        result.stderr,
                    )

    def test_default_command_preserves_compilers_and_propagates_cargo_status(
        self,
    ) -> None:
        capture = self.root / "cargo-invocation.json"
        environment = self._environment(
            extra={
                "CARGO_FAST_CAPTURE": str(capture),
                "CARGO_MOCK_EXIT": "37",
                "CC": "fixture-clang --target=x86_64-unknown-linux-gnu",
                "CXX": "fixture-clang++ -std=c++20",
            }
        )
        expected_environment = self._print_environment(
            self.repository,
            command=("test", "--workspace", "--all-features"),
            environment=environment,
        )
        environment["CARGO_FAST_ENV_KEYS"] = json.dumps(
            sorted(expected_environment)
        )
        result = self._invoke(
            self.repository,
            "test",
            "--workspace",
            "--all-features",
            environment=environment,
        )

        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertTrue(capture.exists())
        invocation = json.loads(capture.read_text(encoding="utf-8"))
        self.assertEqual(
            invocation["argv"], ["test", "--workspace", "--all-features"]
        )
        self.assertEqual(invocation["cwd"], str(self.repository))
        self.assertEqual(invocation["env"], expected_environment)
        self.assertNotEqual(
            invocation["env"]["RUSTC_WRAPPER"],
            str(SCRIPT.with_name("rustc-cache.py").resolve()),
        )
        self.assertTrue(Path(invocation["env"]["RUSTC_WRAPPER"]).is_file())
        self.assertEqual(invocation["env"]["RUSTC_WORKSPACE_WRAPPER"], "")
        self.assertEqual(
            shlex.split(invocation["env"]["CC"]),
            [
                str(self.tool_bin / "ccache"),
                "fixture-clang",
                "--target=x86_64-unknown-linux-gnu",
            ],
        )
        self.assertEqual(
            shlex.split(invocation["env"]["CXX"]),
            [str(self.tool_bin / "ccache"), "fixture-clang++", "-std=c++20"],
        )
        self.assertNotIn("CMAKE_C_COMPILER_LAUNCHER", invocation["env"])
        self.assertNotIn("CMAKE_CXX_COMPILER_LAUNCHER", invocation["env"])
        self._assert_cache_paths(invocation["env"], self.cache_root)
        self.assertTrue(Path(invocation["env"]["CARGO_TARGET_DIR"]).is_dir())
        self.assertTrue(
            Path(invocation["env"]["CARGO_BUILD_BUILD_DIR"]).is_dir()
        )

    def test_no_arguments_runs_locked_build(self) -> None:
        capture = self.root / "default-cargo-invocation.json"
        environment = self._environment(
            extra={"CARGO_FAST_CAPTURE": str(capture)}
        )
        expected_environment = self._print_environment(
            self.repository, environment=environment
        )
        environment["CARGO_FAST_ENV_KEYS"] = json.dumps(
            sorted(expected_environment)
        )

        result = self._invoke(self.repository, environment=environment)

        self.assertEqual(result.returncode, 0, result.stderr)
        invocation = json.loads(capture.read_text(encoding="utf-8"))
        self.assertEqual(invocation["argv"], ["build", "--locked"])
        self.assertEqual(invocation["env"], expected_environment)

    def test_command_from_nested_directory_runs_cargo_at_repository_root(
        self,
    ) -> None:
        nested = self.repository / "src"
        capture = self.root / "nested-cargo-invocation.json"
        result = self._invoke(
            nested,
            "check",
            environment=self._environment(
                extra={"CARGO_FAST_CAPTURE": str(capture)}
            ),
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        invocation = json.loads(capture.read_text(encoding="utf-8"))
        self.assertEqual(invocation["cwd"], str(self.repository))
        self.assertEqual(invocation["argv"], ["check"])

    def test_nearest_cargo_manifest_under_git_sets_build_directory_and_cwd(
        self,
    ) -> None:
        generated = self.repository / "target" / "generated" / "child"
        generated.mkdir(parents=True)
        (generated / "Cargo.toml").write_text(
            '[workspace]\n[package]\nname = "generated-child"\n'
            'version = "0.1.0"\nedition = "2024"\n',
            encoding="utf-8",
        )
        root_environment = self._print_environment(self.repository)
        child_environment = self._print_environment(generated)
        capture = self.root / "nested-manifest-cargo-invocation.json"
        result = self._invoke(
            generated,
            "check",
            environment=self._environment(
                extra={"CARGO_FAST_CAPTURE": str(capture)}
            ),
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        invocation = json.loads(capture.read_text(encoding="utf-8"))
        self.assertEqual(invocation["cwd"], str(generated))
        self.assertEqual(invocation["argv"], ["check"])
        self.assertIn("CARGO_TARGET_DIR", child_environment)
        self.assertNotEqual(
            child_environment["CARGO_TARGET_DIR"],
            root_environment["CARGO_TARGET_DIR"],
        )
        self.assertNotEqual(
            child_environment["RUSTC_WORKSPACE_WRAPPER"],
            root_environment["RUSTC_WORKSPACE_WRAPPER"],
        )

    def test_nested_workspace_member_runs_cargo_at_workspace_root(
        self,
    ) -> None:
        manifest = self.repository / "Cargo.toml"
        manifest.write_text(
            '[package]\nname = "cache-test"\nversion = "0.1.0"\n'
            'edition = "2024"\n\n[workspace]\n'
            'members = ["nested/member"]\nresolver = "3"\n',
            encoding="utf-8",
        )
        nested = self.repository / "nested" / "member"
        nested.mkdir(parents=True)
        (nested / "Cargo.toml").write_text(
            '[package]\nname = "workspace-member"\n'
            'version = "0.1.0"\nedition = "2024"\n',
            encoding="utf-8",
        )
        capture = self.root / "workspace-member-cargo-invocation.json"
        environment = self._environment(
            extra={
                "CARGO_FAST_CAPTURE": str(capture),
                "MOCK_WORKSPACE_ROOT": str(self.repository / "Cargo.toml"),
            }
        )
        result = self._invoke(nested, "check", environment=environment)

        self.assertEqual(result.returncode, 0, result.stderr)
        invocation = json.loads(capture.read_text(encoding="utf-8"))
        self.assertEqual(invocation["cwd"], str(self.repository))
        self.assertEqual(invocation["argv"], ["check"])

    def test_no_git_on_path_uses_workspace_paths_and_shared_compiler_cache(
        self,
    ) -> None:
        tools_without_git = self.root / "tools-without-git"
        shutil.copytree(self.tool_bin, tools_without_git, symlinks=True)
        (tools_without_git / "git").unlink()
        environment = self._environment(tool_bin=tools_without_git)
        repository_environment = self._print_environment(
            self.repository, environment=environment
        )
        worktree_environment = self._print_environment(
            self.worktree, environment=environment
        )
        clone_environment = self._print_environment(
            self.clone, environment=environment
        )

        for key in ("CARGO_TARGET_DIR", "CARGO_BUILD_BUILD_DIR"):
            with self.subTest(key=key):
                self.assertNotEqual(
                    repository_environment[key], worktree_environment[key]
                )
                self.assertNotEqual(
                    repository_environment[key], clone_environment[key]
                )
        for key in ("SCCACHE_DIR", "CCACHE_DIR"):
            with self.subTest(key=key):
                self.assertEqual(
                    repository_environment[key], worktree_environment[key]
                )
                self.assertEqual(
                    repository_environment[key], clone_environment[key]
                )

    def test_locate_project_failure_is_friendly_before_cache_creation(
        self,
    ) -> None:
        isolated_cache = self.root / "locate-project-failure-cache"
        result = self._invoke(
            self.repository,
            "--print-env",
            environment=self._environment(
                cache_root=isolated_cache,
                extra={"MOCK_LOCATE_EXIT": "23"},
            ),
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("cargo locate-project failed", result.stderr.lower())
        self.assertFalse(isolated_cache.exists())

    def test_windows_support_branch_fails_before_cache_discovery(self) -> None:
        specification = importlib.util.spec_from_file_location(
            "cargo_fast", SCRIPT
        )
        self.assertIsNotNone(specification)
        self.assertIsNotNone(specification.loader)
        cargo_fast = importlib.util.module_from_spec(specification)
        specification.loader.exec_module(cargo_fast)
        isolated_cache = self.root / "unsupported-platform-cache"

        with (
            patch.object(cargo_fast.sys, "platform", "win32"),
            patch.object(cargo_fast.shutil, "which") as find_tool,
            patch.dict(
                os.environ,
                {"BEVY_BUILD_CACHE": str(isolated_cache)},
            ),
        ):
            with self.assertRaisesRegex(
                ValueError, "ordinary Cargo on Windows"
            ):
                cargo_fast.build_environment(self.repository, None)

        find_tool.assert_not_called()
        self.assertFalse(isolated_cache.exists())

    def test_clean_is_rejected_before_cache_creation(self) -> None:
        isolated_cache = self.root / "clean-cache"
        result = self._invoke(
            self.repository,
            "clean",
            environment=self._environment(cache_root=isolated_cache),
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("shared cache", result.stderr.lower())
        self.assertFalse(isolated_cache.exists())

    def test_clean_command_after_cargo_global_options_is_rejected(
        self,
    ) -> None:
        prefixes = (
            ("--config", "net.offline=true"),
            ("--color", "always"),
            ("-Z", "unstable-options"),
            ("-C", "net.offline=true"),
            ("+nightly",),
        )
        for prefix in prefixes:
            with self.subTest(prefix=prefix):
                result = self._invoke(self.repository, *prefix, "clean")

                self.assertNotEqual(result.returncode, 0)
                self.assertIn("shared cache", result.stderr.lower())

    def test_package_named_clean_and_clean_binary_argument_are_forwarded(
        self,
    ) -> None:
        for arguments in (
            ("run", "--package", "clean"),
            ("run", "--", "--clean"),
        ):
            with self.subTest(arguments=arguments):
                capture = self.root / "cargo-clean-argument.json"
                environment = self._environment(
                    extra={"CARGO_FAST_CAPTURE": str(capture)}
                )
                result = self._invoke(
                    self.repository, *arguments, environment=environment
                )

                self.assertEqual(result.returncode, 0, result.stderr)
                invocation = json.loads(capture.read_text(encoding="utf-8"))
                self.assertEqual(invocation["argv"], list(arguments))

    def test_shell_environment_is_shell_quoted_and_matches_json(self) -> None:
        external_cache = self.root / "cache with ' quote"
        shell_capture = (
            self.root / "shell-env-unexpected-cargo-invocation.json"
        )
        capture = self.root / "shell-env-cargo-invocation.json"
        environment = self._environment(
            cache_root=external_cache,
            extra={
                "CC": "fixture-clang --target=x86_64-unknown-linux-gnu",
                "CXX": "fixture-clang++ -std=c++20",
                "CARGO_FAST_CAPTURE": str(shell_capture),
            },
        )
        auto_environment = self._print_environment(
            self.repository, command=("check",), environment=environment
        )
        result = self._invoke(
            self.repository, "--shell-env", environment=environment
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(shell_capture.exists())
        shell_environment: dict[str, str] = {}
        for line in result.stdout.splitlines():
            fields = shlex.split(line)
            self.assertEqual(len(fields), 2, line)
            self.assertEqual(fields[0], "export")
            key, separator, value = fields[1].partition("=")
            self.assertTrue(separator, line)
            shell_environment[key] = value

        self.assertNotEqual(
            shell_environment["CARGO_BUILD_BUILD_DIR"],
            auto_environment["CARGO_BUILD_BUILD_DIR"],
        )
        self.assertEqual(shell_environment["RUSTC_WORKSPACE_WRAPPER"], "")
        self.assertEqual(
            shell_environment["RUSTC_WRAPPER"],
            auto_environment["RUSTC_WRAPPER"],
        )
        self._assert_cache_paths(shell_environment, external_cache)

        # Reapply exported compiler values, as a shell would.
        environment["CC"] = shell_environment["CC"]
        environment["CXX"] = shell_environment["CXX"]
        reapplied_environment = self._print_environment(
            self.repository, command=("check",), environment=environment
        )
        for key in ("CC", "CXX"):
            self.assertEqual(
                reapplied_environment[key], shell_environment[key]
            )
        environment["CARGO_FAST_CAPTURE"] = str(capture)
        environment["CARGO_FAST_ENV_KEYS"] = json.dumps(
            sorted(reapplied_environment)
        )
        invocation = self._invoke(
            self.repository, "check", environment=environment
        )

        self.assertEqual(invocation.returncode, 0, invocation.stderr)
        cargo_environment = json.loads(capture.read_text(encoding="utf-8"))[
            "env"
        ]
        self.assertEqual(cargo_environment, reapplied_environment)
        for key in ("CC", "CXX"):
            with self.subTest(key=key):
                compiler = shlex.split(cargo_environment[key])
                self.assertEqual(
                    compiler.count(str(self.tool_bin / "ccache")), 1
                )

    def test_isolated_build_clears_inherited_workspace_wrapper(self) -> None:
        inherited_wrapper = self.root / "parent-workspace-wrapper"
        inherited_wrapper.write_text("unused fixture\n", encoding="utf-8")
        capture = self.root / "isolated-cargo-invocation.json"
        environment = self._environment(
            extra={
                "RUSTC_WORKSPACE_WRAPPER": str(inherited_wrapper),
                "CARGO_FAST_CAPTURE": str(capture),
            }
        )
        selected = self._print_environment(
            self.repository, command=("clippy",), environment=environment
        )
        self.assertEqual(selected["RUSTC_WORKSPACE_WRAPPER"], "")
        environment["CARGO_FAST_ENV_KEYS"] = json.dumps(sorted(selected))

        result = self._invoke(
            self.repository, "clippy", environment=environment
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        invocation = json.loads(capture.read_text(encoding="utf-8"))
        self.assertEqual(invocation["argv"], ["clippy"])
        self.assertEqual(invocation["env"]["RUSTC_WORKSPACE_WRAPPER"], "")

    def test_cargo_arguments_with_shell_syntax_are_forwarded_literally(
        self,
    ) -> None:
        marker = self.root / "shell-command-ran"
        payload = f"space ; $(touch {marker})"
        arguments = ("test", "--", f"--payload={payload}")
        capture = self.root / "literal-argv-cargo-invocation.json"
        environment = self._environment(
            extra={"CARGO_FAST_CAPTURE": str(capture)}
        )
        environment["CARGO_FAST_ENV_KEYS"] = json.dumps(
            sorted(
                self._print_environment(
                    self.repository, command=("test",), environment=environment
                )
            )
        )

        result = self._invoke(
            self.repository, *arguments, environment=environment
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        invocation = json.loads(capture.read_text(encoding="utf-8"))
        self.assertEqual(invocation["argv"], list(arguments))
        self.assertFalse(marker.exists())

    def test_cargo_toml_parent_fallback_works_without_git_metadata(
        self,
    ) -> None:
        archive = self.root / "archive"
        nested = archive / "nested"
        nested.mkdir(parents=True)
        (archive / "Cargo.toml").write_text(
            '[workspace]\nmembers = []\nresolver = "3"\n',
            encoding="utf-8",
        )
        selected = self._print_environment(nested)

        self.assertIn("CARGO_TARGET_DIR", selected)
        self.assertIn("CARGO_BUILD_BUILD_DIR", selected)

    def test_outside_cargo_project_fails_before_creating_cache(self) -> None:
        outside = self.root / "outside-cargo-project"
        outside.mkdir()
        isolated_cache = self.root / "outside-project-cache"
        result = self._invoke(
            outside,
            "--print-env",
            environment=self._environment(cache_root=isolated_cache),
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("inside a cargo project", result.stderr.lower())
        self.assertFalse(isolated_cache.exists())


if __name__ == "__main__":
    unittest.main()
