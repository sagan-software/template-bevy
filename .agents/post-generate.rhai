// Keep incompatible plugins out of generated Bevy 0.20 applications.
if variable::get("networked") { throw "Lightyear has no Bevy 0.20 release. Select networked=false."; }
if variable::get("skein") { throw "Skein has no Bevy 0.20 release. Select skein=false."; }
// Remove inspection configuration when its endpoint is omitted.
if !variable::get("mcp") {
    file::delete(".mcp.json");
    file::delete(".codex/config.toml");
    file::delete("opencode.json");
}
// Expanded files replace their runnable defaults after Liquid processing.
file::rename("template/Cargo.toml", "Cargo.toml");
file::rename("template/README.md", "README.md");
file::rename("template/app.toml", "crates/app/Cargo.toml");
file::rename("template/index.html", "index.html");
file::delete("template");
file::delete("Cargo.lock");
system::command("cargo", ["generate-lockfile"]);
file::delete("cargo-generate.toml");
file::delete(".agents/post-generate.rhai");
