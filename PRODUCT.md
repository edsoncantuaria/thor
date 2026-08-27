# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Solo developers running multiple coding agents (Claude Code, Codex, GitHub Copilot, Antigravity,
OpenCode, Mimo, Freebuff) across multiple repositories at the same time. Not a multi-user
collaboration surface — one operator, many concurrent agent sessions.

## Product Purpose

Thor is a local-first desktop workspace for orchestrating parallel AI coding agents. One agent in
one terminal is easy; five agents across three repositories is the real job, and that's where plain
terminal tabs stop working — sessions get lost, MCP servers drift out of sync between agents, and
nobody knows which agent is doing what, where. Thor keeps every agent alive in a real PTY inside a
persistent project layout, and manages the things agents share (CLIs, MCP servers, skills,
conversations) as shared infrastructure instead of per-tab silos.

## Positioning

Unlike a plain terminal multiplexer, Thor's panes don't just switch focus — every agent's PTY stays
alive and keeps its session/history when the UI is rearranged or hidden. And unlike a single-agent
tool, Thor treats CLI installs, MCP config, and skills as infrastructure managed once and shared
across every agent, not duplicated per tab. On top of that, a lead agent can delegate work to other
agent CLIs and models through configurable worker "buckets" (any CLI, any model, including local
Ollama models and OpenAI-compatible proxies) with automatic quota/rate-limit failover between
buckets — a coordination layer other terminal-tab tools don't have.

## Operating Context

Developers work locally against real git repositories, often across several worktrees per project,
running agent CLIs (`claude`, `codex`, `copilot`, `opencode`, `agy`, `mimo`, `freebuff`) that Thor
can discover, install, update, and uninstall. Agent worktree isolation, git worktree-based
conflict resolution, and MCP server management are everyday operations, not edge cases.

## Capabilities and Constraints

- Cross-platform desktop app (Windows, macOS, Linux), built with Tauri, Rust, React, and
  `xterm.js`; the UI itself is a webview (HTML/CSS/React), so it follows web design/craft
  conventions even though it ships as a native desktop shell.
- Local-first, not local-only: update checks and provider usage polling are on by default; other
  network features are optional or action-triggered (see `docs/PRIVACY.md`).
- Real PTYs via `portable-pty`, not simulated terminals — agent CLIs that do TTY detection, ANSI
  cursor control, or interactive prompts must work correctly.

## Brand Commitments

Thor's design system is a confirmed hard constraint: every color, spacing, and status value goes
through the CSS custom properties in `src/styles/theme.css` (`--bg`, `--fg`, `--accent`,
`--status-*`, `--agent-*`, etc.) — never a hardcoded color, never a gradient, nothing
"vibecoded" or generic-template-looking. This applies across every theme variant the app ships
(dark, light, Dracula, the Elite family, and others); new UI work extends the token system, it
does not introduce parallel styling.

## Evidence on Hand

Product screenshots exist per platform under `docs/screenshots/` and a workspace preview GIF at
`docs/assets/alethe-preview.gif` (filename is a pre-rebrand leftover; the asset itself is current).
No customer testimonials, case studies, or usage benchmarks exist — do not fabricate them.

## Product Principles

- Persistence over ephemerality: an agent's PTY and session history survive UI rearrangement; nothing silently dies because a pane got hidden or moved.
- Shared infrastructure over per-agent silos: CLI installs, MCP servers, and skills are configured once and used by every agent, not duplicated per tab.
- Coordination, not just multiplexing: a lead agent can delegate to configurable worker buckets across different CLIs and models, with automatic failover — this is a real orchestration layer, not a tab switcher.
- Strict, tokenized design system: every visual decision routes through `theme.css` tokens; no hardcoded colors, no gradients, no generic AI-template look.
- Local-first, transparently: the app is honest about which specific network calls exist by default (updates, usage polling) rather than claiming an offline-only guarantee it doesn't meet.
