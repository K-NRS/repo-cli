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

## Update

Check for updates and self-update.

```bash
repo update           # check and install latest
repo update --check   # check only, no install
```

## Options

```
-i, --interactive    TUI mode (j/k to navigate, tab to switch panels)
    --graph          show branch visualization
    --no-color       plain output
-n, --commits <N>    commit count (default: 5)
    --fetch          fetch remotes before summary
    --no-fetch       skip fetch (overrides config)
    --stashes        show stash details (count only by default)
```

## Config

`~/.config/repo/config.toml`:

```toml
default_ai = "claude"        # AI provider for commits (claude/codex/gemini)
show_github_stats = true     # show stars/forks in header
auto_fetch = false           # fetch remotes on every invocation
commit_style = "concise"     # default commit message style
```

## License

MIT
