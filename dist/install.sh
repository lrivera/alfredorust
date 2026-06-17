#!/usr/bin/env bash
#
# Installs the spcli binary and the Claude skill so you can drive the platform
# through Claude. Run it from inside the extracted package directory:
#
#   ./install.sh
#
# Optional: INSTALL_DIR overrides where the binary goes (default ~/.local/bin).

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
SKILLS_DIR="$HOME/.claude/skills/spcli"

bin_src="$HERE/spcli"
if [ ! -f "$bin_src" ]; then
  echo "error: spcli binary not found next to this script ($bin_src)" >&2
  exit 1
fi

# 1) Install the binary on the PATH
mkdir -p "$INSTALL_DIR"
cp "$bin_src" "$INSTALL_DIR/spcli"
chmod +x "$INSTALL_DIR/spcli"
# macOS: clear the Gatekeeper quarantine flag so it runs without prompts
xattr -d com.apple.quarantine "$INSTALL_DIR/spcli" 2>/dev/null || true

# 2) Install the Claude skill (global, available in every session)
mkdir -p "$SKILLS_DIR"
cp "$HERE/skills/spcli/SKILL.md" "$SKILLS_DIR/SKILL.md"
cp "$HERE/skills/spcli/reference.md" "$SKILLS_DIR/reference.md"

echo "✓ Installed spcli      → $INSTALL_DIR/spcli"
echo "✓ Installed the skill  → $SKILLS_DIR"
echo

# 3) PATH check
case ":$PATH:" in
  *":$INSTALL_DIR:"*) echo "✓ $INSTALL_DIR is already on your PATH." ;;
  *)
    echo "⚠ $INSTALL_DIR is not on your PATH. Add it, e.g.:"
    echo "    echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.zshrc && source ~/.zshrc"
    ;;
esac

echo
echo "Next: open Claude in any project and ask something like \"list my accounts\"."
echo "First run, Claude will ask for the login URL (https://app.alfredorivera.dev),"
echo "your email, and your TOTP secret."
