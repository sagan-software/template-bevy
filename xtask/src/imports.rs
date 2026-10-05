//! Checks import shape through Rust's syntax tree on the stable toolchain.

use std::fs;
use std::io;
use std::path::Path;
use syn::parse::ParseStream;
use syn::parse::Parser;
use syn::visit::Visit;

/// Checks project Rust sources without inspecting generated build directories.
pub(crate) fn check() -> io::Result<()> {
    // Limit inspection to authored source roots, including private tests.
    ["crates", "xtask"]
        .into_iter()
        .map(Path::new)
        // An omitted source root contributes no files.
        .filter(|root| root.exists())
        .try_for_each(inspect_directory)
}

/// Inspects source files in a stable order and preserves parse and I/O
/// failures.
fn inspect_directory(path: &Path) -> io::Result<()> {
    // Sort one directory at a time so the first failure is reproducible.
    let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    // Stop at the first source or filesystem error.
    entries.into_iter().try_for_each(inspect_entry)
}

/// Inspects one source entry while excluding build directories and symlinks.
fn inspect_entry(entry: fs::DirEntry) -> io::Result<()> {
    let path = entry.path();
    let kind = entry.file_type()?;
    // Recurse through authored directories while excluding generated artifacts.
    let is_source_directory = kind.is_dir() && entry.file_name() != "target";
    if is_source_directory {
        return inspect_directory(&path);
    }
    // Non-Rust files and symlinks do not enter the Rust parser.
    let is_source_file =
        kind.is_file() && path.extension().is_some_and(|extension| extension == "rs");
    if is_source_file {
        return inspect_file(&path);
    }
    Ok(())
}

/// Names the failing file while retaining the parser's error category.
fn inspect_file(path: &Path) -> io::Result<()> {
    let source = fs::read_to_string(path)?;
    let display = path.display();
    inspect_source(&source)
        .map_err(|error| io::Error::new(error.kind(), format!("{display}: {error}")))
}

/// Rejects grouped imports while accepting flat paths, globs, and aliases.
fn inspect_source(source: &str) -> io::Result<()> {
    // Reject malformed Rust before applying the import policy.
    let syntax = syn::parse_file(source)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    // Visit every lexical scope, including imports inside functions.
    let mut visitor = ImportVisitor::default();
    visitor.visit_file(&syntax);
    if let Some(error) = visitor.parse_error {
        return Err(io::Error::new(io::ErrorKind::InvalidData, error));
    }
    if visitor.is_grouped {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "use one item per use statement",
        ))
    } else {
        Ok(())
    }
}

/// Records whether any import group exists in any lexical scope.
#[derive(Default)]
struct ImportVisitor {
    /// A grouped import requires conversion into separate statements.
    is_grouped: bool,
    /// Malformed conditional Rust must fail before the import policy is accepted.
    parse_error: Option<syn::Error>,
}

impl<'ast> Visit<'ast> for ImportVisitor {
    /// Visits import trees, including groups nested below a path.
    fn visit_use_group(&mut self, _group: &'ast syn::UseGroup) {
        self.is_grouped = true;
    }

    /// Inspects standard conditional branches instead of treating them as opaque tokens.
    fn visit_macro(&mut self, syntax: &'ast syn::Macro) {
        let is_conditional = syntax
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "cfg_select");
        if !is_conditional {
            return;
        }
        // Parse every branch, including branches inactive on this host.
        let parser = |input: ParseStream<'_>| self.inspect_conditional(input);
        // Retain malformed branch syntax for the enclosing source-path diagnostic.
        if let Err(error) = parser.parse2(syntax.tokens.clone()) {
            self.parse_error = Some(error);
        }
    }
}

impl ImportVisitor {
    /// Walks conditional Rust expressions while retaining syntax failures.
    fn inspect_conditional(&mut self, input: ParseStream<'_>) -> syn::Result<()> {
        // Predicates select compilation; import policy applies to every authored branch.
        while !input.is_empty() {
            let expression = parse_conditional_branch(input)?;
            self.visit_expr(&expression);
            // Rust accepts optional commas between conditional branches.
            if input.peek(syn::Token![,]) {
                input.parse::<syn::Token![,]>()?;
            }
        }
        Ok(())
    }
}

/// Parses one selection predicate and its complete Rust expression.
fn parse_conditional_branch(input: ParseStream<'_>) -> syn::Result<syn::Expr> {
    // The wildcard has no metadata; other predicates use Rust's attribute grammar.
    if input.peek(syn::Token![_]) {
        input.parse::<syn::Token![_]>()?;
    } else {
        input.parse::<syn::Meta>()?;
    }
    // Parse the expression only after its selection delimiter is present.
    input.parse::<syn::Token![=>]>()?;
    input.parse()
}

/// Covers valid Rust syntax, nested scopes, and malformed input.
#[cfg(test)]
mod tests {
    use super::inspect_source;
    use std::io::ErrorKind;

    /// Accepts individual imports without changing their aliases or glob
    /// semantics.
    #[test]
    fn flat_imports_are_valid() {
        for source in [
            "std::cfg_select! { unix => { use std::io; } _ => {} }",
            "cfg_select! { unix => 1, _ => 2 }",
            "cfg_select! { unix => { cfg_select! { windows => { use std::io; } _ => {} } } _ => {} }",
        ] {
            assert!(inspect_source(source).is_ok(), "{source}");
        }
        assert!(
            inspect_source("use std::io; use std::path::Path as FilePath; use bevy::prelude::*;")
                .is_ok()
        );
    }

    /// Finds root and nested groups, including imports inside functions.
    #[test]
    fn grouped_imports_are_rejected() {
        for source in [
            "use std::{io, fs};",
            "use std::path::{Path};",
            "fn f() { use std::{io, path::{Path}}; }",
            "std::cfg_select! { unix => { use std::{io, fs}; } _ => {} }",
            "cfg_select! { unix => {} _ => { use std::path::{Path}; } }",
            "cfg_select! { unix => { cfg_select! { windows => { use std::{io, fs}; } _ => {} } } _ => {} }",
        ] {
            assert_eq!(
                inspect_source(source)
                    .expect_err("group must be rejected")
                    .kind(),
                ErrorKind::InvalidData
            );
        }
    }

    /// Reports syntax errors instead of accepting sources it cannot inspect.
    #[test]
    fn malformed_source_is_rejected() {
        assert_eq!(
            inspect_source("cfg_select! { unix => { use std::io } _ => {} }")
                .expect_err("conditional Rust is malformed")
                .kind(),
            ErrorKind::InvalidData
        );
        assert_eq!(
            inspect_source("use std::{")
                .expect_err("source is malformed")
                .kind(),
            ErrorKind::InvalidData
        );
    }
}
