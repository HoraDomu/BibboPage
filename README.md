<div align="center">
  <img src="assets/logo.svg" width="900" alt="Bibbo" />
</div>

<br/>

<div align="center">

A native desktop knowledge graph. Write nodes. Connections emerge.

**[horadomu.github.io/BibboPage](https://horadomu.github.io/BibboPage)** &nbsp;·&nbsp; Built by [HoraDomu](https://github.com/HoraDomu)

</div>

---

## What it is

Bibbo is a native desktop app that maps the way your brain actually works. No folders. No hierarchy. One graph. You write nodes, link them with `[[Title]]`, and your understanding takes shape on its own.

Every design decision — the physics simulation, the local graph view, the writing mode, Living Connections — exists to answer one question: *does this help you think more clearly?*

---

## Why this works

Your brain doesn't store knowledge in lists — it stores it in **connections**. When you link two ideas together you're not just filing them away, you're making a claim about *why* they relate. The more paths that lead to a concept, the deeper your understanding of it becomes.

Most note-taking tools fight this. Folders create silos. Lists hide relationships. Bullet points flatten what is inherently dimensional.

Bibbo forces you to answer *"what does this connect to?"* instead of *"where does this go?"* — and that difference compounds over time.

---

## Features

**Living Connections**
Click any edge and see exactly why two nodes are connected — which ideas reference them together, how recently, how strongly. Not just that they're linked. Why.

**Physics-based graph**
Nodes repel, spring toward their connections, and settle into organic arrangements. Drag one and the whole network shifts. The layout emerges from your thinking, not from a grid.

**Local graph navigation**
Click a node to enter a focused view of just it and its neighbors. Click a neighbor to expand. Navigate your knowledge by following connections.

**Temporal salience**
Recently edited nodes glow brighter. Old nodes fade. The graph is a live picture of what's active in your mind, not a flat archive.

**Node weight**
Size grows with incoming references. The concepts your thinking keeps returning to become visually prominent automatically.

**One global graph**
No vaults. No workspaces. No decisions about where to put things. Everything lives in one graph. Structure emerges from connections.

---

## Keybinds

**Writing**

| Key | Action |
|---|---|
| `Ctrl + N` | New node |
| `[[Title]]` | Link to another node |
| `Esc` | Save and return to graph |

**Graph**

| Key | Action |
|---|---|
| Click node | Enter local view |
| Click node again | Open writing mode |
| Click neighbor | Expand the web |
| Click edge | Living Connections — see why they're linked |
| Scroll | Zoom |
| Drag | Pan canvas |
| Drag node | Move node |

**Navigate**

| Key | Action |
|---|---|
| `Ctrl + K` | Search nodes |
| `Esc` | Back / close panel |

**Data**

| Key | Action |
|---|---|
| `Ctrl + E` | Export all nodes to `.md` files |
| `Ctrl + I` | Import a folder of `.md` files |
| `Ctrl + H` | Help and keybinds |

---

## Installation

Bibbo ships as a native installer with the Java runtime bundled — no Java required on the user's machine.

### Windows
1. Download `bibbo-windows.msi` from the [latest release](../../releases/latest)
2. Double-click to install, follow the prompts
3. Launch Bibbo from the Start Menu

*If Windows Defender shows a warning: More info → Run anyway*

### macOS — Apple Silicon (M1 / M2 / M3 / M4)
1. Download `bibbo-mac-arm64.dmg` from the [latest release](../../releases/latest)
2. Open the DMG, drag Bibbo to Applications
3. First launch: right-click → **Open** → **Open** (one-time Gatekeeper bypass)

### macOS — Intel
Same as above, download `bibbo-mac-intel.dmg`

### Linux
1. Download `bibbo-linux.deb` from the [latest release](../../releases/latest)
2. `sudo dpkg -i bibbo-linux.deb`
3. Launch from your app menu or run `bibbo`

---

## Data

Your data is local. Always. No cloud, no account, no sync.

| Platform | Location |
|---|---|
| Windows | `%APPDATA%\Bibbo\bibbo.db` |
| macOS | `~/Library/Application Support/Bibbo/bibbo.db` |
| Linux | `~/.local/share/Bibbo/bibbo.db` |

---

## Stack

Java 21 · JavaFX 21 · SQLite · jpackage

Native installer per platform. JVM bundled. Single local database. No network calls.

---

## License

You may use Bibbo freely. You may not copy, fork, redistribute, or use the source code in any other project. See [LICENSE](LICENSE).

© 2026 HoraDomu. All rights reserved.
