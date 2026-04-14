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
? 32 unstaged file(s). Stage all? [y/N] l=list d=diff
✓ Staged 30 tracked file(s) (untracked skipped)
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

## Feed (multi-repo)

One command, whole folder. Scan a directory (or a saved group) and get a status card for every git repo — dirty state, ahead/behind, last commit, last activity — sorted by most recent first.

```bash
repo feed ~/Developer/PROJECTS           # scan a directory
repo feed <alias>                        # run a saved group
repo feed <target> -i                    # interactive TUI (Feed + Dashboard tabs)
repo feed <target> --filter "status:dirty"
repo feed                                # no target → open group picker
```

**Static output:**
```
▣ ~/Developer/PROJECTS · 173 repos

   ● repo-cli            master         3m 9?          2d  feat(ai): add model selection...
   ● servel              master         2m 2?   14↑    19h  chore: bump src submodule...
   ● werewolf-game-app   main           clean          20h  feat: overhaul betting panel UX...
   ● aktar.io            master         clean          1mo feat: add OpenReplay tracking...
```

Dot color: green = clean, yellow = dirty, dim = stale.

### Filters

Compose filters space-separated. Bare terms become full-text.

| Prefix | Example | Matches |
|---|---|---|
| `msg:` | `msg:refactor` | commit message |
| `author:` | `author:keren` | commit author |
| `date:` | `date:2026-01-01..2026-03-01` | commit date range |
| `repo:` | `repo:cli` | repo name substring |
| `status:` | `status:dirty` | `dirty` / `clean` / `ahead` / `behind` / `stale` |
| `text:` | `text:jwt` | full-text across repo name + branch + commit messages |

```bash
repo feed projects --filter "status:dirty author:keren text:refactor"
```

### TUI (`-i`)

Two tabs: **Feed** (chronological cross-repo commit stream) and **Dashboard** (repo cards list). Live filter bar — type `/` to edit, applies on Enter.

**Keys:**
- `Tab` — switch Feed/Dashboard
- `/` — edit filter  ·  `x` — clear filter
- `j/k` — navigate  ·  `g/G` — top/bottom
- `q` — quit

## Groups

Save named sets of repos so you don't retype paths.

```bash
repo groups              # list saved groups
repo groups new          # interactive picker
repo groups new --root ~/Developer/PROJECTS
repo groups edit <alias>
repo groups rm <alias>
repo groups show <alias> # dump TOML
```

**Picker flow:**
1. Enter scan root (e.g. `~/Developer/PROJECTS`)
2. Fuzzy-search + multi-select repos (`Space` toggle, `^A` all, `^N` none)
3. Enter alias → saved

**Keys (picker):**
- `Space` — toggle repo
- `Tab` / `Shift+Tab` — select all / none (filtered)
- `Enter` — advance step  ·  `Esc` — go back
- Type to search

### Groups config

`~/.config/repo/groups.toml` — hybrid storage: scan root plus explicit pinned/unpinned overrides.

```toml
[[group]]
alias = "projects"
scan_root = "~/Developer/PROJECTS"
max_depth = 3
exclude = ["archive/*", "experiments/*"]
pinned = ["~/work/special-repo"]     # extra repos outside scan_root
unpinned = ["~/Developer/PROJECTS/legacy"]  # exclude specific scan results
```

Written automatically by `repo groups new` — edit by hand or re-run `repo groups edit <alias>`.

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
? 5 unstaged file(s). Stage all? [y/N] l=list d=diff s=select
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
