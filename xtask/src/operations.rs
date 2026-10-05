//! Project workflows shared by ordinary Cargo and Nix entry points.

use crate::cli::Operation;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

/// Runs the selected operation with failures propagated to the CLI.
pub(crate) fn execute(operation: Operation) -> io::Result<()> {
    match operation {
        Operation::Imports => crate::imports::check(),
        Operation::Setup => setup(),
        Operation::Book => run("mdbook", &["build", "docs"]),
        Operation::Coverage => coverage(),
        Operation::Features => features(),
        Operation::GenerateMatrix => generate_matrix(),
        Operation::Check => check(),
    }
}

/// Preserves user-owned instructions and refreshes only the generated link.
fn setup() -> io::Result<()> {
    if !Path::new("AGENTS.md").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "AGENTS.md is required before setup",
        ));
    }
    match fs::symlink_metadata("CLAUDE.md") {
        Ok(metadata) if !metadata.file_type().is_symlink() => return Ok(()),
        Ok(_) => fs::remove_file("CLAUDE.md")?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    create_link(Path::new("AGENTS.md"), Path::new("CLAUDE.md"))
}

/// Creates a relative file link on Unix.
#[cfg(unix)]
fn create_link(source: &Path, destination: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(source, destination)
}

/// Uses a symlink on Windows, which requires Developer Mode or link permission.
#[cfg(windows)]
fn create_link(source: &Path, destination: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(source, destination)
}

/// Reports unsupported link semantics on other platforms.
#[cfg(not(any(unix, windows)))]
fn create_link(_source: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "instruction links require Unix or Windows",
    ))
}

/// Runs a command without a shell and names any failed operation.
fn run(program: &str, arguments: &[&str]) -> io::Result<()> {
    run_in(Path::new("."), program, arguments)
}

/// Runs an operation in the selected project instead of changing process state.
fn run_in(directory: &Path, program: &str, arguments: &[&str]) -> io::Result<()> {
    // Run argv directly so paths and values never enter shell expansion.
    let status = child_command(program)
        .current_dir(directory)
        .args(arguments)
        .status()
        .map_err(|error| {
            io::Error::new(error.kind(), format!("could not run {program}: {error}"))
        })?;
    // A failed child status must stop the remaining workflow.
    if status.success() {
        Ok(())
    } else {
        let arguments = arguments.join(" ");
        Err(io::Error::other(format!(
            "{program} {arguments} failed with {status}"
        )))
    }
}

/// Adapts Trunk's Boolean color setting without changing the parent environment.
fn child_command(program: &str) -> Command {
    let mut command = Command::new(program);
    // Trunk parses NO_COLOR as a Boolean rather than accepting every nonempty value.
    if program == "trunk" {
        command.env("NO_COLOR", "true");
    }
    command
}

/// Checks each independently supported optional build mode.
fn features() -> io::Result<()> {
    // Verify independent optional modes without conflating their feature sets.
    ["dev", "dynamic_linking", "trace_chrome", "trace_tracy"]
        .into_iter()
        .try_for_each(|feature| {
            run(
                "cargo",
                &["check", "--workspace", "--locked", "--features", feature],
            )
        })?;
    test_dimensions()
}

/// Executes dimension-specific behavior that combined feature builds cannot cover.
fn test_dimensions() -> io::Result<()> {
    // Test each camera and physics selection through the owning game crate.
    [
        "dim2,mcp",
        "dim3,mcp",
        "dim3,networked,mcp",
        "dim2,dim3,networked,mcp",
    ]
    .into_iter()
    .try_for_each(|features| {
        run(
            "cargo",
            &[
                "test",
                "-p",
                "game",
                "--locked",
                "--no-default-features",
                "--features",
                features,
            ],
        )
    })
}

/// Generates every coverage format from one measured test execution.
fn coverage() -> io::Result<()> {
    // Remove stale profiles before collecting this test execution.
    fs::create_dir_all("target/llvm-cov")?;
    run("cargo", &["llvm-cov", "clean", "--workspace"])?;
    run(
        "cargo",
        &[
            "llvm-cov",
            "--workspace",
            "--locked",
            "--remap-path-prefix",
            "--no-report",
        ],
    )?;
    // Render all formats from the same collected profile data.
    for (format, destination) in [
        ("--html", "target/llvm-cov"),
        ("--lcov", "target/llvm-cov/lcov.info"),
        ("--json", "target/llvm-cov/coverage.json"),
    ] {
        let destination_flag = if format == "--html" {
            "--output-dir"
        } else {
            "--output-path"
        };
        run(
            "cargo",
            &[
                "llvm-cov",
                "report",
                format,
                destination_flag,
                destination,
                "--ignore-filename-regex",
                "(^|/)(tests|benches)/",
                "--remap-path-prefix",
            ],
        )?;
    }
    run(
        "cargo",
        &[
            "llvm-cov",
            "report",
            "--fail-under-lines",
            "50",
            "--ignore-filename-regex",
            "(^|/)(tests|benches)/",
            "--show-missing-lines",
            "--remap-path-prefix",
        ],
    )
}

/// Runs gates in failure-first order and stops at the first failed gate.
fn check() -> io::Result<()> {
    // Complete prerequisite setup before source and runtime validation.
    let gates: [fn() -> io::Result<()>; 6] = [
        setup,
        crate::imports::check,
        check_cargo,
        features,
        check_documents,
        coverage,
    ];
    // Stop after the first failure so later gates cannot obscure its cause.
    gates.into_iter().try_for_each(|gate| gate())
}

/// Runs exact Cargo format, lint, and test scopes before optional features.
fn check_cargo() -> io::Result<()> {
    let commands: &[&[&str]] = &[
        &["fmt", "--all", "--", "--check"],
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
        &["test", "--workspace", "--locked"],
    ];
    commands
        .iter()
        .try_for_each(|arguments| run("cargo", arguments))
}

/// Builds documentation and benchmarks only after behavioral gates pass.
fn check_documents() -> io::Result<()> {
    let commands: &[(&str, &[&str])] = &[
        (
            "cargo",
            &[
                "doc",
                "--workspace",
                "--no-deps",
                "--document-private-items",
                "--locked",
            ],
        ),
        ("cargo", &["bench", "--workspace", "--locked", "--no-run"]),
        ("mdbook", &["build", "docs"]),
    ];
    commands
        .iter()
        .try_for_each(|(program, arguments)| run(program, arguments))
}

/// Exercises distinct onboarding dimensions and optional integrations.
fn generate_matrix() -> io::Result<()> {
    // Keep generated projects outside authored source and never overwrite them.
    fs::create_dir_all("target/generated")?;
    // Share dependency artifacts while validating each onboarding choice in order.
    [
        ("local-2d", "2d", "false", "false", "false", "false"),
        ("network-2d", "2d", "true", "false", "true", "false"),
        ("local-3d", "3d", "false", "true", "true", "false"),
        ("network-3d", "3d", "true", "false", "false", "false"),
        ("mixed", "both", "true", "true", "true", "false"),
        ("web-2d", "2d", "false", "false", "false", "true"),
        ("web-3d", "3d", "true", "true", "false", "true"),
    ]
    .into_iter()
    .try_for_each(generate_case)
}

/// A fixed generator CLI case: name, dimension, networking, assets, MCP, and
/// web.
type GenerationCase = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

/// Generates a fresh project and verifies the selected output paths.
fn generate_case(case: GenerationCase) -> io::Result<()> {
    let project = Path::new("target/generated").join(case.0);
    // Existing projects may contain user work and must remain untouched.
    ensure_project_absent(&project)?;
    // The generator must preserve its parent workspace before child validation.
    generate_preserving_parent(case)?;
    verify_generated_project(&project, case.5 == "true")
}

/// Rejects existing output before cargo-generate can modify its contents.
fn ensure_project_absent(project: &Path) -> io::Result<()> {
    // Preserve an existing generated project instead of overwriting user work.
    if project.exists() {
        // Name the protected path so the caller can select another destination.
        let display = project.display();
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{display} already exists; select a fresh generation directory"),
        ))
    } else {
        Ok(())
    }
}

/// Checks that generation does not add a member to its parent workspace.
fn generate_preserving_parent(case: GenerationCase) -> io::Result<()> {
    // Compare exact bytes because Cargo generation can silently edit workspace membership.
    let before = fs::read("Cargo.toml")?;
    run_generator(case)?;
    let after = fs::read("Cargo.toml")?;
    let is_preserved = before == after;
    is_preserved
        .then_some(())
        .ok_or_else(|| io::Error::other("generation changed the parent workspace"))
}

/// Formats fixed CLI definitions without sending them through a shell.
fn run_generator((name, dimension, networked, skein, mcp, web): GenerationCase) -> io::Result<()> {
    // Only the generator boundary receives textual capability choices.
    let definitions = [
        ("dimension", dimension),
        ("networked", networked),
        ("skein", skein),
        ("mcp", mcp),
        ("web", web),
    ]
    .map(|(key, value)| format!("{key}={value}"));
    // Disable automatic workspace edits and permit only the template's reviewed hook.
    let mut arguments = vec![
        "generate",
        "--path",
        ".",
        "--name",
        name,
        "--destination",
        "target/generated",
        "--vcs",
        "none",
        "--no-workspace",
        "--silent",
        "--allow-commands",
    ];
    arguments.extend(
        definitions
            .iter()
            .flat_map(|definition| ["--define", definition.as_str()]),
    );
    run("cargo", &arguments)
}

/// Checks setup, host compilation, and the selected browser output.
fn verify_generated_project(project: &Path, is_browser_build: bool) -> io::Result<()> {
    // Restore links after generation, which deliberately skips symbolic links.
    run_in(project, "cargo", &["xtask", "setup"])?;
    verify_instruction_link(project)?;
    // Compile the selected host integrations before attempting browser output.
    run_in(project, "cargo", &["check", "--workspace", "--locked"])?;
    if is_browser_build {
        // Normalize Trunk's inherited color setting before building browser output.
        run_in(project, "trunk", &["build"])?;
    }
    Ok(())
}

/// Requires the generated instructions to refer to the local canonical file.
fn verify_instruction_link(project: &Path) -> io::Result<()> {
    // Verify relative link contents because an absolute source path would break copies.
    let instruction_link = fs::read_link(project.join("CLAUDE.md"))?;
    let is_relative_link = instruction_link == Path::new("AGENTS.md");
    if !is_relative_link {
        return Err(io::Error::other(
            "generated Claude instructions must link to AGENTS.md",
        ));
    }
    Ok(())
}

/// Exercises private command failure context independently of CLI parsing.
#[cfg(test)]
mod tests {
    use super::child_command;
    use super::run;
    use std::io;

    /// Changes only Trunk's child environment and preserves ordinary tool inheritance.
    #[test]
    fn trunk_receives_boolean_color_setting() {
        let trunk = child_command("trunk");
        let overrides: Vec<_> = trunk.get_envs().collect();
        assert_eq!(overrides, [("NO_COLOR".as_ref(), Some("true".as_ref()))]);
        assert_eq!(child_command("cargo").get_envs().count(), 0);
    }

    /// Keeps the operating-system cause and the attempted program in failures.
    #[test]
    fn missing_program_reports_operation_context() {
        let error = run("template-bevy-nonexistent-test-program", &[])
            .expect_err("a missing program must fail");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(
            error
                .to_string()
                .contains("template-bevy-nonexistent-test-program")
        );
    }
    /// Preserves a failed child status and names the attempted command.
    #[cfg(unix)]
    #[test]
    fn child_failure_reports_status() {
        let error = run("false", &[]).expect_err("false must fail");
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(error.to_string().contains("false"));
        assert!(error.to_string().contains("failed with"));
    }
}
