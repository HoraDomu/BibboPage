<div align="center">
  <img src="assets/logo.svg" width="900" alt="Bibbo" />
</div>

<br/>

A native desktop knowledge graph. Write nodes. Connections emerge.

---

## Why this works

Your brain doesn't store knowledge in lists — it stores it in **connections**. When you link two ideas together you're not just filing them away, you're making a claim about *why* they relate. The more paths that lead to a concept, the deeper your understanding of it becomes.

Most note-taking tools fight this. Folders create silos. Lists hide relationships. Bullet points flatten what is inherently dimensional.

Bibbo is built around the structure of thought itself. Every `[[link]]` you write forces you to ask: *how does this connect to what I already know?* Over time your graph becomes a map of your understanding — and the gaps in it show you exactly where to go next. The nodes that grow largest are the ones your thinking keeps returning to. The isolated ones are the ideas you haven't fully integrated yet.

This is why traversing a knowledge graph is a fundamentally different experience than reading your old notes. You're not reviewing what you wrote — you're exploring what you understand.

---

## Why Bibbo over Obsidian?

Obsidian is a great piece of software. Bibbo is a different kind of tool.

**Obsidian is a platform. Bibbo is a single focused experience.**
Obsidian ships with a plugin ecosystem, themes, canvas, databases, and dozens of views. That flexibility is powerful — and it comes with a cost. You spend real time configuring it, choosing plugins, deciding on a folder structure. Bibbo has no configuration. You open it and think.

**In Obsidian, the graph is a feature. In Bibbo, the graph is the app.**
Obsidian's graph view is something you occasionally open to admire. It's decorative. In Bibbo, you write *into* the graph. Every new node appears in it. You navigate through it. The graph isn't a visualization of your notes — it is your notes.

**Bibbo's graph is alive.**
Obsidian's graph is static — nodes sit in fixed positions and don't respond. Bibbo uses physics-based simulation: nodes repel each other, spring toward their connections, and settle into organic arrangements. Drag one node and the whole network shifts. It feels like a brain, not a diagram.

**No folders. No hierarchy. No decisions.**
Obsidian still nudges you toward folders and structure. Bibbo has none. There is one graph. Everything lives in it. The only structure is the one that emerges from your thinking.

**Native, not Electron.**
Obsidian is built on Electron — it ships a full browser engine and weighs over 200MB. Bibbo is a single native binary under 10MB, built in Rust. It starts instantly, uses minimal memory, and never slows down.

**The goal is different.**
Obsidian is built to store everything. Bibbo is built to help you understand things better. Every design decision — the physics, the local graph drill-down, the full-screen writing mode, the way connections form automatically — is made with one question: *does this help the user think more clearly?*

---

## Installation

### Windows
1. Download `bibbo-windows.exe` from the [latest release](../../releases/latest)
2. Double-click to run — no installation needed
3. If Windows Defender shows a warning, click **More info → Run anyway** (expected for unsigned apps)

### macOS (Apple Silicon — M1/M2/M3)
1. Download `bibbo-mac-arm64` from the [latest release](../../releases/latest)
2. Open Terminal and run:
```bash
chmod +x ~/Downloads/bibbo-mac-arm64
~/Downloads/bibbo-mac-arm64
```
Or right-click the file in Finder → **Open** → **Open** (required the first time to bypass Gatekeeper)

### macOS (Intel)
Same steps as above but download `bibbo-mac-intel`.

### Linux
1. Download `bibbo-linux` from the [latest release](../../releases/latest)
2. Open a terminal and run:
```bash
chmod +x ~/Downloads/bibbo-linux
~/Downloads/bibbo-linux
```

**Your data** is stored locally at:
- Windows: `%APPDATA%\Bibbo\bibbo.db`
- macOS: `~/Library/Application Support/Bibbo/bibbo.db`
- Linux: `~/.local/share/Bibbo/bibbo.db`

---

## Quickstart

| Key | Action |
|---|---|
| `Ctrl + N` | New node |
| `Ctrl + K` | Search |
| `Ctrl + H` | Help & keybinds |
| `Ctrl + E` | Export to .md files |
| `Ctrl + I` | Import .md folder |
| Click node | Enter local view |
| Click node again | Open writing mode |
| Click neighbor | Expand web |
| Click edge | See why two nodes are connected |
| `Esc` | Back |

---

## License

Free to use. Source code may not be copied, forked, or distributed. See [LICENSE](LICENSE).

---

**Stack:** Rust · egui · SQLite — single binary, no cloud, no accounts.
