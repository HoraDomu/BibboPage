use eframe::egui;
use rusqlite::Connection;

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

// ── Node ─────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
struct Node {
    id: i64,
    title: String,
    color: egui::Color32,
    pos: egui::Pos2,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn spawn_pos(n: usize, canvas: egui::Vec2) -> egui::Pos2 {
    let s = n as u64;
    let ax = s.wrapping_mul(2654435761).wrapping_add(1013904223);
    let ay = ax.wrapping_mul(2654435761).wrapping_add(1013904223);
    let rx = (ax & 0xFFFF) as f32 / 65535.0;
    let ry = (ay & 0xFFFF) as f32 / 65535.0;
    egui::pos2(
        (80.0 + rx * (canvas.x - 160.0)).clamp(60.0, canvas.x - 60.0),
        (80.0 + ry * (canvas.y - 160.0)).clamp(60.0, canvas.y - 60.0),
    )
}

fn date_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
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
    const MON: &[&str] = &[
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];
    format!("{} {d}, {y}", MON[(m as usize).saturating_sub(1)])
}

// ── App ───────────────────────────────────────────────────────────────────────

struct BibboApp {
    db: Connection,
    nodes: Vec<Node>,
    canvas: egui::Vec2,
    modal: bool,
    focus_title: bool,
    draft_title: String,
    draft_body: String,
    color_idx: usize,
}

impl BibboApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut v = egui::Visuals::dark();
        v.panel_fill = egui::Color32::BLACK;
        v.window_fill = egui::Color32::from_rgb(14, 14, 14);
        cc.egui_ctx.set_visuals(v);

        let db = Connection::open("bibbo.db").expect("failed to open bibbo.db");
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS nodes (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                title     TEXT    NOT NULL,
                body      TEXT    NOT NULL,
                color_idx INTEGER NOT NULL,
                pos_x     REAL    NOT NULL,
                pos_y     REAL    NOT NULL,
                created   TEXT    NOT NULL
            );",
        )
        .expect("failed to init db");

        let nodes = load_nodes(&db);
        let color_idx = nodes.len() % COLORS.len();

        Self {
            db,
            nodes,
            canvas: egui::vec2(1200.0, 800.0),
            modal: false,
            focus_title: false,
            draft_title: String::new(),
            draft_body: String::new(),
            color_idx,
        }
    }

    fn commit_node(&mut self) {
        let title = self.draft_title.trim().to_string();
        let body = std::mem::take(&mut self.draft_body);
        let ci = self.color_idx;
        self.color_idx = (ci + 1) % COLORS.len();
        let pos = spawn_pos(self.nodes.len(), self.canvas);

        let _ = self.db.execute(
            "INSERT INTO nodes (title,body,color_idx,pos_x,pos_y,created) VALUES(?1,?2,?3,?4,?5,?6)",
            rusqlite::params![title, body, ci as i64, pos.x as f64, pos.y as f64, date_string()],
        );
        let id = self.db.last_insert_rowid();

        self.nodes.push(Node { id, title, color: COLORS[ci], pos });
        self.modal = false;
    }
}

fn load_nodes(db: &Connection) -> Vec<Node> {
    let Ok(mut s) =
        db.prepare("SELECT id, title, color_idx, pos_x, pos_y FROM nodes ORDER BY id")
    else {
        return vec![];
    };
    let Ok(rows) = s.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)? as usize,
            r.get::<_, f64>(3)? as f32,
            r.get::<_, f64>(4)? as f32,
        ))
    }) else {
        return vec![];
    };
    rows.filter_map(|r| r.ok())
        .map(|(id, title, ci, x, y)| Node {
            id,
            title,
            color: COLORS[ci % COLORS.len()],
            pos: egui::pos2(x, y),
        })
        .collect()
}

// ── Frame loop ────────────────────────────────────────────────────────────────

impl eframe::App for BibboApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Ctrl+N opens modal
        if ctx.input(|i| i.key_pressed(egui::Key::N) && i.modifiers.ctrl) && !self.modal {
            self.modal = true;
            self.focus_title = true;
            self.draft_title.clear();
            self.draft_body.clear();
        }

        // ── Canvas ──
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::BLACK))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                self.canvas = rect.size();
                let p = ui.painter();

                if self.nodes.is_empty() && !self.modal {
                    p.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Press Ctrl+N for a new node",
                        egui::FontId::proportional(16.0),
                        egui::Color32::from_rgba_unmultiplied(180, 180, 180, 80),
                    );
                }

                for n in &self.nodes {
                    let pos = n.pos;
                    let r = 9.0_f32;

                    // subtle outer glow
                    p.circle_filled(
                        pos,
                        r + 3.0,
                        egui::Color32::from_rgba_unmultiplied(
                            n.color.r(), n.color.g(), n.color.b(), 30,
                        ),
                    );
                    // filled node
                    p.circle_filled(pos, r, n.color);
                    // crisp inner highlight ring
                    p.circle_stroke(
                        pos,
                        r,
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 50)),
                    );
                    // label
                    p.text(
                        egui::pos2(pos.x, pos.y + r + 7.0),
                        egui::Align2::CENTER_TOP,
                        &n.title,
                        egui::FontId::proportional(11.5),
                        egui::Color32::from_rgb(190, 190, 190),
                    );
                }

                // dim overlay when modal is open
                if self.modal {
                    p.rect_filled(
                        rect,
                        egui::CornerRadius::ZERO,
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180),
                    );
                }
            });

        // ── Modal ──
        if self.modal {
            let mut save = false;
            let mut cancel = false;

            egui::Window::new("__bibbo_node__")
                .title_bar(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .fixed_size([520.0, 360.0])
                .frame(
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(14, 14, 14))
                        .inner_margin(egui::Margin::same(28))
                        .corner_radius(egui::CornerRadius::same(10)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(date_string())
                                .color(egui::Color32::from_rgb(70, 70, 70))
                                .size(12.0),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let wc = self.draft_body.split_whitespace().count();
                            ui.label(
                                egui::RichText::new(format!("{wc} words"))
                                    .color(egui::Color32::from_rgb(70, 70, 70))
                                    .size(12.0),
                            );
                        });
                    });

                    ui.add_space(12.0);

                    let title_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.draft_title)
                            .hint_text("Title")
                            .font(egui::FontId::proportional(22.0))
                            .text_color(egui::Color32::from_rgb(240, 240, 240))
                            .desired_width(f32::INFINITY)
                            .frame(false),
                    );
                    if self.focus_title {
                        title_resp.request_focus();
                        self.focus_title = false;
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(10.0);

                    ui.add(
                        egui::TextEdit::multiline(&mut self.draft_body)
                            .hint_text("Start writing...")
                            .font(egui::FontId::proportional(15.0))
                            .text_color(egui::Color32::from_rgb(200, 200, 200))
                            .desired_width(f32::INFINITY)
                            .desired_rows(9)
                            .frame(false),
                    );

                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Ctrl+Enter  save  ·  Esc  cancel")
                            .color(egui::Color32::from_rgb(50, 50, 50))
                            .size(11.0),
                    );

                    if ctx.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl) {
                        save = true;
                    }
                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        cancel = true;
                    }
                });

            if save && !self.draft_title.trim().is_empty() {
                self.commit_node();
            }
            if cancel {
                self.modal = false;
            }
        }
    }
}
