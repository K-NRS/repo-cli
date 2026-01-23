# repo

A minimal git summary for your terminal.

```
📍 ON: main (origin)
   3 files changed, 1 untracked

RECENT
   ●   2h  feat(auth): add jwt validation  Alice
   ●   5h  setup auth routes  Bob
   ●   1d  init feature  Alice

REMOTE
   origin/main         1d  "fix: resolve merge conflict"  Alice
   origin/feature-ui   3d  "wip"  Bob

STASHES (2)
   0: WIP login form
   1: debug stuff
```

Recent commits are shown from all local and remote branches, sorted by time.

## Install

```bash
cargo install --git https://github.com/K-NRS/repo-cli
```

## Usage

```bash
repo              # summary
repo -i           # interactive TUI
repo --graph      # branch tree
repo -n 10        # last 10 commits
repo /path/to/repo
```

## Commit (AI-powered)

Generate commit messages using AI (claude/codex/gemini).

```bash
repo commit                  # auto-detect AI, interactive
repo commit --ai claude      # use specific provider
repo commit --no-interactive # commit directly, skip review
```

**Flow:**
```
? 32 unstaged file(s). Stage all? [Y/n] l=list d=diff
✓ Staged all changes
● Generating commit message with claude...

──────────────────────────────────────────────────
  feat(auth): add JWT validation middleware
──────────────────────────────────────────────────

? Commit? [y/N] e=edit r=regen d=diff
```

**Keys:**
- `y` - commit
- `n` - cancel
- `e` - open TUI editor
- `r` - regenerate (with style options: concise/longer/shorter/detailed/custom)
- `d` - view diff

**Config** (`~/.config/repo/config.toml`):
```toml
default_ai = "claude"
```

## Update

Check for updates and self-update.

```bash
repo update           # check and install latest
repo update --check   # check only, no install
```

## Release Standard

Scaffold auto-release GitHub Actions workflow with conventional commit versioning.

```bash
repo init                    # auto-detect project type
repo init --lang rust        # explicit type
repo init --force            # overwrite existing
repo init /path/to/project   # target different path
```

**Supported:** Rust, Go, Bun, pnpm, Next.js, Node.js, React Native, Xcode, Python, Generic

**Generated workflow:**
- Conventional commit parsing (feat → minor, fix → patch, breaking → major)
- Language-specific build matrix
- Automatic GitHub releases with changelog

## Options

```
-i, --interactive    TUI mode (j/k to navigate, tab to switch panels)
    --graph          show branch visualization
    --no-color       plain output
-n, --commits <N>    commit count (default: 5)
```

## License

MIT
