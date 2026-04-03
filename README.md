# repo

A minimal git summary for your terminal.

```
📍 ON: main (origin)  ★42 ⑂12
   3 files changed, 1 untracked
   156 total commits • 5 branches • popular: main (142), dev (89), feature-ui (45)

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

## Craft

Surgical commit design via full-screen TUI. Reword, split, squash, fixup, reorder, drop, and edit commits interactively.

```bash
repo craft              # TUI with last 20 commits
repo craft --count 50   # show 50 commits
repo craft --last 5     # pre-select last 5
```

**TUI Modes:**
- **Commit list** — browse commits, assign actions
- **Reword** — inline message editing
- **Split** — assign hunks to groups, each becomes its own commit
- **Squash** — pick a target commit to squash into
- **Fixup** — squash keeping the older commit's message
- **Reorder** — move commits up/down with J/K
- **Drop** — mark commits for removal
- **Preview** — review full rebase plan before executing

**Keys (commit list):**
- `j/k` — navigate
- `Enter` — open action menu
- `D` — show diff for current commit
- `p` — preview plan
- `q/Esc` — quit

**Keys (action menu):**
- `r` — reword
- `s` — split
- `q` — squash
- `f` — fixup
- `d` — drop
- `m` — reorder
- `e` — edit (stop for manual editing)
- `x` — reset to pick

## Sync

Pull and push in one command.

```bash
repo sync             # pull then push
repo sync --rebase    # pull --rebase then push
```

## Ignore Files (.repoignore)

Keep files untracked without polluting `.gitignore`. Files matching these patterns are hidden from the commit workflow and never staged.

**Per-repo:** create `.repoignore` in the repo root:

```
# Scratch files
scratch.*
*.local
my-notes.md
debug/

# IDE workspace (personal, not shared via .gitignore)
*.code-workspace
```

**Global:** add to `~/.config/repo/config.toml`:

```toml
ignore_files = ["*.local", "TODO.personal"]
```

Patterns from both sources are merged. Uses standard glob syntax (`*`, `**`, `?`, `[...]`). Bare filenames without `/` match at any depth (e.g. `scratch.log` also matches `src/scratch.log`).

When files are hidden, the commit workflow shows:

```
  ⊘ 3 file(s) hidden by .repoignore
? 5 unstaged file(s). Stage all? [Y/n] l=list d=diff s=select
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
message_box_style = "box"   # commit message display style (see below)
ignore_files = ["*.local"]  # global never-stage patterns (see .repoignore)
```

### Message Box Styles

Controls how the commit message is displayed during interactive commit:

| Style | Value | Description |
|---|---|---|
| Rounded box | `"box"` | Rounded border card around the message |
| Double line | `"double_line"` | Double-line separators above and below |
| Title box | `"title_box"` | Titled header with single-line separators |
| Gutter | `"gutter"` | Colored left bar (blockquote style) |

## License

MIT
