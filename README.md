# horae

This is the mouse interaction fork maintained at [mianmianlingqi/horae](https://github.com/mianmianlingqi/horae), based on [zhaohang1205/horae](https://github.com/zhaohang1205/horae). It retains the upstream GPL-3.0 license and history.

**Download this fork:** [v0.2.6-mouse.1 release](https://github.com/mianmianlingqi/horae/releases/tag/v0.2.6-mouse.1). For Windows x64, extract the ZIP and run `horae.exe` in Windows Terminal. Existing data under `%APPDATA%\horae` is reused. Upstream npm/crates.io installers below install the upstream version; use this fork's release or build its source for the mouse enhancements.

> **Idea → captured in under 10 seconds. Written in Rust.**  
> Sub-second startup · reactive completions as you type · one-line syntax · full GTD · no account

<p>
  <a href="https://github.com/mianmianlingqi/horae/actions/workflows/ci.yml"><img src="https://github.com/mianmianlingqi/horae/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/mianmianlingqi/horae/actions/workflows/release.yml"><img src="https://github.com/mianmianlingqi/horae/actions/workflows/release.yml/badge.svg" alt="Release"></a>
  <a href="https://crates.io/crates/horae"><img src="https://img.shields.io/crates/v/horae.svg" alt="Crates.io"></a>
  <a href="https://crates.io/crates/horae"><img src="https://img.shields.io/crates/d/horae.svg" alt="Downloads"></a>
  <a href="https://www.npmjs.com/package/horae-cli"><img src="https://img.shields.io/npm/v/horae-cli.svg" alt="npm"></a>
  <a href="https://www.npmjs.com/package/horae-cli"><img src="https://img.shields.io/npm/dm/horae-cli.svg" alt="npm downloads"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/MSRV-1.89-orange.svg" alt="MSRV 1.89"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-blue.svg" alt="License"></a>
  <a href="https://github.com/zhaohang1205/horae"><img src="https://img.shields.io/github/stars/zhaohang1205/horae?style=social" alt="GitHub Stars"></a>
</p>

## The capture experience

Most task managers make you pay a tax every time you capture: switch app, wait for it to load, find the input field, pick a project, set a date, close the dialog. **horae eliminates that tax entirely.**

```
$ horae -f
```

That's it. Flash mode opens **directly on the input line** — no splash screen, no list view, no navigation.
Start typing and the cursor is already blinking. Fill in the task, press Enter. Done. The window closes.

```
dentist appointment ~fri 14:00 @health !high ↵
✓ captured  (#a3f)                              [horae exits]
```

**Time from idea to captured: under 10 seconds.**

### Why so fast?

horae is a **Rust binary with ~2 ms of startup overhead** — the font probe reads font directories directly instead of shelling out, settings load in a single batched SQL query, and input-method detection runs on a background thread so it never blocks the cursor.

### Reactive completions, no Tab required

Type `@`, `~`, `*`, or `!` and a completion panel appears instantly — no Tab, no delay.

```
dentist appointment ~fr█
                    ┌──────────────────────────┐
                    │ ~fri    → this Friday     │  ← highlighted
                    │ ~fri2   → next Friday     │
                    │ ~friday → (same as ~fri)  │
                    └──────────────────────────┘
```

`Tab` accepts the top candidate. `Up`/`Down` (or Vim `j`/`k`) moves through the list.
`Enter` completes **and** submits in one keystroke — zero extra keypresses.

Two completion styles (toggle under `F7`):
- **Reference** — token, meaning and a syntax example shown side-by-side
- **Speed** — compact list with ghost text inlined directly in the input

### The full syntax, on one line

```
buy milk @home ~today 18:00 *2w[1,3] !high
│         │     │            │        └─ priority
│         │     │            └────────── recurrence: every 2 weeks, Mon+Wed
│         │     └─────────────────────── scheduled time  (start or deadline)
│         └───────────────────────────── tag
└─────────────────────────────────────── title
```

Tokens can be in any order. Full-width punctuation (`＠ ～ ＊ ！`) is accepted too — Chinese IME users never need to switch to half-width.

---

|  | **horae** | Taskwarrior | todo.txt |
|--|--|--|--|
| Startup overhead | **~2 ms** (Rust binary) | ~50–200 ms | ~0 ms (shell) |
| Input → captured | **< 10 s** (flash mode) | ~30 s | ~15 s |
| Reactive completions | **✅ auto-popup** | ❌ | ❌ |
| TUI | ✅ ratatui, keyboard-driven | ⚠️ separate project | ❌ |
| Recurring habits | ✅ RRULE engine built-in | ✅ | ❌ |
| Pomodoro | ✅ daemon + waybar | ❌ | ❌ |
| Phone push | ✅ ntfy / Syncthing | ❌ | ❌ |
| Install without Rust | ✅ `npx horae-cli` | ⚠️ package manager | ✅ |
| Storage | SQLite (single file) | proprietary text | plain text |
| Offline / no account | ✅ | ✅ | ✅ |

A GTD task manager built in Rust: one binary with an embedded SQLite database, a CLI, and a [ratatui](https://ratatui.rs) TUI. No server, no account, no telemetry — everything lives in `~/.config/horae/`.

Nothing about a task is ever lost: every status change, reschedule and check-in is stamped with a UTC-millisecond timestamp and appended to an append-only `task_events` timeline. `horae show <id>` replays the full history.

中文手册：[README_CN.md](README_CN.md)


## Contents

- [Install](#install)
- [Quick start](#quick-start)
- [Capture](#capture)
- [Concepts](#concepts)
- [Quick-add syntax](#quick-add-syntax)
- [CLI reference](#cli-reference)
- [TUI](#tui)
- [Focus recommendation](#focus-recommendation)
- [Pomodoro](#pomodoro)
- [Calendar](#calendar)
- [Quotes](#quotes)
- [Profiles](#profiles)
- [Phone capture and notifications](#phone-capture-and-notifications)
- [Backup](#backup)
- [Where data lives](#where-data-lives)
- [Contributing](#contributing)
- [License](#license)

## Install

**npm** — prebuilt binaries, no Rust toolchain required:

```sh
npx horae-cli              # run without installing
npm install -g horae-cli   # install globally
```

**Cargo** — requires Rust 1.89+; SQLite is bundled, no C dependencies:

```sh
cargo install horae
cargo install --git https://github.com/zhaohang1205/horae   # latest main
```

Inside China the Gitee mirror is faster: `cargo install --git https://gitee.com/zhao-hang1205/hora.git`.

**From source:**

```sh
git clone https://github.com/zhaohang1205/horae.git
cd horae
cargo build --release      # binary: target/release/horae
```

Two things worth knowing before the first run:

- Icons are Nerd Font glyphs. On startup `horae` probes installed fonts and falls back to plain ASCII if none is found, so you never see tofu boxes. Force a style with `HORAE_ICONS=nerd|ascii`.
- Desktop notifications use native D-Bus notifications (`org.freedesktop.Notifications`) and are Linux-only (avoiding command-line argument leakage). On Windows the TUI alarms, the pomodoro timer and ntfy pushes still work; data is stored under `%APPDATA%\horae\`.

## Quick start

```sh
horae capture "buy milk @home ~today 18:00 !medium"
horae list --status next
horae do --start           # recommend one task, then start a pomodoro
horae done "buy milk"
horae show <id>            # details + full event timeline
horae                      # open the TUI
```

Commands that take a task accept a **full id**, a **unique id prefix** (git-style), or a **unique exact title**; an ambiguous title is an error rather than a guess. Archived tasks can only be addressed by id.

## Capture

Getting a thought out of your head is the one interaction you repeat hundreds of times a day, so it is the path `horae` optimizes hardest. Four ways in, all accepting the same one-line syntax:

| Situation | Command | Notes |
| --- | --- | --- |
| Already in a shell | `horae capture "buy milk @home ~today 18:00"` (`c`) | quotes are optional: `horae c buy milk @home` works |
| Want a box, not a terminal argument | `horae -f` / `horae flash` (`f`) | opens straight on the input line; Enter saves and exits, Esc drops into the list instead |
| Already in the TUI | `a` | capture without leaving the current view |
| Text is on the clipboard | `horae capture --clip` | short text becomes the title; long text keeps its first ~30 characters as the title and stores the rest in notes |

Pasting into the TUI works the same way: paste in any list view and it opens the input line with that text already in it.

### Speed

Everything between "typed the command" and "cursor is blinking" is kept out of the way:

- Startup work before the first frame costs about 2 ms — the font probe reads font directories directly instead of shelling out to `fc-list`, the input-method switch runs on a background thread, and settings load in one batched query. The icon choice is cached in the settings table on first run.
- Flash mode skips the splash screen and lands directly on the input line.
- Invalid input never reaches the database. A time expression that does not parse, or a recurrence rule the engine cannot expand, is reported inline and nothing is written.
- Nothing waits on the network. `capture` is a single local transaction.

### Typing

- Full-width punctuation (`＠ ～ ＊ ！ ［ ］ ：`) is normalized, so a Chinese IME stays usable without switching to half-width. Where possible the input line is switched to English input automatically, asynchronously.
- Completion pops up by itself as soon as you type `@`, `~`, `*` or `!` — no Tab required. `Tab` accepts the highlighted candidate, `Up`/`Down` (or `Ctrl+n`/`Ctrl+p`, `Ctrl+j`/`Ctrl+k`, `Alt+n`/`Alt+p`, `PageUp`/`PageDown`) move through the list, and `Esc`, `Ctrl+E` or `Ctrl+G` dismisses it. `Enter` completes the highlighted candidate and then submits.
- Two completion styles are available under `F7`: a reference card with the token, its meaning and a syntax example, or a compact speed list with ghost text inline in the input line.
- Full Emacs-style line editing: `Ctrl+a`/`Ctrl+e` (start/end), `Ctrl+b`/`Ctrl+f`, `Ctrl+w` (delete word back), `Ctrl+u`/`Ctrl+k`, `Ctrl+h`/`Ctrl+d`, plus `Home`/`End` and the arrow keys.
- Zen capture hides the surrounding chrome while you type. It is on by default.

### Corrections

Right after a capture, `u` undoes it — the toast tells you so. The same key undoes the last checklist tick, archive or restore, so capturing carelessly costs nothing.

### Capture settings

Toggles under `F7` in the TUI, persisted per database:

| Setting | Default | Effect |
| --- | --- | --- |
| Start in capture | off | launching `horae` lands on the input line instead of a list — this is what `horae -f` does for one run |
| Flash mode | off | Enter saves and exits instead of returning to the list |
| Zen capture | on | hide list chrome while typing |
| Completion style | Reference | card vs. compact list with ghost text |
| Password mode | off | require password authentication to open app or view data; press L in TUI to lock screen |

## Concepts

### Status lifecycle

```text
Inbox → Next / Scheduled / Waiting / Someday / Reference → Done
```

A freshly captured task lands in `Inbox`. Clarifying it moves it to one of the actionable or non-actionable states, and each transition stamps the matching `*_at` column. Because every transition, reschedule and check-in carries its own UTC-ms timestamp, the timeline doubles as a time record of the task's life: you can see how long it sat unclarified in the inbox, when it was rescheduled, and when it was finally done.

### Archive vs. purge

- **Archive** (`horae archive`, aliases `rm` / `delete`) is a soft delete: it sets `archived_at` and an archive reason, keeps the task and its timeline, and is undone with `horae restore`.
- **Purge** (`horae purge`) is a hard delete and only works on already-archived tasks. It removes the task together with its events, tags and child rows, and logs nothing — there is no undo.

### Effective due

Sorting, filtering, alarms and the daily summary all use the *effective* due, not the raw `due_at` column. For a recurring task that is the most recently missed occurrence if one is outstanding, otherwise the next one. For everything else it is `due_at`, falling back to `scheduled_start_at`.

## Quick-add syntax

The same one-line parser runs everywhere text is accepted: CLI `capture` and `modify`, the TUI input bar, the phone sync files, and Feishu messages.

```text
buy milk @home ~today 18:00 *2w[1,3] !high
```

- `buy milk` — title
- `@home` — tag
- `~today 18:00` — scheduled start
- `*2w[1,3]` — recurrence rule
- `!high` — priority

Tokens can appear in any order.

Full-width punctuation (`＠ ～ ＊ ！ ［ ］ ：`) is accepted too, so a Chinese IME does not have to switch to half-width.

### Time (`~`)

`~time` sets the planned start (`scheduled_start_at`) and moves the task into `Scheduled`. `--due` sets a deadline (`due_at`) instead. A date without a clock time defaults to 09:00.

| Syntax | Examples | Meaning |
| --- | --- | --- |
| Offset | `now`, `+30m`, `+2h`, `+1d`, `+1w` | offset from now; units are `m` `h` `d` `w` |
| Offset + clock | `+3d 15:30` | offset by days, then anchor to a clock time |
| Day words | `today`, `tomorrow`, `今天`, `明天`, `后天` | that calendar day |
| Weekdays | `mon`–`sun`, `next fri`, `周一`–`周日`, `下周一` | nearest matching weekday; `next` adds a week |
| Clock | `18:00` | today, or tomorrow if that time already passed |
| Dates | `8/20`, `2026.8.20`, `2026-07-24 14:30` | absolute date, optional time |

### Recurrence (`*`)

| Shorthand | Expands to |
| --- | --- |
| `*d` `*w` `*m` `*y` | `FREQ=DAILY` / `WEEKLY` / `MONTHLY` / `YEARLY` |
| `*2d` `*3w` | same with `INTERVAL=2` / `3` |
| `*weekday` / `*weekend` | `FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR` / `SA,SU` |
| `*2w[1,3]` | every two weeks on Monday and Wednesday (1–7 = Mon–Sun) |
| `*1w[mo,we]` | weekly on Monday and Wednesday, by name |
| `*m[1,-1]` `*m[1,15]` | monthly on given days; negative counts from month end |
| `*y[jan,jul]` | yearly in January and July |
| `*2y[6]` | every two years in June |

Only `DAILY`, `WEEKLY`, `MONTHLY` and `YEARLY` are supported, and every write path validates the rule before it reaches the database — an unsupported frequency is an error, never a silent downgrade. Completing a recurring task does not finish it: a check-in is recorded and the anchor advances to the next occurrence. A habit can only be checked in once per day.

### Priority (`!`)

`!high` (also `!h`, `!1`, `!高`), `!medium` (`!m`, `!2`, `!中`), `!low` (`!l`, `!3`, `!低`). Priority is a dedicated `tasks.priority` column, not a tag; clear it with `--clear-priority`.

## CLI reference

| Command | Alias | What it does |
| --- | --- | --- |
| `horae` | | open the TUI |
| `horae -f` / `horae flash` | `f` | flash capture: type one line, Enter, exit |
| `horae tui` | | explicit TUI launch (`--flash` / `--normal`) |
| `horae capture [text]` | `c` | add to inbox; `--clip`, `--tag`, `--due`, `--status`, `--high/--medium/--low`, `--notes`, `--json` |
| `horae list` | `l` | query tasks: `--status`, `--tag`, `--date 0829`, `--due-before`, `--json` |
| `horae show <id>` | `s` | details plus the full event timeline |
| `horae modify <id> [text]` | `m`, `mod`, `edit` | update anything: `--title`, `--tag`, `--untag`, `--due`, `--clear-due`, `--start`, `--end`, `--rrule`, `--clear-schedule`, `--status`, `--notes`, `--edit-notes` |
| `horae next <id>` | | mark as next action |
| `horae wait <id>` | | move to waiting-for |
| `horae someday <id>` | | move to someday/maybe |
| `horae done <id>` | `d` | complete (recurring tasks reschedule) |
| `horae schedule <id>` | | set a planned window: `--start`, `--end`, `--rrule` |
| `horae archive <id>` | `rm`, `delete` | soft delete |
| `horae restore <id>` | | undo archive |
| `horae purge <id>` | | permanently delete an archived task |
| `horae tag <id> <name>` / `untag` | | attach or detach a tag |
| `horae tags` | | list all tags |
| `horae do` / `horae focus` | `do` | recommend the one task to do now (`--start`) |
| `horae log [message]` | | timestamped note that is not a task; bare `log` lists recent ones |
| `horae review` | | weekly review walkthrough |
| `horae stats` | | terminal dashboard |
| `horae calendar [date]` | `cal` | lunar date, solar term, holiday countdown; `--short`, `--json` |
| `horae pomo <start\|stop\|daemon\|waybar>` | `p` | pomodoro timer and status bar output |
| `horae alarm <waybar\|next> [slot]` | | upcoming-task reminders |
| `horae watch` | | phone bridge daemon: `--dir`, `--interval`, `--once` |
| `horae ntfy test` | | send a sample push to your phone |
| `horae feishu <test\|due\|summary\|listen\|doctor\|sync>` | | Feishu / Lark integration |
| `horae export` | | full backup to JSON (`--file`) |
| `horae import <file>` | | restore a backup (`--replace` wipes first) |
| `horae profile <list\|new\|rename\|rm\|set-default>` | | manage data sets |
| `horae completions <shell>` | | generate shell completions |

Global flags apply everywhere: `--profile <name>` selects the data set, `--lang zh` (or `HORAE_LANG=zh`) prints help in Chinese, and `--theme <name>` (or `HORAE_THEME=<name>`) sets the UI theme (supports all 23 official Omarchy themes including `catppuccin`, `latte`, `tokyo-night`, `gruvbox`, `retro-82`, `nord`, `kanagawa`, `everforest`, `rose-pine`, `hackerman`, etc., or `auto` to follow system).

## TUI

Running `horae` opens a three-pane layout: navigation on the left, the task list in the middle, details and the event timeline on the right.

| Keys | Action |
| --- | --- |
| `h` `j` `k` `l` / arrows | move between panes and through the list |
| `0`–`9` | switch view: `1` Inbox, `2` Next, `3` Scheduled, `4` Waiting, `5` Someday, `6` Reference, `7` Done, `8` Archive, `9` Tags, `0` Quotes |
| `J` / `K` | jump to Today / Tomorrow |
| `g` / `G` | jump to first / last row |
| `a` | capture (quick-add syntax works here) |
| `Enter` / `e` | organize / edit the selected task |
| `n` | edit notes in `$EDITOR` |
| `C` | add a checklist item |
| `Tab` | manage checklist items (`j`/`k` move, `Space` toggles, `d` deletes, `J`/`K` reorder, `e` renames) |
| `=` | tick the next uncompleted checklist item |
| `x` / `w` / `s` | done / waiting / someday |
| `A` | archive (`y` to confirm); `u` restores; `D` purges inside the Archive view |
| `v` | visual multi-select; `Space` toggles a row, `Ctrl+a` selects all, `Ctrl+u` inverts, `T` bulk-tags |
| `P` / `S` | start or continue a pomodoro / stop it |
| `[` | configure pomodoro intervals (`25;5;15;4`) |
| `r` / `R` | weekly review; `R` advances to the next step |
| `"` | add to / remove from Quotes (when the feature is enabled) |
| `/` / `f` | search text / filter by tag |
| `F1` or `?` | keybinding help |
| `F5` / `F6` / `F7` | theme / language / feature toggles |
| `M` | settings view (profile management) |
| `W` | GTD workflow view |
| `Ctrl+P` | syntax cheat sheet |
| `Esc` / `q` | cancel / quit |

In the Tags view, `c` creates a tag and `D` deletes one. In the Settings view, `n` creates a profile, `r` renames, `d` deletes, `s` sets the default, `b` toggles sound effects, and `p` configures password protection.

Today and Tomorrow are deliberately strict: both only contain `Next` and `Scheduled` tasks. Today is the union of overdue tasks and tasks whose window falls inside today; Tomorrow contains only tasks scheduled inside tomorrow's window, with no carry-over of unfinished work.

### Mouse

Mouse support is on by default. Task actions and popup buttons reuse existing business logic, and all keyboard shortcuts remain available.

| Action | Effect |
| --- | --- |
| Left-click a task row | select it and load its detail |
| Left-click the leading status marker, such as `.`, `>`, `#`, or `x` | complete the task or restore a done task to Next; recurring tasks advance their schedule as before |
| Left-click a view in the left sidebar | switch to that view and focus the sidebar |
| Wheel over the sidebar, list, detail, help, syntax or workflow | scroll only the pane under the pointer |
| Wheel over the task list | move the selection by the scroll step |
| Left-click inside a popup | the popup owns the pointer; only its own rows and buttons react |

Popups are modal for the mouse: opening one drops every underlying hit target, so a stray click cannot reach a hidden task. Inside a popup each option row and each `Confirm` / `Cancel` (or `Space` / `Enter` / `Esc`) button is registered at its real rendered rectangle — Chinese double-width text included, using display columns rather than character counts. Clicking `Confirm` in a bulk-archive prompt is exactly `Enter`; clicking a row of the `F7` panel is exactly `Space` on that row. When the terminal is too short for the `F7` panel the list scrolls (wheel or `j`/`k`) and the title shows `position/total`.

Mouse settings live under `F7` and are persisted per database:

| Setting | Default | Effect |
| --- | --- | --- |
| Mouse interaction | on | `mouse_enabled=1`; set `0` to stop capturing the mouse and get the terminal's native text selection back |
| Scroll step | 3 | `mouse_scroll_step`, wheel rows per notch, clamped to 1–100; `←`/`→` (or `h`/`l`) and `Space` on the row cycle 1/2/3/5/10/20 |

Both are ordinary rows in the `settings` table, so an existing database keeps working: a missing `mouse_enabled` means on, a missing `mouse_scroll_step` means 3.

Manual verification:

1. Start `horae`, click a task row and check its highlight and details. Click the status marker to complete a normal task or advance a recurring task. Press `u` to verify existing undo behavior.
2. Click sidebar views, then wheel over the sidebar, list and details independently. The list wheel moves the selection. Resize and filter the list, then check task identity when clicking again.
3. Press `F7`, click an option and check that it changes while the popup stays open. Click `[Enter/Esc] close`. Use the wheel or `j`/`k` in a short window. Click outside a popup and verify that underlying tasks are unaffected.
4. Press `A` on a task, cancel in the confirmation overlay, then reopen it and confirm. Check both cancellation and archive behavior.
5. Turn off Mouse interaction in `F7` with `Space`, verify that clicks stop and native text selection returns, then re-enable it with the keyboard and check clicking again.
6. Change the scroll step in `F7` and restart to verify persistence. Press `a`, enter a task and click Confirm or Cancel. Check help scrolling and closing, then exit and verify terminal cursor and text selection restoration.

## Focus recommendation

`horae do` (alias `focus`) scores every active task and prints the single highest one; `--start` also launches a pomodoro for it.

It first drops `Someday`, `Reference`, `Done`, quotes, and any `Scheduled` or `Waiting` task that is neither due nor overdue. What remains is scored as:

| Factor | Points |
| --- | --- |
| Priority `high` / `medium` / `low` | +10000 / +5000 / +1000 |
| Overdue | +2000, plus +10 per day overdue (capped at +500) |
| Due today | +1000 |
| Status `Next` | +500 |

Ties are broken by `created_at`, oldest first.

## Pomodoro

```sh
horae pomo start <id>   # 25-minute session on a task; spawns a background daemon
horae pomo stop
horae pomo waybar       # JSON for a waybar module
```

The daemon ticks once a second, writes `~/.config/horae/pomo.json`, and sends a desktop notification when a phase ends. `horae stats` shows today's count.

## Calendar

`horae calendar` prints the lunar date, the current solar term and upcoming holiday countdowns from a built-in 1900–2100 table — no network access involved. `--short` gives a one-line summary suitable for a login banner, `--json` gives structured output. Major holidays are flagged in the TUI banner and desktop notifications three days and one day ahead.

## Quotes

An opt-in feature (off by default, toggled under `F7`) for keeping quotes and reference fragments. A quote is a `reference` task tagged `@quote`; it only ever appears in the Quotes view (`0`) and never in the inbox or action lists. Capturing anything with `@quote` routes it there.

## Profiles

A profile is an independent SQLite file, so work and personal data can be kept apart.

```sh
horae profile list
horae profile new work
horae profile set-default work
horae --profile work capture "buy milk"
```

The TUI settings view (`M`) does the same with `n` / `r` / `d` / `s`. Switching takes effect on the next launch. Configuration lives in `~/.config/horae/config.json`; the database file is never touched by profile commands.

## Phone capture and notifications

### Syncthing folder bridge

Sync `~/.config/horae/sync` with your phone and run `horae watch` on the computer (in systemd, tmux, or autostart). Every few seconds it reconciles the folder:

| File | Direction | Contents |
| --- | --- | --- |
| `capture.txt` | phone writes | one quick-add line per task |
| `actions.txt` | phone writes | `done <id>`, `set <id> status next`, `set <id> due <time>` |
| `today.md` | computer writes | snapshot of active and overdue tasks |
| `reminders/*.md` | computer writes | a file per due task, which itself triggers a sync notification on the phone |

It polls rather than watching inotify because Syncthing writes via atomic rename, and consumed lines get a `.done` receipt so a crash cannot replay them.

### ntfy

Add an `ntfy` block to a profile in `config.json`, subscribe to the topic in the ntfy app, then keep `horae watch` running. Timed tasks push a native notification `lead_minutes` before they are due. The token is read from the environment (`token_env`) and never written to disk.

```json
{
  "default_profile": "default",
  "profiles": {
    "default": {
      "db": "horae.db",
      "ntfy": {
        "url": "https://ntfy.sh",
        "topic": "horae-my-private-topic",
        "token_env": "HORAE_NTFY_TOKEN",
        "priority": 5,
        "lead_minutes": 10
      }
    }
  }
}
```

Check it with `horae ntfy test`.

### Feishu / Lark

Configure a `feishu` block and `horae watch` will push due-task cards and a daily briefing. `horae feishu listen` opens a WebSocket long connection so you can capture tasks by messaging the bot and act on cards from your phone, without a public IP. `horae feishu doctor` checks credentials and connectivity; `horae feishu sync` reconciles with Feishu Tasks v2.

## Backup

```sh
horae export --file ~/horae-backup.json   # tasks, events, tags, settings, pomodoro
horae import ~/horae-backup.json          # merge; existing ids are skipped
horae import --replace ~/horae-backup.json # wipe and restore exactly
```

## Where data lives

| Path | Contents |
| --- | --- |
| `~/.config/horae/horae.db` | default SQLite database |
| `~/.config/horae/config.json` | profiles and their integration config |
| `~/.config/horae/pomo.json` | pomodoro runtime state |
| `~/.config/horae/sync/` | phone bridge folder |

`HORAE_CONFIG_DIR` overrides the whole directory. On Windows the default is `%APPDATA%\horae\`.

## Contributing

```sh
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow and [ARCHITECTURE.md](ARCHITECTURE.md) for the layering rules, migration rules and domain vocabulary.

## License

GPL-3.0. See [LICENSE](LICENSE).
