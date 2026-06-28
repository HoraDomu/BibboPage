use eframe::egui;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

const COLORS: &[egui::Color32] = &[
    egui::Color32::from_rgb(255, 107, 107),
    egui::Color32::from_rgb(255, 159, 67),
    egui::Color32::from_rgb(255, 206, 84),
    egui::Color32::from_rgb(46, 213, 115),
    egui::Color32::from_rgb(30, 144, 255),
    egui::Color32::from_rgb(147, 51, 234),
    egui::Color32::from_rgb(236, 72, 153),
    egui::Color32::from_rgb(20, 184, 166),
];

fn main() -> eframe::Result {
    eframe::run_native(
        "Bibbo",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Bibbo")
                .with_inner_size([1200.0, 800.0])
                .with_min_inner_size([800.0, 600.0]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(BibboApp::new(cc)))),
    )
}

// ── Data ─────────────────────────────────────────────────────────────────────

struct Node {
    id: i64,
    title: String,
    body: String,
    #[allow(dead_code)]
    color: egui::Color32,
    pos: egui::Pos2,
    vel: egui::Vec2,
    dragging: bool,
    dirty: bool,
}

struct Edge {
    source_id: i64,
    target_id: i64,
}

// ── Physics ───────────────────────────────────────────────────────────────────

const MIN_DIST: f32    = 110.0;
const REPULSION: f32   = 2200.0;
const DAMPING: f32     = 5.5;
const SPRING_K: f32    = 1.8;
const SPRING_REST: f32 = 145.0;
const SPRING_DEAD: f32 = 3.0;
const STOP_VEL: f32    = 1.5;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn node_radius(conns: usize) -> f32 {
    5.0 + (1.0 - (-(conns as f32) / 6.0).exp()) * 6.0
}

fn edge_rest_len(a: i64, b: i64) -> f32 {
    let (lo, hi) = if a < b { (a as u64, b as u64) } else { (b as u64, a as u64) };
    let h = lo.wrapping_mul(6364136223846793005).wrapping_add(hi);
    let t = (h & 0xFF) as f32 / 255.0;
    SPRING_REST * (0.80 + t * 0.40)
}

fn spawn_pos(n: usize, view_center: egui::Pos2) -> egui::Pos2 {
    let s = (n as u64).wrapping_add(1);
    let ax = s.wrapping_mul(2654435761).wrapping_add(1013904223);
    let ay = ax.wrapping_mul(2654435761).wrapping_add(1013904223);
    let rx = (ax & 0xFF) as f32 / 255.0 - 0.5;
    let ry = (ay & 0xFF) as f32 / 255.0 - 0.5;
    egui::pos2(view_center.x + rx * 220.0, view_center.y + ry * 220.0)
}

fn date_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let s = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let z = s / 86400 + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    const MON: &[&str] = &["January","February","March","April","May","June",
                            "July","August","September","October","November","December"];
    format!("{} {d}, {y}", MON[(m as usize).saturating_sub(1)])
}

fn parse_links(body: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        if let Some(end) = rest.find("]]") {
            let tag = rest[..end].trim().to_string();
            if !tag.is_empty() { links.push(tag); }
            rest = &rest[end + 2..];
        } else { break; }
    }
    links
}

fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn point_segment_dist(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq < 0.001 { return (p - a).length(); }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

fn parse_md_file(content: &str, path: &std::path::Path) -> (String, String) {
    let mut lines = content.lines();
    let title = loop {
        match lines.next() {
            None => break String::new(),
            Some(l) => {
                let trimmed = l.trim();
                if trimmed.starts_with("# ") {
                    break trimmed[2..].trim().to_string();
                } else if !trimmed.is_empty() {
                    // First non-empty line is not a heading — use filename
                    break path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                }
            }
        }
    };
    // Body: everything after the first heading (or all content if no heading)
    let body_start = if content.contains("\n# ") || content.starts_with("# ") {
        content.find('\n').map(|i| i + 1).unwrap_or(content.len())
    } else { 0 };
    let body = content[body_start..].trim().to_string();
    (title, body)
}

fn connection_reasons(a: &Node, b: &Node) -> Vec<String> {
    let a_tags = parse_links(&a.body);
    let b_tags = parse_links(&b.body);
    let a_title = normalize(&a.title);
    let b_title = normalize(&b.title);
    let mut reasons: Vec<String> = Vec::new();
    for tag in &a_tags {
        let tn = normalize(tag);
        if tn == b_title || b_tags.iter().any(|bt| normalize(bt) == tn) {
            let s = format!("[[{}]]", tag);
            if !reasons.contains(&s) { reasons.push(s); }
        }
    }
    for tag in &b_tags {
        let tn = normalize(tag);
        if tn == a_title || a_tags.iter().any(|at| normalize(at) == tn) {
            let s = format!("[[{}]]", tag);
            if !reasons.contains(&s) { reasons.push(s); }
        }
    }
    reasons
}

// ── App ───────────────────────────────────────────────────────────────────────

struct BibboApp {
    db: Connection,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    canvas: egui::Vec2,
    // Camera
    pan: egui::Vec2,
    pan_vel: egui::Vec2,
    zoom: f32,
    zoom_target: f32,
    zoom_anchor: egui::Vec2,
    is_panning: bool,
    press_origin: egui::Pos2,
    initialized: bool,
    fly_to: Option<egui::Pos2>,
    // Local graph traversal
    local_root: Option<i64>,
    local_visible: HashSet<i64>,
    nav_history: Vec<i64>,
    // Full-screen writing mode
    writing: bool,
    writing_new: bool,
    writing_focus_title: bool,
    draft_title: String,
    draft_body: String,
    editing_id: Option<i64>,
    // Living Connections (edge click panel)
    selected_edge: Option<(i64, i64, egui::Pos2)>,
    // Search
    search_open: bool,
    search_query: String,
    search_sel: usize,
    // Color cycling for new nodes
    color_idx: usize,
    // Toast notification (message, seconds remaining)
    toast: Option<(String, f32)>,
    // Writing mode delete confirmation
    writing_confirm_delete: bool,
    // Help overlay
    help_open: bool,
    // Delete-all confirmation
    confirm_delete_all: bool,
}

impl BibboApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut v = egui::Visuals::dark();
        v.panel_fill = egui::Color32::BLACK;
        v.window_fill = egui::Color32::from_rgb(10, 10, 14);
        cc.egui_ctx.set_visuals(v);

        let db_path = {
            let mut p = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            p.push("Bibbo");
            std::fs::create_dir_all(&p).ok();
            p.push("bibbo.db");
            p
        };
        let db = Connection::open(&db_path).expect("open db");
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS nodes (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                title     TEXT    NOT NULL,
                body      TEXT    NOT NULL,
                color_idx INTEGER NOT NULL,
                pos_x     REAL    NOT NULL,
                pos_y     REAL    NOT NULL,
                created   TEXT    NOT NULL
            );
            CREATE TABLE IF NOT EXISTS edges (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                source_id INTEGER NOT NULL,
                target_id INTEGER NOT NULL,
                UNIQUE(source_id, target_id)
            );",
        ).expect("init db");

        let nodes = load_nodes(&db);
        let edges = load_edges(&db);
        let color_idx = nodes.len() % COLORS.len();

        Self {
            db, nodes, edges,
            canvas: egui::vec2(1200.0, 800.0),
            pan: egui::Vec2::ZERO, pan_vel: egui::Vec2::ZERO,
            zoom: 1.0, zoom_target: 1.0,
            zoom_anchor: egui::Vec2::ZERO,
            is_panning: false, press_origin: egui::Pos2::ZERO,
            initialized: false, fly_to: None,
            local_root: None, local_visible: HashSet::new(), nav_history: Vec::new(),
            writing: false, writing_new: false, writing_focus_title: false,
            draft_title: String::new(), draft_body: String::new(), editing_id: None,
            selected_edge: None,
            search_open: false, search_query: String::new(), search_sel: 0,
            color_idx,
            toast: None,
            writing_confirm_delete: false,
            help_open: false,
            confirm_delete_all: false,
        }
    }

    // ── Camera ────────────────────────────────────────────────────────────────

    fn to_screen(&self, world: egui::Pos2) -> egui::Pos2 {
        egui::pos2(world.x * self.zoom + self.pan.x, world.y * self.zoom + self.pan.y)
    }

    fn to_world(&self, screen: egui::Pos2) -> egui::Pos2 {
        egui::pos2((screen.x - self.pan.x) / self.zoom, (screen.y - self.pan.y) / self.zoom)
    }

    fn view_center(&self) -> egui::Pos2 {
        self.to_world(egui::pos2(self.canvas.x * 0.5, self.canvas.y * 0.5))
    }

    // ── Local graph traversal ─────────────────────────────────────────────────

    fn compute_neighbors(&self, id: i64) -> Vec<i64> {
        self.edges.iter().filter_map(|e| {
            if e.source_id == id { Some(e.target_id) }
            else if e.target_id == id { Some(e.source_id) }
            else { None }
        }).collect()
    }

    fn enter_local(&mut self, id: i64) {
        self.local_root = Some(id);
        self.local_visible.clear();
        self.local_visible.insert(id);
        for n in self.compute_neighbors(id) { self.local_visible.insert(n); }
        self.nav_history.clear();
        if let Some(p) = self.nodes.iter().find(|n| n.id == id).map(|n| n.pos) {
            self.fly_to = Some(p);
        }
        self.apply_bloom(id);
    }

    fn expand_local(&mut self, new_root: i64) {
        if let Some(old) = self.local_root { self.nav_history.push(old); }
        self.local_root = Some(new_root);
        self.local_visible.insert(new_root);
        for n in self.compute_neighbors(new_root) { self.local_visible.insert(n); }
        self.apply_bloom(new_root);
        if let Some(p) = self.nodes.iter().find(|n| n.id == new_root).map(|n| n.pos) {
            self.fly_to = Some(p);
        }
    }

    fn local_back(&mut self) {
        if let Some(prev) = self.nav_history.pop() {
            self.local_root = Some(prev);
            if let Some(p) = self.nodes.iter().find(|n| n.id == prev).map(|n| n.pos) {
                self.fly_to = Some(p);
            }
        } else {
            self.local_root = None;
            self.local_visible.clear();
        }
    }

    fn apply_bloom(&mut self, center_id: i64) {
        let cp = self.nodes.iter().find(|n| n.id == center_id).map(|n| n.pos);
        if let Some(cp) = cp {
            let neighbors = self.compute_neighbors(center_id);
            for n in &mut self.nodes {
                if neighbors.contains(&n.id) {
                    let dir = n.pos - cp;
                    let len = dir.length();
                    if len > 0.1 { n.vel += (dir / len) * 60.0; }
                }
            }
        }
    }

    // ── Writing mode ──────────────────────────────────────────────────────────

    fn open_writing_new(&mut self) {
        self.draft_title.clear();
        self.draft_body.clear();
        self.editing_id = None;
        self.writing = true;
        self.writing_new = true;
        self.writing_focus_title = true;
        self.writing_confirm_delete = false;
    }

    fn open_writing_edit(&mut self, id: i64) {
        if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
            self.draft_title = node.title.clone();
            self.draft_body  = node.body.clone();
        }
        self.editing_id = Some(id);
        self.writing = true;
        self.writing_new = false;
        self.writing_focus_title = false;
        self.writing_confirm_delete = false;
    }

    fn commit_writing(&mut self) {
        let title = self.draft_title.trim().to_string();

        if let Some(id) = self.editing_id.take() {
            // Update existing node
            if !title.is_empty() {
                let body = self.draft_body.clone();
                let _ = self.db.execute(
                    "UPDATE nodes SET title=?1, body=?2 WHERE id=?3",
                    rusqlite::params![title, body, id],
                );
                if let Some(n) = self.nodes.iter_mut().find(|n| n.id == id) {
                    n.title = title; n.body = body.clone();
                }
                self.rebuild_edges_for(id, &self.draft_body.clone());
            }
            self.writing = false;
            self.enter_local(id);
        } else if !title.is_empty() {
            // Create new node
            let body = self.draft_body.clone();
            let ci = self.color_idx;
            self.color_idx = (ci + 1) % COLORS.len();
            let pos = spawn_pos(self.nodes.len(), self.view_center());
            let _ = self.db.execute(
                "INSERT INTO nodes (title,body,color_idx,pos_x,pos_y,created) VALUES(?1,?2,?3,?4,?5,?6)",
                rusqlite::params![title, body, ci as i64, pos.x as f64, pos.y as f64, date_string()],
            );
            let id = self.db.last_insert_rowid();
            let h = (id as u64).wrapping_mul(6364136223846793005);
            let kick = egui::vec2(
                (h & 0xFF) as f32 / 255.0 * 120.0 - 60.0,
                ((h >> 8) & 0xFF) as f32 / 255.0 * 120.0 - 60.0,
            );
            self.nodes.push(Node {
                id, title, body: body.clone(), color: COLORS[ci],
                pos, vel: kick, dragging: false, dirty: false,
            });
            self.rebuild_edges_for(id, &body);
            self.writing = false;
            self.enter_local(id);
        } else {
            // Empty title — just cancel
            self.writing = false;
        }
    }

    // ── Node data ops ─────────────────────────────────────────────────────────

    fn rebuild_edges_for(&mut self, node_id: i64, body: &str) {
        let _ = self.db.execute("DELETE FROM edges WHERE source_id=?1", rusqlite::params![node_id]);
        self.edges.retain(|e| e.source_id != node_id);

        let my_title_norm = self.nodes.iter().find(|n| n.id == node_id)
            .map(|n| normalize(&n.title)).unwrap_or_default();
        let my_tags: Vec<String> = parse_links(body);

        let targets: Vec<i64> = self.nodes.iter()
            .filter(|n| n.id != node_id)
            .filter(|n| {
                let their_title = normalize(&n.title);
                let their_tags: Vec<String> = parse_links(&n.body);
                let forward = my_tags.iter().any(|t| {
                    let tn = normalize(t);
                    their_title == tn || their_tags.iter().any(|tt| normalize(tt) == tn)
                });
                let reverse = !my_title_norm.is_empty()
                    && (their_title == my_title_norm
                        || their_tags.iter().any(|tt| normalize(tt) == my_title_norm));
                forward || reverse
            })
            .map(|n| n.id)
            .collect();

        for target_id in targets {
            let _ = self.db.execute(
                "INSERT OR IGNORE INTO edges (source_id, target_id) VALUES (?1, ?2)",
                rusqlite::params![node_id, target_id],
            );
            self.edges.push(Edge { source_id: node_id, target_id });
        }
    }

    fn save_position(&self, id: i64, pos: egui::Pos2) {
        let _ = self.db.execute(
            "UPDATE nodes SET pos_x=?1, pos_y=?2 WHERE id=?3",
            rusqlite::params![pos.x as f64, pos.y as f64, id],
        );
    }

    // ── Import / Export ───────────────────────────────────────────────────────

    fn export_vault(&mut self) {
        let Some(dir) = rfd::FileDialog::new().set_title("Export to folder").pick_folder()
        else { return };

        let mut count = 0usize;
        for node in &self.nodes {
            let safe_name = node.title
                .chars()
                .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
                .collect::<String>();
            let path = dir.join(format!("{}.md", safe_name));
            let content = if node.body.trim().is_empty() {
                format!("# {}\n", node.title)
            } else {
                format!("# {}\n\n{}\n", node.title, node.body)
            };
            if std::fs::write(&path, content).is_ok() { count += 1; }
        }
        self.toast = Some((format!("Exported {} notes", count), 3.0));
    }

    fn import_vault(&mut self) {
        let Some(dir) = rfd::FileDialog::new().set_title("Import from folder").pick_folder()
        else { return };

        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => { self.toast = Some(("Could not read folder".into(), 3.0)); return; }
        };

        let existing_titles: HashSet<String> = self.nodes.iter().map(|n| normalize(&n.title)).collect();
        let mut imported = 0usize;
        let mut new_ids: Vec<i64> = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") { continue; }
            let Ok(content) = std::fs::read_to_string(&path) else { continue };

            // Title: first `# Heading` line, or stem of filename
            let (title, body) = parse_md_file(&content, &path);
            if title.is_empty() { continue; }
            if existing_titles.contains(&normalize(&title)) { continue; }

            let ci = self.color_idx;
            self.color_idx = (ci + 1) % COLORS.len();
            let pos = spawn_pos(self.nodes.len(), egui::pos2(0.0, 0.0));
            let _ = self.db.execute(
                "INSERT INTO nodes (title,body,color_idx,pos_x,pos_y,created) VALUES(?1,?2,?3,?4,?5,?6)",
                rusqlite::params![title, body, ci as i64, pos.x as f64, pos.y as f64, date_string()],
            );
            let id = self.db.last_insert_rowid();
            self.nodes.push(Node {
                id, title, body, color: COLORS[ci],
                pos, vel: egui::Vec2::ZERO, dragging: false, dirty: false,
            });
            new_ids.push(id);
            imported += 1;
        }

        // Rebuild edges for all imported nodes
        for id in new_ids {
            let body = self.nodes.iter().find(|n| n.id == id).map(|n| n.body.clone()).unwrap_or_default();
            self.rebuild_edges_for(id, &body);
        }

        self.toast = Some((format!("Imported {} notes", imported), 3.5));
    }

    // ── Search ────────────────────────────────────────────────────────────────

    fn search_results(&self, query: &str) -> Vec<usize> {
        if query.is_empty() {
            let mut idxs: Vec<usize> = (0..self.nodes.len()).collect();
            idxs.sort_by(|&a, &b| self.nodes[b].id.cmp(&self.nodes[a].id));
            idxs.truncate(8);
            return idxs;
        }
        let q = query.to_lowercase();
        let mut scored: Vec<(usize, i32)> = self.nodes.iter().enumerate()
            .filter_map(|(i, n)| {
                let t = n.title.to_lowercase();
                let score = if t.starts_with(&q) { 3 }
                       else if t.contains(&q) { 2 }
                       else if n.body.to_lowercase().contains(&q) { 1 }
                       else { 0 };
                if score > 0 { Some((i, score)) } else { None }
            }).collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.truncate(8);
        scored.into_iter().map(|(i, _)| i).collect()
    }

    // ── Full-screen writing view ───────────────────────────────────────────────

    fn show_writing(&mut self, ctx: &egui::Context) {
        let exit = ctx.input(|i| i.key_pressed(egui::Key::Escape));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(6, 6, 8)))
            .show(ctx, |ui| {
                let total_w = ui.available_width();
                let content_w = (680.0_f32).min(total_w - 80.0);
                let side = (total_w - content_w) / 2.0;

                // ── Back button top-left ──────────────────────────────────
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space(side.max(24.0));
                    let back_label = if self.editing_id.is_some() {
                        format!("← {}", self.draft_title.trim())
                    } else {
                        "← graph".to_string()
                    };
                    if ui.add(egui::Button::new(
                        egui::RichText::new(&back_label)
                            .color(egui::Color32::from_rgb(50, 48, 60))
                            .size(12.0)
                    ).frame(false)).clicked() || exit {
                        self.commit_writing();
                    }
                });

                ui.add_space(48.0);

                // ── Centered content ──────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.add_space(side);
                    ui.vertical(|ui| {
                        ui.set_max_width(content_w);

                        // Title
                        let tr = ui.add(
                            egui::TextEdit::singleline(&mut self.draft_title)
                                .hint_text("Title")
                                .font(egui::FontId::proportional(30.0))
                                .text_color(egui::Color32::from_rgb(238, 237, 243))
                                .desired_width(content_w)
                                .frame(false),
                        );
                        if self.writing_focus_title {
                            tr.request_focus();
                            self.writing_focus_title = false;
                        }

                        ui.add_space(14.0);
                        ui.add(egui::Separator::default());
                        ui.add_space(18.0);

                        // Body
                        let body_h = ui.available_height().max(240.0) - 44.0;
                        let br = ui.add(
                            egui::TextEdit::multiline(&mut self.draft_body)
                                .hint_text("Start writing...  use [[Node Title]] to link ideas")
                                .font(egui::FontId::proportional(16.0))
                                .text_color(egui::Color32::from_rgb(188, 186, 200))
                                .desired_width(content_w)
                                .desired_rows((body_h / 22.0) as usize)
                                .frame(false),
                        );
                        if !self.writing_new && !self.writing_focus_title {
                            br.request_focus();
                        }

                        ui.add_space(12.0);
                        let wc = self.draft_body.split_whitespace().count();
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{wc} words  ·  Esc to return"))
                                    .color(egui::Color32::from_rgb(38, 36, 48))
                                    .size(11.0),
                            );
                            // Delete only for existing nodes
                            if self.editing_id.is_some() {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if self.writing_confirm_delete {
                                        if ui.add(egui::Button::new(
                                            egui::RichText::new("Confirm delete")
                                                .color(egui::Color32::from_rgb(210, 60, 60))
                                                .size(11.0)
                                        ).frame(false)).clicked() {
                                            if let Some(id) = self.editing_id.take() {
                                                let _ = self.db.execute("DELETE FROM nodes WHERE id=?1", rusqlite::params![id]);
                                                let _ = self.db.execute("DELETE FROM edges WHERE source_id=?1 OR target_id=?1", rusqlite::params![id]);
                                                self.nodes.retain(|n| n.id != id);
                                                self.edges.retain(|e| e.source_id != id && e.target_id != id);
                                                if self.local_root == Some(id) {
                                                    self.local_root = None;
                                                    self.local_visible.clear();
                                                    self.nav_history.clear();
                                                }
                                            }
                                            self.writing = false;
                                            self.writing_confirm_delete = false;
                                        }
                                        ui.add_space(8.0);
                                        if ui.add(egui::Button::new(
                                            egui::RichText::new("Cancel")
                                                .color(egui::Color32::from_rgb(55, 53, 65))
                                                .size(11.0)
                                        ).frame(false)).clicked() {
                                            self.writing_confirm_delete = false;
                                        }
                                    } else if ui.add(egui::Button::new(
                                        egui::RichText::new("Delete")
                                            .color(egui::Color32::from_rgb(70, 50, 50))
                                            .size(11.0)
                                    ).frame(false)).clicked() {
                                        self.writing_confirm_delete = true;
                                    }
                                });
                            }
                        });
                    });
                });
            });
    }
}

fn load_nodes(db: &Connection) -> Vec<Node> {
    let Ok(mut s) = db.prepare("SELECT id, title, body, color_idx, pos_x, pos_y FROM nodes ORDER BY id")
    else { return vec![] };
    let Ok(rows) = s.query_map([], |r| Ok((
        r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
        r.get::<_, i64>(3)? as usize, r.get::<_, f64>(4)? as f32, r.get::<_, f64>(5)? as f32,
    ))) else { return vec![] };
    rows.filter_map(|r| r.ok())
        .map(|(id, title, body, ci, x, y)| Node {
            id, title, body, color: COLORS[ci % COLORS.len()],
            pos: egui::pos2(x, y), vel: egui::Vec2::ZERO,
            dragging: false, dirty: false,
        }).collect()
}

fn load_edges(db: &Connection) -> Vec<Edge> {
    let Ok(mut s) = db.prepare("SELECT source_id, target_id FROM edges") else { return vec![] };
    let Ok(rows) = s.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
    else { return vec![] };
    rows.filter_map(|r| r.ok())
        .map(|(source_id, target_id)| Edge { source_id, target_id }).collect()
}

// ── Frame loop ────────────────────────────────────────────────────────────────

impl eframe::App for BibboApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Writing mode takes over the whole frame ───────────────────────
        if self.writing {
            self.show_writing(ctx);
            return;
        }

        // ── Global shortcuts ──────────────────────────────────────────────
        if ctx.input(|i| i.key_pressed(egui::Key::N) && i.modifiers.ctrl) && !self.search_open {
            self.open_writing_new();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::K) && i.modifiers.ctrl) {
            self.search_open = !self.search_open;
            self.search_query.clear();
            self.search_sel = 0;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::E) && i.modifiers.ctrl) && !self.search_open {
            self.export_vault();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::I) && i.modifiers.ctrl) && !self.search_open {
            self.import_vault();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::H) && i.modifiers.ctrl) {
            self.help_open = !self.help_open;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.help_open                { self.help_open = false; }
            else if self.search_open         { self.search_open = false; }
            else if self.selected_edge.is_some() { self.selected_edge = None; }
            else if self.local_root.is_some()    { self.local_back(); }
        }

        let dt = ctx.input(|i| i.unstable_dt).clamp(0.001, 0.05);
        let primary_down    = ctx.input(|i| i.pointer.primary_down());
        let primary_pressed = ctx.input(|i| i.pointer.primary_pressed());
        let pointer_screen  = ctx.input(|i| i.pointer.interact_pos());

        let mut conn_count: HashMap<i64, usize> = HashMap::new();
        for edge in &self.edges {
            *conn_count.entry(edge.source_id).or_insert(0) += 1;
            *conn_count.entry(edge.target_id).or_insert(0) += 1;
        }

        let in_local = self.local_root.is_some();
        let local_vis = self.local_visible.clone();

        // ── Smooth zoom ───────────────────────────────────────────────────
        if !self.search_open {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                if let Some(ms) = pointer_screen {
                    let factor = (scroll * 0.004).exp();
                    self.zoom_target = (self.zoom_target * factor).clamp(0.08, 8.0);
                    self.zoom_anchor = ms.to_vec2();
                }
            }
        }
        let zoom_diff = self.zoom_target - self.zoom;
        if zoom_diff.abs() > 0.0005 {
            let old = self.zoom;
            self.zoom += zoom_diff * (1.0 - (-12.0 * dt).exp());
            let ratio = self.zoom / old;
            self.pan.x = self.zoom_anchor.x - (self.zoom_anchor.x - self.pan.x) * ratio;
            self.pan.y = self.zoom_anchor.y - (self.zoom_anchor.y - self.pan.y) * ratio;
            ctx.request_repaint();
        }

        // ── Camera glide ──────────────────────────────────────────────────
        if let Some(target) = self.fly_to {
            let tp = egui::vec2(
                self.canvas.x * 0.5 - target.x * self.zoom,
                self.canvas.y * 0.5 - target.y * self.zoom,
            );
            let diff = tp - self.pan;
            if diff.length() > 0.5 {
                self.pan += diff * (1.0 - (-9.0 * dt).exp());
                self.pan_vel = egui::Vec2::ZERO;
                ctx.request_repaint();
            } else {
                self.pan = tp; self.fly_to = None;
            }
        }

        // ── Press: drag / edge click / pan ───────────────────────────────
        if primary_pressed && !self.search_open {
            if let Some(ps) = pointer_screen {
                self.press_origin = ps;
                self.pan_vel = egui::Vec2::ZERO;
                let wp = self.to_world(ps);
                let hit_node = self.nodes.iter().rposition(|n| {
                    if in_local && !local_vis.contains(&n.id) { return false; }
                    let r = node_radius(*conn_count.get(&n.id).unwrap_or(&0));
                    (n.pos - wp).length() < r + 8.0 / self.zoom
                });
                if let Some(idx) = hit_node {
                    self.nodes[idx].dragging = true;
                } else {
                    let edge_hit = self.edges.iter().find(|e| {
                        if in_local && (!local_vis.contains(&e.source_id) || !local_vis.contains(&e.target_id)) { return false; }
                        let s = self.nodes.iter().find(|n| n.id == e.source_id);
                        let t = self.nodes.iter().find(|n| n.id == e.target_id);
                        if let (Some(s), Some(t)) = (s, t) {
                            point_segment_dist(ps, self.to_screen(s.pos), self.to_screen(t.pos)) < 8.0
                        } else { false }
                    }).map(|e| (e.source_id, e.target_id));

                    if let Some((sid, tid)) = edge_hit {
                        self.selected_edge = Some((sid, tid, ps));
                    } else {
                        self.selected_edge = None;
                        self.is_panning = true;
                    }
                }
            }
        }

        // ── Pan with momentum ─────────────────────────────────────────────
        if !primary_down { self.is_panning = false; }
        if self.is_panning {
            let delta = ctx.input(|i| i.pointer.delta());
            if delta != egui::Vec2::ZERO {
                self.pan += delta;
                let iv = if dt > 0.0 { delta / dt } else { egui::Vec2::ZERO };
                self.pan_vel = self.pan_vel * 0.7 + iv * 0.3;
                ctx.request_repaint();
            }
        } else if self.pan_vel.length_sq() > 4.0 {
            self.pan += self.pan_vel * dt;
            self.pan_vel *= (-5.0 * dt).exp();
            ctx.request_repaint();
        } else {
            self.pan_vel = egui::Vec2::ZERO;
        }

        // ── Node drag + click detection ───────────────────────────────────
        let mut clicked_id: Option<i64> = None;
        if let Some(ps) = pointer_screen {
            let pan = self.pan; let zoom = self.zoom;
            let press_origin = self.press_origin;
            for node in &mut self.nodes {
                if !node.dragging { continue; }
                if primary_down {
                    let old = node.pos;
                    node.pos = egui::pos2((ps.x - pan.x) / zoom, (ps.y - pan.y) / zoom);
                    let fv = if dt > 0.0 { (node.pos - old) / dt } else { egui::Vec2::ZERO };
                    node.vel = node.vel * 0.6 + fv * 0.4;
                } else {
                    node.dragging = false;
                    if (ps - press_origin).length() < 6.0 {
                        clicked_id = Some(node.id);
                        node.vel = egui::Vec2::ZERO;
                    } else {
                        node.dirty = true;
                        let speed = node.vel.length();
                        if speed > 400.0 { node.vel = node.vel / speed * 400.0; }
                    }
                }
            }
        }

        // ── Physics ───────────────────────────────────────────────────────
        let n = self.nodes.len();
        let mut forces = vec![egui::Vec2::ZERO; n];

        for i in 0..n {
            for j in (i + 1)..n {
                let delta = self.nodes[i].pos - self.nodes[j].pos;
                let dist  = delta.length();
                if dist < MIN_DIST && dist > 0.5 {
                    let t = 1.0 - dist / MIN_DIST;
                    let dir = delta / dist;
                    forces[i] += dir * (REPULSION * t * t);
                    forces[j] -= dir * (REPULSION * t * t);
                }
            }
        }

        let id_to_idx: HashMap<i64, usize> =
            self.nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
        for edge in &self.edges {
            let (Some(&i), Some(&j)) = (id_to_idx.get(&edge.source_id), id_to_idx.get(&edge.target_id))
            else { continue };
            let delta = self.nodes[j].pos - self.nodes[i].pos;
            let dist  = delta.length();
            if dist < 0.5 { continue; }
            let rest = edge_rest_len(edge.source_id, edge.target_id);
            let disp = dist - rest;
            if disp.abs() < SPRING_DEAD { continue; }
            let f = SPRING_K * disp * (delta / dist);
            forces[i] += f; forces[j] -= f;
        }

        let damp = (-DAMPING * dt).exp();
        let mut to_save: Option<(i64, egui::Pos2)> = None;
        let mut any_active = false;
        for (i, node) in self.nodes.iter_mut().enumerate() {
            if node.dragging { any_active = true; continue; }
            node.vel += forces[i] * dt;
            node.vel *= damp;
            if node.vel.length() > STOP_VEL {
                node.pos += node.vel * dt;
                any_active = true;
            } else {
                node.vel = egui::Vec2::ZERO;
                if node.dirty { node.dirty = false; to_save = Some((node.id, node.pos)); }
            }
        }
        if let Some((id, pos)) = to_save { self.save_position(id, pos); }
        if any_active { ctx.request_repaint(); }

        // ── Toast tick ────────────────────────────────────────────────────
        if let Some((_, ref mut t)) = self.toast {
            *t -= dt;
            if *t <= 0.0 { self.toast = None; }
            ctx.request_repaint();
        }

        // ── Handle click ──────────────────────────────────────────────────
        if let Some(id) = clicked_id {
            if !in_local {
                // Full graph: enter local mode
                self.enter_local(id);
                ctx.request_repaint();
            } else if self.local_root == Some(id) {
                // Tap the root node again → write
                self.open_writing_edit(id);
            } else if local_vis.contains(&id) {
                // Tap a neighbor → expand + shift root
                self.expand_local(id);
                ctx.request_repaint();
            }
        }

        // ── Hover ─────────────────────────────────────────────────────────
        let any_dragging = self.nodes.iter().any(|n| n.dragging);
        let hovered_id = if any_dragging { None } else {
            pointer_screen.and_then(|ps| {
                let wp = self.to_world(ps);
                self.nodes.iter()
                    .filter(|n| !in_local || local_vis.contains(&n.id))
                    .filter_map(|n| {
                        let r = node_radius(*conn_count.get(&n.id).unwrap_or(&0));
                        let d = (n.pos - wp).length();
                        if d < r + 10.0 / self.zoom { Some((n.id, d)) } else { None }
                    })
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(id, _)| id)
            })
        };
        if hovered_id.is_some() { ctx.request_repaint(); }

        // ── Render ────────────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(4, 4, 6)))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                self.canvas = rect.size();

                if !self.initialized {
                    self.initialized = true;
                    self.zoom = 0.65; self.zoom_target = 0.65;
                    if self.nodes.is_empty() {
                        self.pan = egui::vec2(rect.width() * 0.5, rect.height() * 0.5);
                    } else {
                        let cx = self.nodes.iter().map(|n| n.pos.x).sum::<f32>() / self.nodes.len() as f32;
                        let cy = self.nodes.iter().map(|n| n.pos.y).sum::<f32>() / self.nodes.len() as f32;
                        self.pan = egui::vec2(rect.width() * 0.5 - cx * self.zoom, rect.height() * 0.5 - cy * self.zoom);
                    }
                }

                let p = ui.painter();

                if self.nodes.is_empty() {
                    p.text(egui::pos2(rect.center().x, rect.center().y - 10.0),
                        egui::Align2::CENTER_CENTER,
                        "Ctrl+N  new node  ·  Ctrl+I  import vault",
                        egui::FontId::proportional(14.0),
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22));
                }

                // ── Edges ─────────────────────────────────────────────────
                for edge in &self.edges {
                    if in_local && (!local_vis.contains(&edge.source_id) || !local_vis.contains(&edge.target_id)) { continue; }
                    let s = self.nodes.iter().find(|n| n.id == edge.source_id);
                    let t = self.nodes.iter().find(|n| n.id == edge.target_id);
                    if let (Some(s), Some(t)) = (s, t) {
                        let is_root_edge = in_local && (Some(s.id) == self.local_root || Some(t.id) == self.local_root);
                        let alpha = if !in_local { 28 } else if is_root_edge { 80 } else { 35 };
                        p.line_segment(
                            [self.to_screen(s.pos), self.to_screen(t.pos)],
                            egui::Stroke::new(if is_root_edge { 1.0 } else { 0.7 },
                                egui::Color32::from_rgba_unmultiplied(210, 210, 225, alpha)),
                        );
                    }
                }

                // ── Nodes ─────────────────────────────────────────────────
                for node in &self.nodes {
                    if in_local && !local_vis.contains(&node.id) { continue; }
                    let conns = *conn_count.get(&node.id).unwrap_or(&0);
                    let r = node_radius(conns) * self.zoom;
                    let sp = self.to_screen(node.pos);
                    let is_root = in_local && self.local_root == Some(node.id);

                    let alpha: u8 = if !in_local { 210 }
                                    else if is_root { 255 }
                                    else { 190 };

                    p.circle_filled(sp, r, egui::Color32::from_rgba_unmultiplied(225, 224, 228, alpha));

                    // Root pulse ring
                    if is_root {
                        p.circle_stroke(sp, r + 4.0 * self.zoom,
                            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 50)));
                    }

                    // Labels: always in local mode, hover-only in full graph
                    let show_label = in_local || hovered_id == Some(node.id);
                    if show_label {
                        let sz = if in_local {
                            if is_root { 14.0_f32.max(14.0 * self.zoom.sqrt()).min(18.0) }
                            else       { 12.0_f32.max(12.0 * self.zoom.sqrt()).min(14.5) }
                        } else {
                            12.5_f32.max(12.5 * self.zoom.sqrt()).min(16.0)
                        };
                        let label_alpha: u8 = if is_root { 230 } else { 160 };
                        p.text(
                            egui::pos2(sp.x, sp.y - r - 7.0),
                            egui::Align2::CENTER_BOTTOM,
                            &node.title,
                            egui::FontId::proportional(sz),
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, label_alpha),
                        );
                    }
                }

                // ── Search dimming overlay ─────────────────────────────────
                if self.search_open {
                    p.rect_filled(rect, egui::CornerRadius::ZERO,
                        egui::Color32::from_rgba_unmultiplied(4, 4, 6, 180));
                }

                // ── Local mode: back button overlay ───────────────────────
                if in_local {
                    let back_label = if !self.nav_history.is_empty() {
                        let prev_id = *self.nav_history.last().unwrap();
                        let title = self.nodes.iter().find(|n| n.id == prev_id)
                            .map(|n| n.title.as_str()).unwrap_or("back");
                        format!("← {}", title)
                    } else {
                        "← Full graph".to_string()
                    };

                    let back_pos = egui::pos2(rect.left() + 20.0, rect.top() + 20.0);
                    let back_rect = p.text(
                        back_pos,
                        egui::Align2::LEFT_TOP,
                        &back_label,
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_rgba_unmultiplied(120, 118, 135, 160),
                    );

                    // Hit detection for back button click
                    if let Some(ps) = pointer_screen {
                        if back_rect.expand(8.0).contains(ps) {
                            p.text(
                                back_pos,
                                egui::Align2::LEFT_TOP,
                                &back_label,
                                egui::FontId::proportional(12.0),
                                egui::Color32::from_rgba_unmultiplied(200, 198, 215, 200),
                            );
                            if ctx.input(|i| i.pointer.primary_clicked()) {
                                self.local_back();
                                ctx.request_repaint();
                            }
                        }
                    }

                    // Hint: tap root to write
                    p.text(
                        egui::pos2(rect.right() - 20.0, rect.top() + 20.0),
                        egui::Align2::RIGHT_TOP,
                        "tap node again to write",
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 18),
                    );
                }

                // ── Toast ─────────────────────────────────────────────────
                if let Some((ref msg, t)) = self.toast {
                    let alpha = ((t * 100.0).min(255.0) as u8).min(((t - 0.0) * 255.0) as u8);
                    let toast_pos = egui::pos2(rect.center().x, rect.bottom() - 36.0);
                    let font = egui::FontId::proportional(13.0);
                    let galley = p.layout_no_wrap(
                        msg.clone(), font.clone(),
                        egui::Color32::from_rgba_unmultiplied(220, 219, 228, alpha),
                    );
                    let bg_rect = egui::Rect::from_center_size(
                        toast_pos,
                        egui::vec2(galley.size().x + 28.0, 30.0),
                    );
                    p.rect_filled(bg_rect, egui::CornerRadius::same(8),
                        egui::Color32::from_rgba_unmultiplied(18, 17, 24, alpha));
                    p.galley(
                        egui::pos2(bg_rect.center().x - galley.size().x * 0.5,
                                   bg_rect.center().y - galley.size().y * 0.5),
                        galley, egui::Color32::WHITE,
                    );
                }
            });

        // ── Help overlay ──────────────────────────────────────────────────
        if self.help_open {
            egui::Window::new("__help__")
                .title_bar(false)
                .resizable(false)
                .anchor(egui::Align2::LEFT_TOP, [20.0, 52.0])
                .fixed_size([300.0, 0.0])
                .frame(egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(10, 10, 14, 245))
                    .inner_margin(egui::Margin::same(20))
                    .corner_radius(egui::CornerRadius::same(10)))
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new("Bibbo").color(egui::Color32::from_rgb(200, 199, 210)).size(14.0).strong());
                    ui.add_space(10.0);

                    let key_col  = egui::Color32::from_rgb(200, 199, 215);
                    let desc_col = egui::Color32::from_rgb(90, 88, 105);
                    let head_col = egui::Color32::from_rgb(55, 53, 68);

                    let row = |ui: &mut egui::Ui, key: &str, desc: &str| {
                        ui.horizontal(|ui| {
                            ui.add_sized([110.0, 16.0], egui::Label::new(
                                egui::RichText::new(key).color(key_col).size(12.0).monospace()
                            ));
                            ui.label(egui::RichText::new(desc).color(desc_col).size(12.0));
                        });
                        ui.add_space(3.0);
                    };

                    ui.label(egui::RichText::new("WRITING").color(head_col).size(10.0));
                    ui.add_space(4.0);
                    row(ui, "Ctrl + N",   "new node");
                    row(ui, "Esc",        "save & return to graph");
                    row(ui, "[[Title]]",  "link to another node");

                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("GRAPH").color(head_col).size(10.0));
                    ui.add_space(4.0);
                    row(ui, "Click node",       "enter local view");
                    row(ui, "Click node again", "open writing mode");
                    row(ui, "Click neighbor",   "expand web");
                    row(ui, "Click edge",       "see why connected");
                    row(ui, "Scroll",           "zoom in / out");
                    row(ui, "Drag",             "pan canvas");
                    row(ui, "Drag node",        "move node");

                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("NAVIGATE").color(head_col).size(10.0));
                    ui.add_space(4.0);
                    row(ui, "Ctrl + K",  "search nodes");
                    row(ui, "Esc",       "back / close panel");

                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("DATA").color(head_col).size(10.0));
                    ui.add_space(4.0);
                    row(ui, "Ctrl + E",  "export to .md files");
                    row(ui, "Ctrl + I",  "import .md folder");

                    ui.add_space(12.0);
                    ui.add(egui::Separator::default());
                    ui.add_space(6.0);
                    if ui.add(egui::Button::new(
                        egui::RichText::new("Close  Esc")
                            .color(egui::Color32::from_rgb(55, 53, 68))
                            .size(11.0)
                    ).frame(false)).clicked() {
                        self.help_open = false;
                    }
                });
        } else {
            // Persistent "?" hint in top-left when help is closed
            egui::Window::new("__help_hint__")
                .title_bar(false)
                .resizable(false)
                .anchor(egui::Align2::LEFT_TOP, [20.0, 52.0])
                .fixed_size([60.0, 0.0])
                .frame(egui::Frame::new()
                    .fill(egui::Color32::TRANSPARENT))
                .show(ctx, |ui| {
                    if ui.add(egui::Button::new(
                        egui::RichText::new("? help")
                            .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22))
                            .size(11.0)
                    ).frame(false)).clicked() {
                        self.help_open = true;
                    }
                });
        }

        // ── Delete all nodes ─────────────────────────────────────────────
        egui::Window::new("__delete_all__")
            .title_bar(false)
            .resizable(false)
            .anchor(egui::Align2::RIGHT_BOTTOM, [-20.0, -20.0])
            .fixed_size([160.0, 0.0])
            .frame(egui::Frame::new().fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.confirm_delete_all {
                        if ui.add(egui::Button::new(
                            egui::RichText::new("Confirm delete all")
                                .color(egui::Color32::from_rgb(200, 55, 55))
                                .size(11.0)
                        ).frame(false)).clicked() {
                            let _ = self.db.execute("DELETE FROM nodes", []);
                            let _ = self.db.execute("DELETE FROM edges", []);
                            self.nodes.clear();
                            self.edges.clear();
                            self.local_root = None;
                            self.local_visible.clear();
                            self.nav_history.clear();
                            self.selected_edge = None;
                            self.confirm_delete_all = false;
                        }
                        ui.add_space(8.0);
                        if ui.add(egui::Button::new(
                            egui::RichText::new("Cancel")
                                .color(egui::Color32::from_rgb(55, 53, 68))
                                .size(11.0)
                        ).frame(false)).clicked() {
                            self.confirm_delete_all = false;
                        }
                    } else if ui.add(egui::Button::new(
                        egui::RichText::new("Delete all nodes")
                            .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22))
                            .size(11.0)
                    ).frame(false)).clicked() {
                        self.confirm_delete_all = true;
                    }
                });
            });

        // ── Living Connections panel ──────────────────────────────────────
        if let Some((src_id, tgt_id, click_pos)) = self.selected_edge {
            let src = self.nodes.iter().find(|n| n.id == src_id).map(|n| (n.title.clone(), n.body.clone()));
            let tgt = self.nodes.iter().find(|n| n.id == tgt_id).map(|n| (n.title.clone(), n.body.clone()));
            if let (Some((st, sb)), Some((tt, tb))) = (src, tgt) {
                let tmp_src = Node { id: src_id, title: st.clone(), body: sb, color: egui::Color32::WHITE, pos: egui::Pos2::ZERO, vel: egui::Vec2::ZERO, dragging: false, dirty: false };
                let tmp_tgt = Node { id: tgt_id, title: tt.clone(), body: tb, color: egui::Color32::WHITE, pos: egui::Pos2::ZERO, vel: egui::Vec2::ZERO, dragging: false, dirty: false };
                let reasons = connection_reasons(&tmp_src, &tmp_tgt);
                let pw = 300.0_f32;
                let px = (click_pos.x - pw * 0.5).clamp(8.0, self.canvas.x - pw - 8.0);
                let py = (click_pos.y + 18.0).min(self.canvas.y - 160.0);
                egui::Window::new("__living__")
                    .title_bar(false).resizable(false)
                    .fixed_pos([px, py]).fixed_size([pw, 0.0])
                    .frame(egui::Frame::new()
                        .fill(egui::Color32::from_rgb(10, 10, 14))
                        .inner_margin(egui::Margin::same(18))
                        .corner_radius(egui::CornerRadius::same(8)))
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&st).color(egui::Color32::from_rgb(200, 199, 210)).size(13.0));
                            ui.label(egui::RichText::new("  ─  ").color(egui::Color32::from_rgb(55, 54, 62)).size(13.0));
                            ui.label(egui::RichText::new(&tt).color(egui::Color32::from_rgb(200, 199, 210)).size(13.0));
                        });
                        ui.add_space(8.0);
                        if reasons.is_empty() {
                            ui.label(egui::RichText::new("Linked by title reference").color(egui::Color32::from_rgb(85, 83, 95)).size(12.0));
                        } else {
                            ui.label(egui::RichText::new("Connected via").color(egui::Color32::from_rgb(70, 68, 80)).size(11.0));
                            ui.add_space(4.0);
                            for r in &reasons {
                                ui.label(egui::RichText::new(r).color(egui::Color32::from_rgb(160, 162, 180)).size(13.0).monospace());
                            }
                        }
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Esc to close").color(egui::Color32::from_rgb(40, 39, 48)).size(10.0));
                    });
            }
        }

        // ── Search palette ────────────────────────────────────────────────
        if self.search_open {
            let esc   = ctx.input(|i| i.key_pressed(egui::Key::Escape));
            let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
            let up    = ctx.input(|i| i.key_pressed(egui::Key::ArrowUp));
            let down  = ctx.input(|i| i.key_pressed(egui::Key::ArrowDown));

            let results = self.search_results(&self.search_query.clone());
            let count = results.len();
            if esc { self.search_open = false; }
            if up   && self.search_sel > 0         { self.search_sel -= 1; }
            if down && self.search_sel + 1 < count { self.search_sel += 1; }
            if self.search_sel >= count && count > 0 { self.search_sel = count - 1; }

            let mut fly_idx: Option<usize> = None;
            if enter && !results.is_empty() {
                fly_idx = Some(results[self.search_sel]);
                self.search_open = false;
            }

            egui::Window::new("__search__")
                .title_bar(false).resizable(false)
                .anchor(egui::Align2::CENTER_TOP, [0.0, 60.0])
                .fixed_size([500.0, 0.0])
                .frame(egui::Frame::new()
                    .fill(egui::Color32::from_rgb(10, 10, 14))
                    .inner_margin(egui::Margin::same(14))
                    .corner_radius(egui::CornerRadius::same(10)))
                .show(ctx, |ui| {
                    ui.set_min_width(472.0);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Search nodes...")
                            .font(egui::FontId::proportional(16.0))
                            .text_color(egui::Color32::from_rgb(230, 229, 234))
                            .desired_width(f32::INFINITY).frame(false),
                    );
                    resp.request_focus();
                    if resp.changed() { self.search_sel = 0; }

                    if !results.is_empty() {
                        ui.add_space(8.0);
                        ui.separator();
                        for (row, &node_idx) in results.iter().enumerate() {
                            let node = &self.nodes[node_idx];
                            let sel = row == self.search_sel;
                            let bg = if sel { egui::Color32::from_rgba_unmultiplied(255,255,255,12) } else { egui::Color32::TRANSPARENT };
                            let rr = ui.add_sized([472.0, 34.0], egui::Button::new(
                                egui::RichText::new(&node.title)
                                    .font(egui::FontId::proportional(14.0))
                                    .color(if sel { egui::Color32::from_rgb(240,239,245) } else { egui::Color32::from_rgb(150,148,158) })
                            ).fill(bg).frame(false));
                            if rr.clicked() { fly_idx = Some(node_idx); self.search_open = false; }
                            if rr.hovered() { self.search_sel = row; }
                        }
                    } else if !self.search_query.is_empty() {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("No nodes found").color(egui::Color32::from_rgb(65,63,73)).size(13.0));
                    }
                });

            if let Some(idx) = fly_idx {
                let id  = self.nodes[idx].id;
                let pos = self.nodes[idx].pos;
                self.fly_to = Some(pos);
                self.zoom_target = 1.0;
                self.enter_local(id);
            }
        }
    }
}
