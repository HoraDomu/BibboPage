<div align="center">
  <img src="assets/logo.svg" width="900" alt="Bibbo" />
</div>

<br/>

A native desktop knowledge graph. Write nodes. Connections emerge.

---

## Why this works

Your brain doesn't store knowledge in lists it stores it in **connections**. When you link two ideas together you're not just filing them away, you're making a claim about *why* they relate. The more paths that lead to a concept, the deeper your understanding of it becomes.

Most note-taking tools fight this. Folders create silos. Lists hide relationships. Bullet points flatten what is inherently dimensional.

Bibbo is built around the structure of thought itself. Every `[[link]]` you write forces you to ask: *how does this connect to what I already know?* Over time your graph becomes a map of your understanding and the gaps in it show you exactly where to go next. The nodes that grow largest are the ones your thinking keeps returning to. The isolated ones are the ideas you haven't fully integrated yet.

This is why traversing a knowledge graph is a fundamentally different experience than reading your old notes. You're not reviewing what you wrote you're exploring what you understand.

---

## The science

Human memory is **associative, not hierarchical**. Collins & Loftus (1975) showed that recall works through *spreading activation* you access a memory by traversing related concepts, not by navigating folders. When you think "Einstein," you don't open a mental drawer labeled "Physics > 20th Century." You move through a web of associations.

Folders fight this. A graph mirrors it.

The act of *linking* two concepts is itself a cognitive operation. It forces you to articulate why they're related, which deepens encoding a mechanism called **elaborative interrogation**: the questioning act is the learning act. A link isn't the insight. The *why* is.

Three things Bibbo does that matter scientifically:

**Living Connections relationship epistemology.**  
Obsidian links are boolean: connected or not. Bibbo's Living Connections expose *why* two nodes are connected which documents reference them together, how recently, how strongly. You don't just know that Einstein and relativity are related; you know *how* and *in what contexts*. That contextual metadata is epistemically meaningful.

**Temporal decay and salience.**  
Old nodes fade, recently edited nodes glow. This mirrors the brain's recency and frequency biases and makes the graph a living picture of your current thinking rather than a static archive. Obsidian's graph shows you everything equally. Bibbo shows you what's active in your mind right now.

**Weight from incoming references.**  
Node size grows with references. This is a PageRank-style importance signal derived from your own knowledge, not someone else's. The nodes that matter to your thinking become visually prominent automatically.

The constraint of no folders isn't a limitation it's the point. Bibbo bets that forcing you to answer only "what does this connect to?" instead of "where does this go?" produces a more honest map of how you actually understand things.

---

## Why Bibbo over Obsidian?

Obsidian is a great piece of software. Bibbo is a different kind of tool.

**Obsidian is a platform. Bibbo is a single focused experience.**  
Obsidian ships with a plugin ecosystem, themes, canvas, and dozens of views. That flexibility comes with a cost you spend real time configuring it, choosing plugins, deciding on a folder structure. Bibbo has no configuration. You open it and think.

**In Obsidian, the graph is a feature. In Bibbo, the graph is the app.**  
Obsidian's graph view is something you occasionally open to admire. It's decorative. In Bibbo, you write *into* the graph. Every new node appears in it. You navigate through it. The graph isn't a visualization of your notes it is your notes.

**Bibbo's graph is alive.**  
Obsidian's graph is static nodes sit in fixed positions and don't respond. Bibbo uses physics-based simulation: nodes repel, spring toward their connections, and settle into organic arrangements. Drag a node and the whole network shifts. It feels like a brain, not a diagram.

**No folders. No hierarchy. No decisions.**  
Obsidian still nudges you toward folders and structure. Bibbo has none. There is one graph. Everything lives in it. The only structure is the one that emerges from your thinking.

**Native, not Electron.**  
Obsidian ships a full browser engine and weighs over 200MB. Bibbo is a single native binary under 10MB, built in Rust. It opens instantly and never slows down.

**The goal is different.**  
Obsidian is built to store everything. Bibbo is built to help you *understand* things better. Every design decision the physics, the local graph, the writing mode, the way connections form automatically exists to answer one question: *does this help you think more clearly?*

---

## Installation

### Windows
1. Download `bibbo-windows.exe` from the [latest release](../../releases/latest)
2. Double-click to run no installation needed
3. If Windows Defender shows a warning, click **More info → Run anyway** (expected for unsigned apps)

### macOS (Apple Silicon M1/M2/M3)
1. Download `bibbo-mac-arm64` from the [latest release](../../releases/latest)
2. Right-click in Finder → **Open** → **Open** (required the first time to bypass Gatekeeper)
3. If blocked, go to **System Settings → Privacy & Security** → click **Open Anyway**

Or via Terminal:
```bash
chmod +x ~/Downloads/bibbo-mac-arm64
~/Downloads/bibbo-mac-arm64
```

### macOS (Intel)
Same steps as above but download `bibbo-mac-intel`.

### Linux
1. Download `bibbo-linux` from the [latest release](../../releases/latest)
2. Open a terminal and run:
```bash
chmod +x ~/Downloads/bibbo-linux
~/Downloads/bibbo-linux
```

**Your data is stored locally:**
- Windows: `%APPDATA%\Bibbo\bibbo.db`
- macOS: `~/Library/Application Support/Bibbo/bibbo.db`
- Linux: `~/.local/share/Bibbo/bibbo.db`

---

## Quickstart

**Writing**

| Key | Action |
|---|---|
| `Ctrl + N` | New node |
| `[[Title]]` | Link to a node |
| `Esc` | Save & return |

**Graph**

| Key | Action |
|---|---|
| Click node | Enter local view |
| Click again | Open writing mode |
| Click neighbor | Expand web |
| Click edge | See why two nodes are connected |

**Navigate**

| Key | Action |
|---|---|
| `Ctrl + K` | Search |
| Scroll | Zoom |
| Drag | Pan canvas |
| `Esc` | Back / close |

**Data**

| Key | Action |
|---|---|
| `Ctrl + E` | Export to .md files |
| `Ctrl + I` | Import .md folder |
| `Ctrl + H` | Help & keybinds |

---

## License

Free to use. Source code may not be copied, forked, or distributed. See [LICENSE](LICENSE).

---

**Stack:** Rust · egui · SQLite single binary, no cloud, no accounts.
