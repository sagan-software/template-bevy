#!/usr/bin/env python3
"""Run Cargo with repository-scoped worktree caches and compiler caches.

Before a build the wrapper evicts cache entries under repositories/ that no
live process uses. Each entry is one worktree or lane folder, or a
repository's shared dependencies folder. An entry is in use while a wrapper
holds its lock or a process names it in CARGO_TARGET_DIR or
CARGO_BUILD_BUILD_DIR. Defaults, as Bill decided on 2026-11-13:

- BEVY_BUILD_MAX_IDLE_HOURS=6: remove entries unused for longer.
- BEVY_BUILD_MAX_GIB=200: then remove least recently used entries until
  repositories/ holds at most this many GiB.
- BEVY_BUILD_EVICT_INTERVAL_SECONDS=600: check at most this often.

A value of 0 turns that limit off.
"""

import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import time

DEFAULT_MAX_IDLE_HOURS = 6.0
DEFAULT_MAX_GIB = 200.0
DEFAULT_EVICT_INTERVAL_SECONDS = 600.0
STAMP = ".cargo-fast-used"


def lane_name(value: str) -> str:
    """Reject lane names that could escape or alias the cache directory."""
    if re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]{0,63}", value) is None:
        raise argparse.ArgumentTypeError(
            "lane requires 1–64 letters, digits, underscores or hyphens"
        )
    return value


def project_root(directory: Path) -> Path:
    """Find the nearest workspace, including generated archives without Git."""
    if shutil.which("cargo") is None:
        raise ValueError("cargo is required; install it or enter nix develop")
    for candidate in (directory, *directory.parents):
        if (candidate / "Cargo.toml").is_file():
            # Cargo locates the workspace without resolving dependencies.
            located = subprocess.run(
                [
                    "cargo",
                    "locate-project",
                    "--workspace",
                    "--message-format",
                    "plain",
                    "--manifest-path",
                    str(candidate / "Cargo.toml"),
                ],
                cwd=candidate,
                text=True,
                capture_output=True,
                check=False,
            )
            if located.returncode != 0:
                detail = located.stderr.strip()
                raise ValueError(f"cargo locate-project failed: {detail}")
            return Path(located.stdout.strip()).resolve().parent
    raise ValueError("run inside a Cargo project")


def native_environment() -> dict[str, str]:
    """Remove absent developer-shell output search paths from Nix flags."""
    result = {}
    output = os.environ.get("out")
    for name in ("NIX_LDFLAGS", "NIX_CFLAGS_COMPILE"):
        if name not in os.environ:
            continue
        original = os.environ[name]
        words = shlex.split(original)
        kept = []
        index = 0
        while index < len(words):
            word = words[index]
            paired = word in ("-rpath", "-L", "-I", "-isystem")
            path = (
                words[index + 1] if paired and index + 1 < len(words) else ""
            )
            absent_output = (
                output
                and path
                in (str(Path(output) / "lib"), str(Path(output) / "include"))
                and not Path(path).exists()
            )
            if absent_output:
                index += 2
                continue
            kept.append(word)
            index += 1
        result[name] = original if kept == words else shlex.join(kept)
    return result


def requested_features(arguments: list[str]) -> list[str]:
    """Forward Cargo feature switches without examining runtime arguments."""
    features = []
    index = 0
    while index < len(arguments) and arguments[index] != "--":
        word = arguments[index]
        if word in ("--features", "-F"):
            features.append(word)
            index += 1
            if index < len(arguments):
                features.append(arguments[index])
        elif word in (
            "--all-features",
            "--no-default-features",
        ) or word.startswith(("--features=", "-F")):
            features.append(word)
        index += 1
    return features


def dependency_graph_is_shareable(
    root: Path,
    cargo_home: Path,
    arguments: list[str],
) -> bool:
    """Require immutable dependencies or namespaced workspace members."""
    try:
        result = subprocess.run(
            [
                "cargo",
                "metadata",
                "--locked",
                "--format-version",
                "1",
                *requested_features(arguments),
            ],
            cwd=root,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            return False
        metadata = json.loads(result.stdout)
        if (
            type(metadata["version"]) is not int
            or metadata["version"] != 1
            or Path(metadata["workspace_root"]).resolve() != root
        ):
            return False
        members = metadata["workspace_members"]
        packages = metadata["packages"]
        if not isinstance(members, list) or not all(
            isinstance(member, str) for member in members
        ):
            return False
        if not isinstance(packages, list) or not packages:
            return False
        seen = set()
        for package in packages:
            identity = package["id"]
            source = package["source"]
            manifest = Path(package["manifest_path"]).resolve()
            if not isinstance(identity, str) or identity in seen:
                return False
            seen.add(identity)
            if identity in members:
                continue
            # Isolate unknown sources and mutable path/vendor packages.
            if not isinstance(source, str) or not source.startswith(
                ("registry+", "sparse+", "git+")
            ):
                return False
            if manifest.is_relative_to(root):
                return False
            if not any(
                manifest.is_relative_to(path)
                for path in (
                    cargo_home / "registry" / "src",
                    cargo_home / "git" / "checkouts",
                    Path("/nix/store"),
                )
            ):
                return False
        return set(members).issubset(seen)
    except (OSError, ValueError, KeyError, TypeError):
        return False


def cached_launcher(source: Path, destination: Path) -> str:
    """Install an executable once and retain its timestamp when unchanged."""
    interpreter = str(Path(sys.executable).resolve())
    content = source.read_bytes().replace(
        b"#!/usr/bin/env python3",
        f"#!{interpreter}".encode(),
        1,
    )
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.is_file() and destination.read_bytes() == content:
        return str(destination)
    descriptor, temporary = tempfile.mkstemp(dir=destination.parent)
    try:
        with os.fdopen(descriptor, "wb") as file:
            file.write(content)
        os.chmod(temporary, 0o700)
        os.replace(temporary, destination)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
    return str(destination)


def build_environment(
    root: Path,
    lane: str | None,
    arguments: list[str] | None = None,
    is_isolated: bool = False,
) -> dict[str, str]:
    """Derive caches from repository, compiler and worktree identities."""
    if sys.platform == "win32":
        raise ValueError(
            "cache launchers require Unix; use ordinary Cargo on Windows"
        )
    tools = {}
    for name in ("cargo", "rustc", "sccache", "ccache"):
        executable = shutil.which(name)
        if executable is None:
            raise ValueError(
                f"{name} is required; install it or enter nix develop"
            )
        tools[name] = executable

    # Linked worktrees share the canonical common Git directory; clones do not.
    identity = root
    if shutil.which("git"):
        common = subprocess.run(
            ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
            cwd=root,
            capture_output=True,
            text=True,
            check=False,
        )
        if common.returncode == 0:
            identity = Path(common.stdout.strip()).resolve()
    compiler = subprocess.check_output([tools["rustc"], "-vV"], cwd=root)
    cargo = subprocess.check_output([tools["cargo"], "--version"], cwd=root)
    native = native_environment()
    compiler_commands = {
        "CC": cached_compiler(tools["ccache"], os.environ.get("CC", "cc")),
        "CXX": cached_compiler(tools["ccache"], os.environ.get("CXX", "c++")),
    }
    settings = (
        native
        | {
            name: os.environ.get(name, "")
            for name in (
                "RUSTFLAGS",
                "CARGO_ENCODED_RUSTFLAGS",
            )
        }
        | compiler_commands
    )
    settings["compiler-identities"] = {
        name: compiler_identity(command)
        for name, command in compiler_commands.items()
    }
    key = hashlib.sha256(
        os.fsencode(identity)
        + b"\0"
        + compiler
        + b"\0"
        + cargo
        + json.dumps(settings, sort_keys=True).encode()
    ).hexdigest()
    default_cache = (
        Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache"))
        / "bevy-build"
    )
    cache = (
        Path(os.environ.get("BEVY_BUILD_CACHE", default_cache))
        .expanduser()
        .resolve()
    )
    worktree = hashlib.sha256(os.fsencode(root)).hexdigest()
    artifacts = cache / "repositories" / key / (lane or worktree)
    repository = cache / "repositories" / key
    socket = str(cache / "sccache.sock")
    if len(os.fsencode(socket)) > 100:
        raise ValueError(
            "cache path is too long for a Unix socket; "
            "shorten BEVY_BUILD_CACHE"
        )
    arguments = arguments or ["build", "--locked"]
    scope = arguments[
        : arguments.index("--") if "--" in arguments else len(arguments)
    ]
    overrides = any(
        word.startswith(
            ("--config", "--manifest-path", "--target-dir", "-Z", "+")
        )
        or word == "-C"
        for word in scope
    )
    shared = (
        not is_isolated
        and not overrides
        and cargo_subcommand(arguments) in ("build", "check", "run", "test")
        and dependency_graph_is_shareable(
            root,
            Path(
                os.environ.get("CARGO_HOME", Path.home() / ".cargo")
            ).resolve(),
            arguments,
        )
    )
    scripts = Path(__file__).parent

    # Isolate outputs by default; serialize commands in a shared lane.
    environment = {
        "CARGO_TARGET_DIR": str(artifacts / "target"),
        "CARGO_BUILD_BUILD_DIR": str(
            repository / "dependencies" / "build"
            if shared
            else artifacts / "worktrees" / worktree / "build"
        ),
        "RUSTC_WRAPPER": cached_launcher(
            scripts / "rustc-cache.py",
            repository / "launchers" / "rustc-cache",
        ),
        "BEVY_CARGO_CACHE_MODE": (
            "shared-dependencies" if shared else "isolated"
        ),
        "SCCACHE_DIR": str(cache / "sccache"),
        "SCCACHE_BASEDIRS": str(root),
        "SCCACHE_CACHE_SIZE": os.environ.get("SCCACHE_CACHE_SIZE", "20G"),
        "CCACHE_DIR": str(cache / "ccache"),
        "CCACHE_BASEDIR": str(root),
        "CCACHE_MAXSIZE": os.environ.get("CCACHE_MAXSIZE", "5G"),
    }
    environment.update(native)
    environment.update(compiler_commands)
    if shared:
        environment["RUSTC_WORKSPACE_WRAPPER"] = cached_launcher(
            scripts / "workspace-rustc.py",
            artifacts
            / "worktrees"
            / worktree
            / "launchers"
            / "workspace-rustc",
        )
    else:
        environment["RUSTC_WORKSPACE_WRAPPER"] = ""
    # Keep this cache separate from unrelated sccache daemons.
    if os.name != "nt":
        environment["SCCACHE_SERVER_UDS"] = socket
    return environment


def cached_compiler(cache: str, compiler: str) -> str:
    """Avoid nesting cache launchers when setup runs repeatedly."""
    words = shlex.split(compiler)
    if words and Path(words[0]).name in ("ccache", "ccache.exe"):
        return compiler
    return f"{shlex.quote(cache)} {compiler}"


def compiler_identity(command: str) -> dict[str, str | int]:
    """Identify the actual native compiler behind its cache launcher."""
    words = shlex.split(command)
    if words and Path(words[0]).name in ("ccache", "ccache.exe"):
        words = words[1:]
    executable = shutil.which(words[0]) if words else None
    if executable is None:
        raise ValueError(f"native compiler is unavailable: {command}")
    path = Path(executable).resolve()
    try:
        result = subprocess.run(
            [str(path), *words[1:], "--version"],
            capture_output=True,
            text=True,
            check=False,
            timeout=10,
        )
    except subprocess.TimeoutExpired as error:
        raise ValueError(
            f"native compiler identity timed out: {command}"
        ) from error
    if result.returncode != 0:
        raise ValueError(f"native compiler identity failed: {command}")
    metadata = path.stat()
    return {
        "path": str(path),
        "version": result.stdout + result.stderr,
        "size": metadata.st_size,
        "mtime": metadata.st_mtime_ns,
    }


def limit(name: str, default: float) -> float:
    """Read a non-negative limit; 0 turns it off."""
    value = os.environ.get(name, "").strip()
    if not value:
        return default
    try:
        number = float(value)
    except ValueError as error:
        raise ValueError(f"{name} must be a number: {value}") from error
    if number < 0 or number != number:
        raise ValueError(f"{name} must be 0 or more: {value}")
    return number


def entry_lock(cache: Path, entry: Path) -> Path:
    """Keep locks outside entries so removing one keeps its lock valid."""
    name = hashlib.sha256(os.fsencode(entry)).hexdigest()
    return cache / "locks" / f"{name}.lock"


def lock_entry(cache: Path, entry: Path, shared: bool) -> int | None:
    """Lock an entry; return None when a shared or exclusive lock is held."""
    path = entry_lock(cache, entry)
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_RDWR | os.O_CREAT, 0o600)
    try:
        if shared:
            fcntl.flock(descriptor, fcntl.LOCK_SH)
        else:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(descriptor)
        return None
    return descriptor


def cache_entries(cache: Path) -> list[Path]:
    """List worktree, lane and dependency folders of every repository."""
    entries = []
    repositories = cache / "repositories"
    if not repositories.is_dir():
        return entries
    for repository in sorted(repositories.iterdir()):
        if repository.is_dir() and not repository.is_symlink():
            entries.extend(
                child
                for child in sorted(repository.iterdir())
                if child.is_dir()
                and not child.is_symlink()
                and child.name != "launchers"
            )
    return entries


def last_used(entry: Path) -> float:
    """Take the newest of the stamp and the folders Cargo writes on use."""
    newest = 0.0
    for path in (
        entry / STAMP,
        entry,
        entry / "target",
        entry / "build",
        *entry.glob("target/*"),
        *entry.glob("worktrees/*/build"),
    ):
        try:
            newest = max(newest, path.lstat().st_mtime)
        except OSError:
            pass
    return newest


def disk_usage(path: Path) -> int:
    """Count allocated bytes once per inode, without following links."""
    total = 0
    seen = set()
    pending = [path]
    while pending:
        try:
            with os.scandir(pending.pop()) as children:
                for child in children:
                    try:
                        status = child.stat(follow_symlinks=False)
                    except OSError:
                        continue
                    if status.st_nlink > 1:
                        inode = (status.st_dev, status.st_ino)
                        if inode in seen:
                            continue
                        seen.add(inode)
                    total += status.st_blocks * 512
                    if child.is_dir(follow_symlinks=False):
                        pending.append(Path(child.path))
        except OSError:
            continue
    return total


def named_by_processes(proc: Path = Path("/proc")) -> set[Path]:
    """Find cache folders that live processes name as Cargo outputs."""
    named = set()
    try:
        processes = list(proc.iterdir())
    except OSError:
        return named
    for process in processes:
        if not process.name.isdigit():
            continue
        try:
            environ = (process / "environ").read_bytes()
        except OSError:
            continue
        for variable in environ.split(b"\0"):
            name, _, value = variable.partition(b"=")
            if name in (b"CARGO_TARGET_DIR", b"CARGO_BUILD_BUILD_DIR"):
                named.add(Path(os.fsdecode(value)))
    return named


def evict(
    cache: Path,
    keep: list[Path],
    now: float | None = None,
    proc: Path = Path("/proc"),
) -> list[Path]:
    """Remove idle and least recently used entries; return removed ones."""
    max_idle = limit("BEVY_BUILD_MAX_IDLE_HOURS", DEFAULT_MAX_IDLE_HOURS)
    max_bytes = limit("BEVY_BUILD_MAX_GIB", DEFAULT_MAX_GIB) * 1024**3
    interval = limit(
        "BEVY_BUILD_EVICT_INTERVAL_SECONDS", DEFAULT_EVICT_INTERVAL_SECONDS
    )
    now = time.time() if now is None else now
    marker = cache / "last-eviction"
    try:
        if now - marker.stat().st_mtime < interval:
            return []
    except OSError:
        pass
    cache.mkdir(parents=True, exist_ok=True)
    marker.touch()
    os.utime(marker, (now, now))
    named = named_by_processes(proc)
    keep = [path.resolve() for path in keep]
    candidates = []
    for entry in cache_entries(cache):
        resolved = entry.resolve()
        if any(
            resolved == path or resolved in path.parents for path in keep
        ) or any(
            resolved == path or resolved in path.parents for path in named
        ):
            continue
        candidates.append((last_used(entry), entry))
    candidates.sort(key=lambda candidate: candidate[0])
    removed = []

    def remove(entry: Path) -> bool:
        descriptor = lock_entry(cache, entry, shared=False)
        if descriptor is None:
            return False
        try:
            shutil.rmtree(entry, ignore_errors=True)
        finally:
            os.close(descriptor)
        removed.append(entry)
        return True

    remaining = []
    for used, entry in candidates:
        if max_idle and now - used > max_idle * 3600 and remove(entry):
            continue
        remaining.append(entry)
    if max_bytes:
        total = disk_usage(cache / "repositories")
        for entry in remaining:
            if total <= max_bytes:
                break
            size = disk_usage(entry)
            if remove(entry):
                total -= size
    for entry in removed:
        print(f"cargo-fast: removed unused cache entry {entry}", file=sys.stderr)
    return removed


def cargo_subcommand(arguments: list[str]) -> str | None:
    """Find the Cargo command after its global flags."""
    index = 0
    while index < len(arguments):
        value = arguments[index]
        if value in ("--config", "--color", "-Z", "-C"):
            index += 2
        elif value.startswith(("-", "+")):
            index += 1
        else:
            return value
    return None


def main() -> int:
    """Print cache settings or forward Cargo arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lane", type=lane_name)
    parser.add_argument(
        "--cache-mode", choices=("auto", "isolated"), default="auto"
    )
    output = parser.add_mutually_exclusive_group()
    output.add_argument("--print-env", action="store_true")
    output.add_argument("--shell-env", action="store_true")
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    options, cargo_options = parser.parse_known_args()
    arguments = cargo_options + options.arguments
    if arguments[:1] == ["--"]:
        arguments = arguments[1:]
    # Cleaning can discard another worktree's verified artifacts.
    if cargo_subcommand(arguments) == "clean":
        parser.error(
            "shared cache is protected; "
            "use ordinary Cargo with an isolated target to clean"
        )
    try:
        root = project_root(Path.cwd().resolve())
        environment = build_environment(
            root,
            options.lane,
            arguments,
            is_isolated=options.shell_env or options.cache_mode == "isolated",
        )
        if options.print_env:
            print(json.dumps(environment, indent=2))
            return 0
        # Hold this build's entries so other wrappers never evict them.
        cache = Path(environment["SCCACHE_DIR"]).parent
        repositories = cache / "repositories"
        entries = [
            repositories.joinpath(*path.relative_to(repositories).parts[:2])
            for path in (
                Path(environment["CARGO_TARGET_DIR"]),
                Path(environment["CARGO_BUILD_BUILD_DIR"]),
            )
        ]
        for entry in entries:
            # The lock lasts until this process exits.
            lock_entry(cache, entry, shared=True)
        evict(cache, entries)
        for name in (
            "CARGO_TARGET_DIR",
            "CARGO_BUILD_BUILD_DIR",
            "SCCACHE_DIR",
            "CCACHE_DIR",
        ):
            Path(environment[name]).mkdir(parents=True, exist_ok=True)
        for entry in entries:
            (entry / STAMP).touch()
        if options.shell_env:
            for name, value in environment.items():
                print(f"export {name}={shlex.quote(value)}")
            return 0
        return subprocess.run(
            ["cargo", *(arguments or ["build", "--locked"])],
            cwd=root,
            env=os.environ | environment,
            check=False,
        ).returncode
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"cargo-fast: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
