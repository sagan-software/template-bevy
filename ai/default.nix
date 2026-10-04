{
  pkgs,
  lib ? pkgs.lib,
  bevyBrpMcp,
  packageName ? "template-bevy",
}:
let
  ensureLinks = pkgs.writeShellApplication {
    name = "ensure-ai-links";
    runtimeInputs = [ pkgs.coreutils ];
    text = ''
      set -euo pipefail

      require_file() {
        if [ ! -e "$1" ]; then
          echo "missing required AI source: $1" >&2
          exit 1
        fi
      }

      link_if_safe() {
        target="$1"
        dest="$2"

        mkdir -p "$(dirname "$dest")"
        if [ -e "$dest" ] && [ ! -L "$dest" ]; then
          echo "skip $dest: exists and is not a symlink" >&2
          return 0
        fi

        ln -sfn "$target" "$dest"
      }

      require_file ai/AGENTS.md
      require_file ai/codex/config.toml
      require_file ai/mcp.json
      require_file ai/opencode.json
      require_file ai/skills

      mkdir -p .codex .claude .cursor/rules .opencode

      link_if_safe ai/AGENTS.md AGENTS.md
      link_if_safe ai/AGENTS.md CLAUDE.md
      link_if_safe ai/mcp.json .mcp.json
      link_if_safe ai/opencode.json opencode.json

      link_if_safe ../ai/AGENTS.md .codex/AGENTS.md
      link_if_safe ../ai/codex/config.toml .codex/config.toml
      link_if_safe ../ai/skills .codex/skills

      link_if_safe ../ai/AGENTS.md .claude/AGENTS.md
      link_if_safe ../ai/AGENTS.md .claude/CLAUDE.md
      link_if_safe ../ai/mcp.json .claude/mcp.json
      link_if_safe ../ai/skills .claude/skills

      link_if_safe ../ai/AGENTS.md .cursor/AGENTS.md
      link_if_safe ../../ai/AGENTS.md .cursor/rules/bevy-template.mdc
      link_if_safe ../ai/mcp.json .cursor/mcp.json
      link_if_safe ../ai/skills .cursor/skills

      link_if_safe ../ai/AGENTS.md .opencode/AGENTS.md
      link_if_safe ../ai/mcp.json .opencode/mcp.json
      link_if_safe ../ai/skills .opencode/skills
    '';
  };
in
{
  inherit ensureLinks;

  packages = [
    bevyBrpMcp
    ensureLinks
  ];

  shellHook = ''
    export BEVY_BRP_MCP="${lib.getExe bevyBrpMcp}"
    export BEVY_TEMPLATE_PACKAGE="${packageName}"
    ${lib.getExe ensureLinks}
  '';
}
