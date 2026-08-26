<a id="readme-top"></a>

<br />
<div align="center">
  <img src="./src/assets/theme-icons/elite-gold.png" alt="Thor Logo" width="160">

  <h1 align="center">Thor</h1>

  <p align="center">
    <b>The multi-agent coding workspace.</b>
    <br />
    Run Claude Code, Codex, Copilot and your shells side by side — in one local-first desktop app.
  </p>

  <p align="center">
    <a href="./SECURITY.md">Security</a>
    ·
    <a href="./docs/PRIVACY.md">Privacy</a>
    ·
    <a href="#contributing">Contribute</a>
  </p>
</div>

> [!IMPORTANT]
> Thor is an early public release. The desktop app is free, open source, and local-first, not
> local-only: update checks and provider usage polling are on by default, while other network features
> are optional or action-triggered. Manual GitHub Gist Sync is already available; first-party hosted
> sync or cloud backup may be offered separately later. See the
> [privacy and data-flow guide](./docs/PRIVACY.md).

<div align="center">
  <img src="./docs/assets/alethe-preview.gif" alt="Thor multi-agent coding workspace preview" width="760">
</div>

## What Thor Is

One agent in one terminal is easy. Five agents across three repositories is the actual job — and
that is where terminal tabs stop working: sessions get lost, MCP servers drift out of sync between
agents, and nobody knows which agent is doing what, where.

**Thor is a desktop workspace built for that.** Every agent runs in a real PTY inside a persistent
project layout, keeps its own session and history, and stays alive when you rearrange the UI. On top
of that, Thor manages the things agents share: their CLIs, their MCP servers, their skills, and the
conversations you move between them.

Cross-platform (Windows, macOS, Linux), local-first, built with Tauri, Rust, React, and `xterm.js`.
“Local-first” describes workspace persistence, not an internet-free guarantee; see
[`docs/PRIVACY.md`](./docs/PRIVACY.md) for current network defaults, credentials, and retention.

## Supported Platforms

<table>
  <tr>
    <th width="33.33%">macOS</th>
    <th width="33.33%">Windows</th>
    <th width="33.33%">Linux</th>
  </tr>
  <tr>
    <td align="center">
      <img src="./docs/screenshots/alethe-macos.png" alt="Thor running on macOS" width="100%">
    </td>
    <td align="center">
      <img src="./docs/screenshots/alethe-windows.png" alt="Thor running on Windows" width="100%">
    </td>
    <td align="center">
      <img src="./docs/screenshots/alethe-linux.png" alt="Thor running on Linux" width="100%">
    </td>
  </tr>
  <tr>
    <td align="center">Available on macOS</td>
    <td align="center">Available on Windows</td>
    <td align="center">Available on Linux</td>
  </tr>
</table>

## Agents

| Agent | CLI | |
|---|---|---|
| **Claude Code** | `claude` | Session resume, usage cards, local history |
| **Codex** | `codex` | Session resume, usage cards |
| **GitHub Copilot CLI** | `copilot` | |
| **Antigravity** | `agy` | Usage cards |
| **OpenCode** | `opencode` | Session resume |
| **Mimo** | `mimo` | |
| **Freebuff** | `freebuff` | |
| **Shell** | pwsh / bash / zsh | The plain terminal, same pane model |

Missing CLIs can be installed, updated, and uninstalled from inside Thor — it probes the machine
for Node, npm, WinGet, Scoop, and Chocolatey and offers only the methods that actually work there,
preferring each vendor's official installer. Already-installed CLIs are discovered across PATH,
registry, npm/pnpm/Volta/fnm/nvm/Bun/Cargo/Scoop/Chocolatey, and can be pointed at a custom path.

## What It Does

**Run agents in parallel**

- Projects, groups, and subgroups organize repositories; each open project becomes a container with
  its own panes.
- One agent per pane, or several agents as sub-tabs inside the same pane — each with its own PTY,
  working directory, and session.
- Auto, spotlight, sidebar, and custom grid layouts, editable directly on the grid.
- Closing a container hides it; the process keeps running.

**Keep the context**

- Sessions of Claude Code, Codex, and OpenCode resume after a crash or a restart.
- **Recent chats** lists the conversations of a pane's working directory and reopens any of them.
- A Claude Code conversation can be **handed off to Codex** (and back) through a locally redacted
  context packet — no copy-pasting the thread by hand. Redaction is best effort, so review the packet
  before starting the target agent.
- Scrollback is persisted per PTY, so reattaching shows what happened before.

**Manage what the agents share**

- **MCP tab**: every MCP server configured on the machine, grouped by server and showing which agents
  have it — read from Claude Code, Codex, OpenCode, and Antigravity configs. Add, remove, copy a
  server from one agent to another, search the official registry, and ask each agent to verify it can
  really reach a server. Every write is backed up, re-parsed, and committed atomically.
- **Skills tab**: the skills installed for each agent, with links and shared stores resolved so a
  shared skill shows up once.
- **Graphify**: a code graph of the project, served to the agents as an MCP server.

**Stay in control**

- RAM readout in the title bar; disable a terminal or suspend a whole group to get memory back.
- Git panel per project — status, stage, commit, branches, diffs in a pane — plus worktrees for
  parallel tasks.
- Content panes beside the terminals: file explorer, Markdown, diffs, images, video, embedded browser.
- Todos per project, isolated profiles, local backup export/import, 14 UI and terminal themes,
  EN and pt-BR.
- **Remote Control**: an authenticated LAN web view, paired by QR code, to follow and answer agents
  from your phone. It is off by default and uses unencrypted HTTP/WebSocket transport on the LAN, so
  enable it only on a trusted network. Clean profiles are read-only; answering agents requires a
  separate input opt-in, and shell input has its own additional opt-in.
- Spotify Now Playing, using your own Spotify app credentials in **Preferences ▸ Spotify** with
  `http://127.0.0.1:8888/callback` as the redirect URI. Current releases store those credentials in
  local profile files; see the privacy guide before exporting or sharing profile data.

## Core Concepts

| | |
|---|---|
| **Group** | A collection of projects that opens, collapses, and suspends together. |
| **Project** | A saved working context: terminals, layout, color, local state. |
| **Container** | The visible frame of an opened project. Closing it does not kill anything. |
| **Pane** | A terminal view inside a container. |
| **Sub-tab** | A separate agent or shell session inside the same pane. |
| **PTY** | The real backend process, alive independently of the UI. |

## Product Philosophy

A focused core with optional capabilities, closer to Obsidian than to a maximalist IDE. Non-essential
features ship behind feature flags or opt-in settings, and a clean installation stays a first-class
experience. Coherence over volume.

## Install

Build from source (see below) or use a published installer for this repository when one is available.

> [!WARNING]
> Windows builds are **not code-signed yet**, so Defender may flag `Thor.exe` as
> `Trojan:Win32/Bearfoos.A!ml` and quarantine it. The `!ml` suffix denotes a machine-learning
> heuristic rather than a publisher signature, and terminal-multiplexer behavior such as spawning
> child processes and creating PTYs can produce false positives. Verify that the download came from
> this project; do not bypass a warning for an artifact from another source.

To recover it: **Windows Security → Virus & threat protection → Protection history → Actions →
Restore**, then add an exclusion for `%LOCALAPPDATA%\com.thor.app` (and `src-tauri/target` if you
build from source). Reports of incorrect detection go to
[Microsoft Security Intelligence](https://www.microsoft.com/wdsi/filesubmission). macOS builds are
not notarized yet either — right-click the app and choose **Open** to bypass Gatekeeper. Signing and
notarization are on the [roadmap](#roadmap).

## Run From Source

```sh
git clone <this-repository>
cd thor
npm install
npm run app
```

Requirements: Node.js 18+, Rust stable, Visual Studio Build Tools on Windows, Tauri system
dependencies on Linux:

```sh
sudo apt install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

```sh
npm run app          # desktop app with hot reload
npm run dev          # frontend only
npm run build        # typecheck + build frontend
npm run tauri build  # installers → src-tauri/target/release/bundle/
```

## Terminal Command

Install the `thor` command from **Settings ▸ Integrations ▸ Terminal command**:

```bash
thor                # opens the current folder as a project
thor ~/some/project # opens the given folder
```

If the folder is already a project, it is brought into the workspace instead of duplicated. If Thor
is already running, the existing window is focused. The command lands in `~/.local/bin/thor`
(macOS/Linux) or `%LOCALAPPDATA%\Thor\bin\thor.cmd` (Windows) — reinstall it after moving the app.

## Roadmap

- [x] Multi-agent workspace with projects, groups, containers, and sub-tabs.
- [x] Real PTYs with spawn, attach, resize, scrollback, and session resume.
- [x] Agent install/update/uninstall, MCP and skills management.
- [x] Releases for Windows, Linux, and macOS.
- [ ] Windows release signing and macOS notarization.
- [ ] Broader Linux/macOS validation on real machines.
- [ ] First-party hosted cloud sync/backup (manual GitHub Gist Sync is already available).

## Contributing

Contributions are welcome. Read [`CONTRIBUTING.md`](CONTRIBUTING.md) for setup, project layout, and
house rules. The easiest ways to help:

- Report a bug with clear reproduction steps, or request a feature with the workflow it improves.
- Improve docs, screenshots, and platform validation — Linux and macOS are the least tested.

For larger changes, open an issue first so the direction can be discussed.

## Built with Thor

Projects and products built with Thor as the workspace — agents running in parallel, shells alongside them, sessions resumed across days.

<!-- showcase:start -->

_Nothing here yet._ Built something with Thor? Add it to [`SHOWCASE.md`](SHOWCASE.md) — it's one line and a pull request, and you end up in the contributors list too.

<!-- showcase:end -->

See [`SHOWCASE.md`](SHOWCASE.md) for the full list and how to submit.

## License

The source code is distributed under **AGPL-3.0-or-later**. See [`LICENSE`](LICENSE) for details.

This project is a modified version of [Alethe](https://github.com/Kc1t/alethe-agents).

## Community

- Security reports: [`SECURITY.md`](SECURITY.md)
- Privacy and data flows: [`docs/PRIVACY.md`](docs/PRIVACY.md)
