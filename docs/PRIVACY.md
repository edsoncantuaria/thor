# Privacy and data flow

This document describes the behavior of the current Thor `main` branch. It is a technical data-flow
guide, not a policy for Anthropic, OpenAI, Google, GitHub, Discord, an MCP publisher, or any
other third party.

## Local-first, not local-only

Thor is **local-first, not local-only**. Workspace state is stored on the user's machine and Thor
does not require an Thor cloud account, but the app has automatic and user-triggered network
surfaces. Coding-agent CLIs and other tools started inside its terminals can also use the network
under their own terms and configuration.

There is no first-party Thor analytics uploader in the audited source. That does not make the app
network-free: update checks and provider usage polling are on by default, and other integrations
connect when enabled or used.

## Local data, retention, export, and deletion

Thor uses Tauri's platform-specific application local-data directory. It keeps a profile registry at
the root and isolates most state under `profiles/<profileId>/`. Typical data includes:

- `projects.json`: projects, groups, terminal and layout state, preferences, custom CLI paths, and
  recent items;
- `scrollback/*.bin`: terminal scrollback snapshots;
- `activity-stats.json`: local activity summaries;
- `spawn.log`: process-launch and suspension diagnostics, including command/launcher representations,
  a PATH preview, identifiers, and timing data;
- `github_sync.json`: integration credentials and sync metadata when that integration is connected;
- `handoffs/<handoffId>/context.md`: a temporary cross-agent context packet when a handoff is used;
- `mcp/registry-cache.json` and MCP backup files: registry results and backups made while managing
  agent configuration; and
- root-level `logs/`: crash and frontend errors, resource snapshots, lifecycle messages, and
  `telemetry.jsonl` shared by the application installation.

Thor also reads or modifies data outside its own directory when a feature requires it: repositories,
agent session histories, agent configuration and credential stores, MCP/skill directories, user-chosen
todo or export locations, and operating-system credential stores. The embedded browser uses a private
webview, but the operating-system webview and sites may still maintain process-level caches or other
state outside Thor's profile directory.

### Retention

Most profile data remains until it is overwritten, the profile is deleted, or app data is reset.
There is no general time-based retention policy. Crash and frontend errors keep at most 20 files per
prefix, and the newest 500 event traces are retained in memory. Profile `spawn.log`, root
`resource.log`, `app-events.log`, and `telemetry.jsonl` are append-only and currently have no rotation
or size cap. Scrollback and logs may contain commands, output, paths, timing/resource facts, error
messages, or payloads supplied by app features.

### Export and deletion controls

- **Backup export/import** writes a ZIP to a location the user chooses. A profile export includes the
  profile tree, including persisted integration secrets and handoff/MCP artifacts, except `.log` and
  `.tmp` files and embedded-webview runtime data. Profile `spawn.log` is therefore excluded. Treat
  exported backups as sensitive and delete old copies yourself.
- **Log export** writes the files in the root `logs/` directory to a user-chosen ZIP. Review and redact
  it before sharing. It does not include profile `spawn.log`.
- **Delete profile** removes that profile directory; Thor prevents deletion of the last profile.
- **Reset/factory reset** removes active-profile data or, for the full reset, the app local-data root.
  The reset is best effort while files are open, and the app should be relaunched afterward.
- **GitHub Sync logout** clears its stored token but leaves non-secret sync metadata in
  `github_sync.json`.

Deleting Thor data does not delete copies already exported or synced, repositories, coding-agent
histories/configuration, operating-system credentials, remote GitHub Gists, or data held by external
services. Remove those at their source as well.

## Secrets and credentials

Thor does not currently move all integration secrets into an operating-system keyring:

- A GitHub personal access token is a JSON field in `github_sync.json`.
- Profile backups can contain those files and values.
- MCP credentials remain in the agent configuration files where the user adds them. Thor masks
  environment values in normal MCP views and reveals one value only on explicit request, but config
  files and Thor-created MCP backups can still contain plaintext secrets.
- Claude usage polling reads a token from `CLAUDE_OAUTH_TOKEN`, Claude Code's credentials file, or the
  OS keyring. Antigravity usage polling reads the `agy` credential from the OS keyring. Thor uses
  these values for the request and does not intentionally copy them into Thor profile persistence.
- Codex usage is requested through a short-lived `codex app-server` subprocess, so Codex remains
  responsible for its own authentication storage.

File protection therefore depends on OS account permissions and the security of any destination to
which a backup is copied. Do not place profile data, exports, logs, or agent configs in a public or
untrusted location.

## Local telemetry and diagnostics

The backend subscribes to Thor's internal event bus on every launch. Each event, including its
arbitrary JSON `data` payload, is appended to `logs/telemetry.jsonl`; counts and selected numeric
fields are also kept in memory, with the newest 500 event traces retained in memory. The current
source contains no telemetry upload client or first-party analytics endpoint.

Crash, frontend, resource, app-lifecycle, and spawn diagnostics are also local. They can include stack
traces, process/resource facts, paths, messages, command/launcher representations, PATH previews,
identifiers, and terminal launch timings. Log export is a manual local operation; Thor does not
automatically send the archive. Inspect both the archive and any separately shared `spawn.log` before
attaching them to an issue or email.

## Network and process inventory

"Default" below means a clean profile on the current `main` branch. A feature being visible or enabled
does not necessarily mean it sends a request before the stated trigger.

| Surface | Exact current default | What happens and where data goes |
|---|---|---|
| Automatic update check | **Off** | This build does not configure an update feed (`plugins.updater.endpoints` is empty). Packaged updater artifacts are not downloaded on startup. This is separate from platform publisher signing: current Windows installers are not code-signed and macOS builds are not notarized. |
| Provider usage polling | **On** | While the title bar is mounted, Thor schedules Claude, Codex, and Antigravity usage reads shortly after startup and every five minutes; ticks are skipped while the window is unfocused/hidden. The three topbar indicators default visible. Claude sends its bearer credential to `https://api.anthropic.com/api/oauth/usage`; Antigravity sends its bearer credential to `https://daily-cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels`; Codex is queried through a short-lived `codex app-server` subprocess. The current visibility preferences hide indicators but do not gate the polling effects. |
| Discord Rich Presence | **On** | Every 30 seconds, Thor sends generic activity text (Thor and the current app view, not project names) to the local Discord desktop IPC client. Discord controls any onward network publication under the Discord account's settings and terms. It can be disabled in Preferences. |
| GitHub Sync | **Off / disconnected; manual** | Entering a token validates it against `https://api.github.com/user`. Explicit Push uploads `projects.json` and, if present, `activity-stats.json` to a secret GitHub Gist; explicit Pull downloads them. A secret Gist is access-controlled by GitHub, not end-to-end encrypted by Thor. Thor does not periodically push or pull. |
| MCP registry | **MCP feature on; search on demand** | Local agent configs are scanned without registry traffic. Searching the add flow queries `https://registry.modelcontextprotocol.io/v0/servers` and caches the first page of up to 20 search terms under the profile. Adding a result writes agent configuration; package runtimes or remote URLs connect later when an agent/MCP client runs them. |
| MCP health checks and server tools | **On demand** | A Check action launches the selected coding-agent CLI so that CLI can test configured servers. The generic health probe may start a user-configured server command and probe it on an ephemeral loopback port. Those subprocesses and MCP servers can contact their configured hosts. |
| Embedded web content | **Browser feature on; no page open** | A page is loaded only after the user adds a URL. The desktop build creates an incognito native webview with autofill off; JavaScript is **on by default** for each pane. The visited site and all resources it embeds receive ordinary network requests and can observe data normally exposed by the OS webview and network. Incognito mode is not anonymity and does not stop the site from collecting data. |
| LAN Remote Control | **Off** | When enabled, Thor binds plain HTTP and WebSocket listeners to the detected LAN interface, choosing ports in `9340–9360` (WebSocket starts at the next port range). Pairing is opened explicitly for two minutes and uses a token; clean-profile limits are one device, one-hour sessions, read-only access on, and shell input off. Terminal/workspace metadata and scrollback are then available to authenticated paired devices. Use only on a trusted LAN; the transport is not TLS-encrypted. |
| Agent-event hook listener | **On, loopback only** | At backend startup Thor listens on the first available address from `127.0.0.1:9123–9143`. Requests require an in-memory `X-Thor-Token`; Thor writes a temporary hook configuration only when an agent workflow requests it. Hook payloads can feed the local event bus and therefore local telemetry. |
| AI Memory bridge | **Off** | The feature default is disabled. Detection, when requested, checks the local `ai-memory` executable and `127.0.0.1:49374`; configured agent sessions run the third-party `ai-memory mcp` subprocess over stdio. Its own storage and network behavior belong to that tool. |
| Development server | **Development only** | `npm run dev` and `npm run app` start Vite with hot-module reload on a loopback development port. This server and development CSP are not part of packaged releases; do not expose the development server to an untrusted network. |
| Git remotes, installers, and external links | **On demand** | Clone, fetch/pull/push, agent install/update/uninstall, and opened links run only after a user action. Thor delegates to Git, package managers/vendor installers, or the system browser; those tools contact the user-selected remote or their package/vendor services and apply their own credential and telemetry rules. |

## Coding-agent CLIs and other subprocesses

Claude Code, Codex, Copilot, Antigravity, OpenCode, Mimo, Freebuff, shells, Git, package managers, MCP
servers, Graphify, and user-entered commands are separate processes. Thor supplies their command-line
arguments, selected working directory, PTY input/output, and an inherited or constructed environment.
They may read repositories and home-directory configuration, use credentials they own, retain their
own histories, and contact provider or tool endpoints. Thor's local-first design does not proxy,
block, audit, or make those subprocesses local-only. Review each tool's permissions, privacy terms,
configuration, and unrestricted/approval mode before launching it.

## Handoff artifacts and redaction limits

A Claude-to-Codex or Codex-to-Claude handoff reads the selected provider's local session history and
builds an editable Markdown context packet. It can include user and assistant messages, clipped tool
calls/output, working-directory and Git metadata, and the source session identifier. Before showing
the draft, Thor applies regular-expression redaction for several common token, credential, header,
and private-key patterns.

That redaction is best effort, not a guarantee. Unusual secrets, confidential prose, source code,
paths, identifiers, or credentials split across text can remain. Review and edit the draft before
starting the target agent. Materializing a handoff writes
`handoffs/<handoffId>/context.md`; Thor requests cleanup after the first target turn or when the pane
closes, but a crash or interrupted flow can leave the file behind until manual/profile deletion. The
target agent then reads the packet and may send its contents to that agent's provider under the
provider's terms.

## Embedded content, CSP, and privileged commands

Remote pages are placed in separate incognito native webviews, while the main Thor UI invokes the
Tauri backend. The production configuration currently sets `csp` to `null`. A Content Security Policy,
when configured, is **defense in depth**, not privileged-command containment and not an authorization
boundary for Tauri commands. Treat content from web pages, repositories, terminal output, handoff
packets, agent histories, and MCP metadata as untrusted regardless of CSP.

## Third-party services

External requests disclose ordinary connection metadata such as IP address, time, TLS and HTTP
metadata, plus the request-specific account credential or content described above. Third parties set
their own retention, logging, training, and deletion rules. Consult and configure the relevant
provider before enabling an integration or launching a tool.

For a suspected security problem, follow [`SECURITY.md`](../SECURITY.md). For a non-security bug or
documentation correction, use this repository's public issue tracker.
