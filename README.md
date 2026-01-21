# repo

A minimal git summary for your terminal.

```
📍 ON: feature-auth (2↑ 1↓ origin)
   3 files changed, 1 untracked

RECENT
   ●   2h  add jwt validation
   ●   5h  setup auth routes
   ●   1d  init feature

REMOTE
   origin/main         1d  "fix: resolve merge conflict"
   origin/feature-ui   3d  "wip"

STASHES (2)
   0: WIP login form
   1: debug stuff
```

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

## Options

```
-i, --interactive    TUI mode (j/k to navigate, tab to switch panels)
    --graph          show branch visualization
    --no-color       plain output
-n, --commits <N>    commit count (default: 5)
```

## License

MIT
