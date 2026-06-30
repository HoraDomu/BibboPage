package com.bibbo;

import com.bibbo.db.Database;
import com.bibbo.model.Edge;
import com.bibbo.model.Node;
import com.bibbo.util.QuadTree;
import com.bibbo.util.Utils;
import javafx.animation.AnimationTimer;
import javafx.application.Platform;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.geometry.VPos;
import javafx.scene.Cursor;
import javafx.scene.Scene;
import javafx.scene.canvas.Canvas;
import javafx.scene.canvas.GraphicsContext;
import javafx.scene.control.Label;
import javafx.scene.control.Separator;
import javafx.scene.control.TextArea;
import javafx.scene.control.TextField;
import javafx.scene.input.KeyCode;
import javafx.scene.input.KeyEvent;
import javafx.scene.input.MouseButton;
import javafx.scene.layout.*;
import javafx.scene.paint.Color;
import javafx.scene.text.Font;
import javafx.scene.text.TextAlignment;
import javafx.stage.DirectoryChooser;
import javafx.stage.Stage;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.SQLException;
import java.util.*;
import java.util.stream.Collectors;

public class BibboApp {

    // ── Physics constants ─────────────────────────────────────────────────────
    private static final double MIN_DIST    = 110.0;
    private static final double REPULSION   = 2200.0;
    private static final double DAMPING     = 5.5;
    private static final double SPRING_K    = 1.8;
    private static final double SPRING_DEAD = 3.0;
    private static final double STOP_VEL    = 1.5;

    private static final Color[] COLORS = {
        Color.rgb(255, 107, 107), Color.rgb(255, 159, 67),
        Color.rgb(255, 206, 84),  Color.rgb(46,  213, 115),
        Color.rgb(30,  144, 255), Color.rgb(147, 51,  234),
        Color.rgb(236, 72,  153), Color.rgb(20,  184, 166),
    };

    // ── Core ──────────────────────────────────────────────────────────────────
    private final Stage stage;
    private Database db;
    private final List<Node> nodes = new ArrayList<>();
    private final List<Edge> edges = new ArrayList<>();
    private final Map<Long, Node>    nodeById    = new HashMap<>();
    private final Map<String, Long>  titleIndex  = new HashMap<>();
    private final Map<String, Set<Long>> tagRefs = new HashMap<>();
    // Cached per-frame data — rebuilt only when the underlying list changes
    private final Map<Long, Integer> connCounts  = new HashMap<>();
    private final Map<Long, Integer> physicsIdx  = new HashMap<>();
    private boolean connCountsDirty = true;

    // ── Camera ────────────────────────────────────────────────────────────────
    private double panX, panY, panVelX, panVelY;
    private double zoom = 1.0, zoomTarget = 1.0;
    private double zoomAnchorX, zoomAnchorY;
    private boolean isPanning;
    private double pressOriginX, pressOriginY;
    private boolean initialized;
    private boolean hasFlyTo;
    private double flyToX, flyToY;

    // ── Local graph ───────────────────────────────────────────────────────────
    private Long localRoot = null;
    private final Set<Long> localVisible = new HashSet<>();
    private final Deque<Long> navHistory = new ArrayDeque<>();

    // ── Writing mode ──────────────────────────────────────────────────────────
    private boolean writing;
    private boolean writingNew;
    private boolean writingFocusTitle;
    private Long editingId = null;
    private boolean writingConfirmDelete;

    // ── Living Connections ────────────────────────────────────────────────────
    private Long selectedEdgeSrc = null;
    private Long selectedEdgeTgt = null;
    private double selectedEdgeClickX, selectedEdgeClickY;

    // ── Search ────────────────────────────────────────────────────────────────
    private boolean searchOpen;
    private int searchSel;
    private final List<Integer> currentSearchResults = new ArrayList<>();

    // ── Physics sleep ─────────────────────────────────────────────────────────
    private boolean physicsAwake = true;

    // ── Misc ──────────────────────────────────────────────────────────────────
    private int colorIdx;
    private String toastMsg;
    private double toastTimer;
    private boolean helpOpen;
    private boolean confirmDeleteAll;
    private double helpPanelBottom; // y of bottom of help panel for hit detection

    // ── Canvas / rendering ────────────────────────────────────────────────────
    private final Canvas canvas = new Canvas();
    private final GraphicsContext gc = canvas.getGraphicsContext2D();
    private StackPane root;

    // ── Writing overlay ───────────────────────────────────────────────────────
    private Pane writingOverlay;
    private Label writingBackBtn;
    private TextField titleField;
    private TextArea bodyArea;
    private Label wordCountLabel;
    private HBox deleteRow;
    private Label deleteBtn, confirmDeleteBtn, cancelDeleteBtn;

    // ── Search overlay ────────────────────────────────────────────────────────
    private Pane searchOverlay;
    private TextField searchField;
    private VBox searchResultsBox;

    // ── Input state (set by event handlers, consumed by update loop) ──────────
    private double pointerX, pointerY;
    private double mouseDeltaX, mouseDeltaY;
    private double scrollAccum;
    private boolean primaryDown;
    private boolean primaryPressedThisFrame;
    private boolean primaryReleasedThisFrame;

    // ── Key state (one-shot, cleared after processing) ────────────────────────
    private boolean keyCtrlN, keyCtrlK, keyCtrlE, keyCtrlI, keyCtrlH;
    private boolean keyEscape, keyEnter, keyUp, keyDown;

    // ── Loop ──────────────────────────────────────────────────────────────────
    private long lastFrameNanos = -1L;
    private AnimationTimer timer;

    // ─────────────────────────────────────────────────────────────────────────

    public BibboApp(Stage stage) {
        this.stage = stage;
        try {
            Path dir = Utils.dataDir();
            Files.createDirectories(dir);
            db = new Database(dir.resolve("bibbo.db"));
            nodes.addAll(db.loadNodes());
            edges.addAll(db.loadEdges());
        } catch (IOException | SQLException e) {
            throw new RuntimeException("Failed to open database", e);
        }
        for (int i = 0; i < nodes.size(); i++) {
            Node n = nodes.get(i);
            nodeById.put(n.id, n);
            titleIndex.put(Utils.normalize(n.title), n.id);
            for (String tag : Utils.parseLinks(n.body))
                tagRefs.computeIfAbsent(Utils.normalize(tag), k -> new HashSet<>()).add(n.id);
            physicsIdx.put(n.id, i);
        }
        rebuildConnCounts();
        // Large graphs are already laid out — skip physics until user drags
        if (nodes.size() > 2000) physicsAwake = false;
        colorIdx = nodes.size() % COLORS.length;
    }

    // ── Public entry point ────────────────────────────────────────────────────

    public void show() {
        buildWritingOverlay();
        buildSearchOverlay();

        root = new StackPane(canvas, searchOverlay, writingOverlay);
        root.setStyle("-fx-background-color: rgb(4,4,6);");

        canvas.widthProperty().bind(root.widthProperty());
        canvas.heightProperty().bind(root.heightProperty());

        Scene scene = new Scene(root, 1200, 800);
        scene.setFill(Color.rgb(4, 4, 6));
        scene.getStylesheets().add(getClass().getResource("app.css").toExternalForm());
        scene.setOnKeyPressed(this::handleKey);

        stage.setTitle("Bibbo");
        stage.setMinWidth(800);
        stage.setMinHeight(600);
        stage.setOnHidden(e -> onClose());
        stage.setScene(scene);

        setupInputHandlers();

        stage.show();
        canvas.requestFocus();
        startLoop();
    }

    // ── Writing overlay ───────────────────────────────────────────────────────

    private void buildWritingOverlay() {
        writingOverlay = new Pane();
        writingOverlay.setStyle("-fx-background-color: rgb(6,6,8);");
        writingOverlay.setVisible(false);

        writingBackBtn = new Label("← graph");
        writingBackBtn.setStyle("""
            -fx-text-fill: rgb(50,48,60);
            -fx-font-size: 12;
            -fx-cursor: hand;
            """);
        writingBackBtn.setLayoutX(24);
        writingBackBtn.setLayoutY(20);
        writingBackBtn.setOnMouseClicked(e -> commitWriting());

        titleField = new TextField();
        titleField.setPromptText("Title");
        titleField.setStyle("""
            -fx-background-color: transparent;
            -fx-control-inner-background: rgb(6,6,8);
            -fx-text-fill: rgb(238,237,243);
            -fx-font-size: 30;
            -fx-border-color: transparent;
            -fx-focus-color: transparent;
            -fx-faint-focus-color: transparent;
            -fx-padding: 0;
            """);

        Separator sep = new Separator();
        sep.setStyle("-fx-background-color: rgb(30,29,38);");

        bodyArea = new TextArea();
        bodyArea.getStyleClass().add("writing-area");
        bodyArea.setPromptText("Start writing...  use [[Node Title]] to link ideas");
        bodyArea.setWrapText(true);
        bodyArea.setPrefRowCount(20);
        bodyArea.setStyle("""
            -fx-background-color: transparent;
            -fx-control-inner-background: rgb(6,6,8);
            -fx-text-fill: rgb(188,186,200);
            -fx-font-size: 16;
            -fx-border-color: transparent;
            -fx-focus-color: transparent;
            -fx-faint-focus-color: transparent;
            -fx-padding: 0;
            """);
        VBox.setVgrow(bodyArea, Priority.ALWAYS);

        wordCountLabel = new Label("0 words  ·  Esc to return");
        wordCountLabel.setStyle("-fx-text-fill: rgb(38,36,48); -fx-font-size: 11;");

        deleteBtn = new Label("Delete");
        deleteBtn.setStyle("-fx-text-fill: rgb(70,50,50); -fx-font-size: 11; -fx-cursor: hand;");

        confirmDeleteBtn = new Label("Confirm delete");
        confirmDeleteBtn.setStyle("-fx-text-fill: rgb(210,60,60); -fx-font-size: 11; -fx-cursor: hand;");
        confirmDeleteBtn.setVisible(false);

        cancelDeleteBtn = new Label("Cancel");
        cancelDeleteBtn.setStyle("-fx-text-fill: rgb(55,53,65); -fx-font-size: 11; -fx-cursor: hand;");
        cancelDeleteBtn.setVisible(false);

        deleteBtn.setOnMouseClicked(e -> {
            writingConfirmDelete = true;
            deleteBtn.setVisible(false);
            confirmDeleteBtn.setVisible(true);
            cancelDeleteBtn.setVisible(true);
        });
        confirmDeleteBtn.setOnMouseClicked(e -> performDelete());
        cancelDeleteBtn.setOnMouseClicked(e -> {
            writingConfirmDelete = false;
            deleteBtn.setVisible(true);
            confirmDeleteBtn.setVisible(false);
            cancelDeleteBtn.setVisible(false);
        });

        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);
        deleteRow = new HBox(6, spacer, cancelDeleteBtn, confirmDeleteBtn, deleteBtn);
        deleteRow.setAlignment(Pos.CENTER_RIGHT);

        HBox statusRow = new HBox(wordCountLabel, deleteRow);
        statusRow.setAlignment(Pos.CENTER_LEFT);

        VBox content = new VBox(14, titleField, sep, bodyArea, statusRow);
        content.setMaxWidth(680);
        content.setPadding(new Insets(48, 0, 0, 0));

        HBox centering = new HBox(content);
        centering.setAlignment(Pos.TOP_CENTER);
        centering.layoutXProperty().set(0);
        centering.layoutYProperty().set(0);

        writingOverlay.widthProperty().addListener((obs, ov, nv) -> {
            centering.setPrefWidth(nv.doubleValue());
        });
        writingOverlay.heightProperty().addListener((obs, ov, nv) -> {
            centering.setPrefHeight(nv.doubleValue());
        });

        writingOverlay.getChildren().addAll(writingBackBtn, centering);

        bodyArea.textProperty().addListener((obs, ov, nv) -> {
            long words = Arrays.stream(nv.split("\\s+"))
                               .filter(w -> !w.isEmpty()).count();
            wordCountLabel.setText(words + " words  ·  Esc to return");
        });
    }

    // ── Search overlay ────────────────────────────────────────────────────────

    private void buildSearchOverlay() {
        searchField = new TextField();
        searchField.setPromptText("Search nodes...");
        searchField.setStyle("""
            -fx-background-color: transparent;
            -fx-text-fill: rgb(230,229,234);
            -fx-font-size: 16;
            -fx-border-color: transparent;
            -fx-focus-color: transparent;
            -fx-faint-focus-color: transparent;
            -fx-min-width: 472;
            """);

        Separator sep = new Separator();
        sep.setStyle("-fx-background-color: rgb(35,34,45);");

        searchResultsBox = new VBox(0);

        VBox panel = new VBox(8, searchField, sep, searchResultsBox);
        panel.setStyle("""
            -fx-background-color: rgb(10,10,14);
            -fx-background-radius: 10;
            -fx-padding: 14;
            """);
        panel.setMaxWidth(500);

        searchOverlay = new Pane();
        searchOverlay.setPickOnBounds(false);
        searchOverlay.getChildren().add(panel);

        searchOverlay.widthProperty().addListener((obs, ov, nv) -> {
            double pw = Math.min(500, nv.doubleValue() - 40);
            panel.setMaxWidth(pw);
            panel.setLayoutX((nv.doubleValue() - pw) / 2.0);
            panel.setLayoutY(60);
        });

        searchField.textProperty().addListener((obs, ov, nv) -> {
            searchSel = 0;
            updateSearchResults();
        });

        searchField.setOnKeyPressed(e -> {
            switch (e.getCode()) {
                case ESCAPE -> { keyEscape = true; e.consume(); }
                case ENTER  -> { keyEnter  = true; e.consume(); }
                case UP     -> { keyUp     = true; e.consume(); }
                case DOWN   -> { keyDown   = true; e.consume(); }
                default     -> {}
            }
        });

        searchOverlay.setVisible(false);
    }

    // ── Input handlers ────────────────────────────────────────────────────────

    private void setupInputHandlers() {
        canvas.setFocusTraversable(true);

        canvas.setOnMousePressed(e -> {
            if (e.getButton() == MouseButton.PRIMARY) {
                primaryDown = true;
                primaryPressedThisFrame = true;
                pressOriginX = e.getX();
                pressOriginY = e.getY();
                pointerX = e.getX();
                pointerY = e.getY();
                mouseDeltaX = 0;
                mouseDeltaY = 0;
                canvas.requestFocus();
            }
        });

        canvas.setOnMouseReleased(e -> {
            if (e.getButton() == MouseButton.PRIMARY) {
                primaryDown = false;
                primaryReleasedThisFrame = true;
            }
        });

        canvas.setOnMouseMoved(e -> {
            double ox = pointerX, oy = pointerY;
            pointerX = e.getX();
            pointerY = e.getY();
            mouseDeltaX += pointerX - ox;
            mouseDeltaY += pointerY - oy;
        });

        canvas.setOnMouseDragged(e -> {
            double ox = pointerX, oy = pointerY;
            pointerX = e.getX();
            pointerY = e.getY();
            mouseDeltaX += pointerX - ox;
            mouseDeltaY += pointerY - oy;
        });

        canvas.setOnScroll(e -> {
            // getDeltaY() is in pixels; ~40px per notch. Normalize to notches.
            scrollAccum += e.getDeltaY() / 40.0;
            zoomAnchorX = e.getX();
            zoomAnchorY = e.getY();
        });
    }

    private void handleKey(KeyEvent e) {
        if (writing) {
            if (e.getCode() == KeyCode.ESCAPE) keyEscape = true;
            return;
        }
        if (searchOpen) return; // search field handles its own keys
        if (e.isControlDown()) {
            switch (e.getCode()) {
                case N -> keyCtrlN = true;
                case K -> keyCtrlK = true;
                case E -> keyCtrlE = true;
                case I -> keyCtrlI = true;
                case H -> keyCtrlH = true;
                default -> {}
            }
        } else if (e.getCode() == KeyCode.ESCAPE) {
            keyEscape = true;
        }
    }

    // ── Camera helpers ────────────────────────────────────────────────────────

    private double toScreenX(double wx) { return wx * zoom + panX; }
    private double toScreenY(double wy) { return wy * zoom + panY; }
    private double toWorldX(double sx)  { return (sx - panX) / zoom; }
    private double toWorldY(double sy)  { return (sy - panY) / zoom; }
    private double viewCenterX()        { return toWorldX(canvas.getWidth()  * 0.5); }
    private double viewCenterY()        { return toWorldY(canvas.getHeight() * 0.5); }

    // ── Local graph ───────────────────────────────────────────────────────────

    private List<Long> computeNeighbors(long id) {
        List<Long> result = new ArrayList<>();
        for (Edge e : edges) {
            if (e.sourceId == id) result.add(e.targetId);
            else if (e.targetId == id) result.add(e.sourceId);
        }
        return result;
    }

    private void enterLocal(long id) {
        localRoot = id;
        localVisible.clear();
        localVisible.add(id);
        localVisible.addAll(computeNeighbors(id));
        navHistory.clear();
        nodes.stream().filter(n -> n.id == id).findFirst().ifPresent(n -> {
            hasFlyTo = true; flyToX = n.x; flyToY = n.y;
        });
        applyBloom(id);
    }

    private void expandLocal(long newRoot) {
        if (localRoot != null) navHistory.push(localRoot);
        localRoot = newRoot;
        localVisible.add(newRoot);
        localVisible.addAll(computeNeighbors(newRoot));
        applyBloom(newRoot);
        nodes.stream().filter(n -> n.id == newRoot).findFirst().ifPresent(n -> {
            hasFlyTo = true; flyToX = n.x; flyToY = n.y;
        });
    }

    private void localBack() {
        if (!navHistory.isEmpty()) {
            long prev = navHistory.pop();
            localRoot = prev;
            nodes.stream().filter(n -> n.id == prev).findFirst().ifPresent(n -> {
                hasFlyTo = true; flyToX = n.x; flyToY = n.y;
            });
        } else {
            localRoot = null;
            localVisible.clear();
        }
    }

    private void applyBloom(long centerId) {
        nodes.stream().filter(n -> n.id == centerId).findFirst().ifPresent(center -> {
            List<Long> neighbors = computeNeighbors(centerId);
            for (Node n : nodes) {
                if (!neighbors.contains(n.id)) continue;
                double dx = n.x - center.x, dy = n.y - center.y;
                double len = Math.sqrt(dx * dx + dy * dy);
                if (len > 0.1) {
                    n.vx += (dx / len) * 60.0;
                    n.vy += (dy / len) * 60.0;
                }
            }
        });
    }

    // ── Writing mode ──────────────────────────────────────────────────────────

    private void openWritingNew() {
        titleField.clear();
        bodyArea.clear();
        editingId = null;
        writing = true;
        writingNew = true;
        writingFocusTitle = true;
        writingConfirmDelete = false;
        deleteRow.setVisible(false);
        resetDeleteButtons();
        updateWritingBackBtn();
        showWriting(true);
    }

    private void openWritingEdit(long id) {
        nodes.stream().filter(n -> n.id == id).findFirst().ifPresent(n -> {
            titleField.setText(n.title);
            bodyArea.setText(n.body);
        });
        editingId = id;
        writing = true;
        writingNew = false;
        writingFocusTitle = false;
        writingConfirmDelete = false;
        deleteRow.setVisible(true);
        resetDeleteButtons();
        updateWritingBackBtn();
        showWriting(true);
    }

    private void updateWritingBackBtn() {
        if (editingId != null) {
            String t = titleField.getText().trim();
            writingBackBtn.setText("← " + (t.isEmpty() ? "…" : t));
        } else {
            writingBackBtn.setText("← graph");
        }
    }

    private void resetDeleteButtons() {
        deleteBtn.setVisible(true);
        confirmDeleteBtn.setVisible(false);
        cancelDeleteBtn.setVisible(false);
    }

    private void performDelete() {
        if (editingId == null) return;
        long id = editingId;
        try {
            db.deleteNode(id);
        } catch (SQLException ex) {
            ex.printStackTrace();
        }
        Node removed = nodeById.remove(id);
        if (removed != null) {
            titleIndex.remove(Utils.normalize(removed.title));
            for (String tag : Utils.parseLinks(removed.body))
                tagRefs.getOrDefault(Utils.normalize(tag), Collections.emptySet()).remove(id);
        }
        nodes.removeIf(n -> n.id == id);
        edges.removeIf(e -> e.sourceId == id || e.targetId == id);
        connCountsDirty = true;
        rebuildPhysicsIdx();
        if (Objects.equals(localRoot, id)) {
            localRoot = null;
            localVisible.clear();
            navHistory.clear();
        }
        editingId = null;
        writing = false;
        showWriting(false);
        writingConfirmDelete = false;
    }

    private void commitWriting() {
        String title = titleField.getText().trim();
        String body  = bodyArea.getText();

        if (editingId != null) {
            long id = editingId;
            if (!title.isEmpty()) {
                try {
                    db.updateNode(id, title, body);
                } catch (SQLException ex) { ex.printStackTrace(); }
                Node updNode = nodeById.get(id);
                if (updNode != null) {
                    String oldTitleNorm = Utils.normalize(updNode.title);
                    for (String tag : Utils.parseLinks(updNode.body))
                        tagRefs.getOrDefault(Utils.normalize(tag), Collections.emptySet()).remove(id);
                    titleIndex.remove(oldTitleNorm);
                    updNode.title = title;
                    updNode.body  = body;
                    String newTitleNorm = Utils.normalize(title);
                    titleIndex.put(newTitleNorm, id);
                    for (String tag : Utils.parseLinks(body))
                        tagRefs.computeIfAbsent(Utils.normalize(tag), k -> new HashSet<>()).add(id);
                    rebuildAffectedEdges(id, oldTitleNorm, newTitleNorm);
                }
            }
            editingId = null;
            writing = false;
            showWriting(false);
            enterLocal(id);
        } else if (!title.isEmpty()) {
            double[] pos = Utils.spawnPos(nodes.size(), viewCenterX(), viewCenterY());
            long id;
            try {
                id = db.insertNode(title, body, colorIdx, pos[0], pos[1], Utils.dateString());
            } catch (SQLException ex) {
                ex.printStackTrace();
                writing = false;
                showWriting(false);
                return;
            }
            int ci = colorIdx;
            colorIdx = (colorIdx + 1) % COLORS.length;
            long h = (id * 6364136223846793005L);
            double kx = (h & 0xFFL) / 255.0 * 120.0 - 60.0;
            double ky = ((h >> 8) & 0xFFL) / 255.0 * 120.0 - 60.0;
            Node newNode = new Node(id, title, body, COLORS[ci], pos[0], pos[1]);
            newNode.vx = kx;
            newNode.vy = ky;
            nodes.add(newNode);
            nodeById.put(id, newNode);
            physicsIdx.put(id, nodes.size() - 1);
            titleIndex.put(Utils.normalize(title), id);
            for (String tag : Utils.parseLinks(body))
                tagRefs.computeIfAbsent(Utils.normalize(tag), k -> new HashSet<>()).add(id);
            physicsAwake = true;
            rebuildEdgesFor(id, body);
            writing = false;
            showWriting(false);
            enterLocal(id);
        } else {
            writing = false;
            showWriting(false);
        }
    }

    private void showWriting(boolean visible) {
        writingOverlay.setVisible(visible);
        if (visible) {
            if (writingFocusTitle) {
                Platform.runLater(() -> {
                    titleField.requestFocus();
                    titleField.end();
                });
                writingFocusTitle = false;
            } else {
                Platform.runLater(() -> bodyArea.requestFocus());
            }
        } else {
            Platform.runLater(() -> canvas.requestFocus());
        }
    }

    // ── Cached index helpers ──────────────────────────────────────────────────

    private void rebuildConnCounts() {
        connCounts.clear();
        for (Edge e : edges) {
            connCounts.merge(e.sourceId, 1, Integer::sum);
            connCounts.merge(e.targetId, 1, Integer::sum);
        }
        connCountsDirty = false;
    }

    private void rebuildPhysicsIdx() {
        physicsIdx.clear();
        for (int i = 0; i < nodes.size(); i++) physicsIdx.put(nodes.get(i).id, i);
    }

    // ── Edge building ─────────────────────────────────────────────────────────

    private void rebuildEdgesFor(long nodeId, String body) {
        try { db.deleteEdgesFrom(nodeId); } catch (SQLException ex) { ex.printStackTrace(); }
        edges.removeIf(e -> e.sourceId == nodeId);

        Node thisNode = nodeById.get(nodeId);
        if (thisNode == null) return;
        String myTitleNorm = Utils.normalize(thisNode.title);
        List<String> myTags = Utils.parseLinks(body);
        Set<Long> connected = new HashSet<>();

        // Forward: my tag matches their title, or we share a common tag
        for (String tag : myTags) {
            String tn = Utils.normalize(tag);
            Long tid = titleIndex.get(tn);
            if (tid != null && tid != nodeId) connected.add(tid);
            for (Long sid : tagRefs.getOrDefault(tn, Collections.emptySet())) {
                if (sid != nodeId) connected.add(sid);
            }
        }

        // Reverse: their tag references my title
        if (!myTitleNorm.isEmpty()) {
            for (Long rid : tagRefs.getOrDefault(myTitleNorm, Collections.emptySet())) {
                if (rid != nodeId) connected.add(rid);
            }
        }

        for (Long targetId : connected) {
            try { db.insertEdge(nodeId, targetId); } catch (SQLException ex) { ex.printStackTrace(); }
            edges.add(new Edge(nodeId, targetId));
        }
        connCountsDirty = true;
    }

    private void rebuildAffectedEdges(long changedId, String oldTitleNorm, String newTitleNorm) {
        Set<Long> affected = new HashSet<>();
        affected.add(changedId);
        affected.addAll(tagRefs.getOrDefault(oldTitleNorm, Collections.emptySet()));
        affected.addAll(tagRefs.getOrDefault(newTitleNorm, Collections.emptySet()));
        for (Long affId : new HashSet<>(affected)) {
            Node n = nodeById.get(affId);
            if (n != null) rebuildEdgesFor(affId, n.body);
        }
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    private void savePosition(long id, double x, double y) {
        try { db.updatePosition(id, x, y); } catch (SQLException ex) { ex.printStackTrace(); }
    }

    private void saveAllPositions() {
        // BUG FIX: save every node on close, not just ones that already settled
        for (Node n : nodes) {
            try { db.updatePosition(n.id, n.x, n.y); } catch (SQLException ex) { ex.printStackTrace(); }
        }
    }

    private void onClose() {
        saveAllPositions();
        db.close();
    }

    // ── Import / Export ───────────────────────────────────────────────────────

    private void exportVault() {
        DirectoryChooser dc = new DirectoryChooser();
        dc.setTitle("Export to folder");
        File dir = dc.showDialog(stage);
        if (dir == null) return;
        int count = 0;
        for (Node n : nodes) {
            String safe = n.title.chars()
                .mapToObj(c -> Character.isLetterOrDigit(c) || c == ' ' || c == '-' || c == '_'
                    ? String.valueOf((char)c) : "_")
                .collect(Collectors.joining());
            Path path = dir.toPath().resolve(safe + ".md");
            String content = n.body.isBlank()
                ? "# " + n.title + "\n"
                : "# " + n.title + "\n\n" + n.body + "\n";
            try {
                Files.writeString(path, content);
                count++;
            } catch (IOException ignored) {}
        }
        toastMsg = "Exported " + count + " notes";
        toastTimer = 3.0;
    }

    private void importVault() {
        DirectoryChooser dc = new DirectoryChooser();
        dc.setTitle("Import from folder");
        File dir = dc.showDialog(stage);
        if (dir == null) return;

        Set<String> existingTitles = nodes.stream()
            .map(n -> Utils.normalize(n.title))
            .collect(Collectors.toSet());

        File[] files = dir.listFiles(f -> f.getName().endsWith(".md"));
        if (files == null) {
            toastMsg = "Could not read folder";
            toastTimer = 3.0;
            return;
        }

        int imported = 0;
        List<Long> newIds = new ArrayList<>();
        for (File f : files) {
            String content;
            try { content = Files.readString(f.toPath()); }
            catch (IOException e) { continue; }

            String[] parsed = Utils.parseMdFile(content, f.getName());
            String title = parsed[0], body = parsed[1];
            if (title.isEmpty()) continue;
            if (existingTitles.contains(Utils.normalize(title))) continue;

            int ci = colorIdx;
            colorIdx = (colorIdx + 1) % COLORS.length;
            double[] pos = Utils.spawnPos(nodes.size(), 0, 0);
            long id;
            try {
                id = db.insertNode(title, body, ci, pos[0], pos[1], Utils.dateString());
            } catch (SQLException e) { continue; }

            Node imp = new Node(id, title, body, COLORS[ci], pos[0], pos[1]);
            nodes.add(imp);
            nodeById.put(id, imp);
            physicsIdx.put(id, nodes.size() - 1);
            titleIndex.put(Utils.normalize(title), id);
            for (String tag : Utils.parseLinks(body))
                tagRefs.computeIfAbsent(Utils.normalize(tag), k -> new HashSet<>()).add(id);
            physicsAwake = true;
            newIds.add(id);
            existingTitles.add(Utils.normalize(title));
            imported++;
        }

        for (long id : newIds) {
            String body = nodes.stream().filter(n -> n.id == id)
                .findFirst().map(n -> n.body).orElse("");
            rebuildEdgesFor(id, body);
        }

        toastMsg = "Imported " + imported + " notes";
        toastTimer = 3.5;
    }

    // ── Search ────────────────────────────────────────────────────────────────

    private List<Integer> searchResults(String query) {
        if (query.isEmpty()) {
            List<Integer> idxs = new ArrayList<>();
            for (int i = 0; i < nodes.size(); i++) idxs.add(i);
            idxs.sort((a, b) -> Long.compare(nodes.get(b).id, nodes.get(a).id));
            if (idxs.size() > 8) idxs = idxs.subList(0, 8);
            return idxs;
        }
        // FTS5 path — searches title + body via SQLite index, scales to 100k+ nodes
        try {
            List<Long> ftsIds = db.searchFTS(query);
            if (!ftsIds.isEmpty()) {
                List<Integer> result = new ArrayList<>();
                for (Long fid : ftsIds) {
                    Node n = nodeById.get(fid);
                    if (n != null) {
                        int idx = nodes.indexOf(n);
                        if (idx >= 0) result.add(idx);
                    }
                    if (result.size() >= 8) break;
                }
                if (!result.isEmpty()) return result;
            }
        } catch (SQLException ignored) {}
        // Fallback: title-only linear scan (no body scan — safe at any node count)
        String q = query.toLowerCase();
        List<int[]> scored = new ArrayList<>();
        for (int i = 0; i < nodes.size(); i++) {
            Node n = nodes.get(i);
            String t = n.title.toLowerCase();
            int score = t.startsWith(q) ? 3 : t.contains(q) ? 2 : 0;
            if (score > 0) scored.add(new int[]{i, score});
        }
        scored.sort((a, b) -> Integer.compare(b[1], a[1]));
        if (scored.size() > 8) scored = scored.subList(0, 8);
        return scored.stream().map(a -> a[0]).collect(Collectors.toList());
    }

    private void updateSearchResults() {
        currentSearchResults.clear();
        currentSearchResults.addAll(searchResults(searchField.getText()));
        rebuildSearchUI();
    }

    private void rebuildSearchUI() {
        searchResultsBox.getChildren().clear();
        for (int row = 0; row < currentSearchResults.size(); row++) {
            int nodeIdx = currentSearchResults.get(row);
            Node n = nodes.get(nodeIdx);
            boolean sel = (row == searchSel);
            Label lbl = new Label(n.title);
            lbl.setMaxWidth(Double.MAX_VALUE);
            lbl.setPadding(new Insets(8, 8, 8, 8));
            lbl.setStyle("-fx-font-size: 14; -fx-cursor: hand; -fx-text-fill: "
                + (sel ? "rgb(240,239,245)" : "rgb(150,148,158)") + ";"
                + (sel ? "-fx-background-color: rgba(255,255,255,0.05); -fx-background-radius: 4;" : ""));
            lbl.setPrefWidth(472);
            final int fi = nodeIdx;
            final int capturedRow = row;
            lbl.setOnMouseClicked(e -> flyToNode(fi));
            lbl.setOnMouseEntered(e -> searchSel = capturedRow);
            searchResultsBox.getChildren().add(lbl);
        }
    }

    private void flyToNode(int nodeIdx) {
        if (nodeIdx < 0 || nodeIdx >= nodes.size()) return;
        Node n = nodes.get(nodeIdx);
        hasFlyTo = true; flyToX = n.x; flyToY = n.y;
        zoomTarget = 1.0;
        enterLocal(n.id);
        searchOpen = false;
        searchOverlay.setVisible(false);
        searchField.clear();
        canvas.requestFocus();
    }

    // ── Animation loop ────────────────────────────────────────────────────────

    private void startLoop() {
        timer = new AnimationTimer() {
            @Override
            public void handle(long now) {
                if (lastFrameNanos < 0) { lastFrameNanos = now; return; }
                double dt = Math.min(0.05, Math.max(0.001, (now - lastFrameNanos) / 1_000_000_000.0));
                lastFrameNanos = now;
                update(dt);
            }
        };
        timer.start();
    }

    // ── Main update ───────────────────────────────────────────────────────────

    private void update(double dt) {
        // Consume one-shot input
        boolean pressed  = primaryPressedThisFrame;  primaryPressedThisFrame  = false;
        boolean released = primaryReleasedThisFrame; primaryReleasedThisFrame = false;
        double scroll = scrollAccum; scrollAccum = 0;
        double mdx = mouseDeltaX;   mouseDeltaX = 0;
        double mdy = mouseDeltaY;   mouseDeltaY = 0;

        boolean doCtrlN = keyCtrlN; keyCtrlN = false;
        boolean doCtrlK = keyCtrlK; keyCtrlK = false;
        boolean doCtrlE = keyCtrlE; keyCtrlE = false;
        boolean doCtrlI = keyCtrlI; keyCtrlI = false;
        boolean doCtrlH = keyCtrlH; keyCtrlH = false;
        boolean doEsc   = keyEscape; keyEscape = false;
        boolean doEnter = keyEnter;  keyEnter  = false;
        boolean doUp    = keyUp;     keyUp     = false;
        boolean doDown  = keyDown;   keyDown   = false;

        // Writing mode
        if (writing) {
            updateWritingBackBtn();
            if (doEsc) commitWriting();
            return;
        }

        // Global shortcuts
        if (doCtrlN && !searchOpen) { openWritingNew(); return; }
        if (doCtrlK) {
            searchOpen = !searchOpen;
            searchOverlay.setVisible(searchOpen);
            if (searchOpen) {
                searchField.clear();
                searchSel = 0;
                updateSearchResults();
                Platform.runLater(() -> searchField.requestFocus());
            } else {
                canvas.requestFocus();
            }
        }
        if (doCtrlE && !searchOpen) { exportVault(); }
        if (doCtrlI && !searchOpen) { importVault(); }
        if (doCtrlH) { helpOpen = !helpOpen; }
        if (doEsc) {
            if (helpOpen)               { helpOpen = false; }
            else if (searchOpen)        { searchOpen = false; searchOverlay.setVisible(false); searchField.clear(); canvas.requestFocus(); }
            else if (selectedEdgeSrc != null) { selectedEdgeSrc = null; selectedEdgeTgt = null; }
            else if (localRoot != null)  { localBack(); }
        }

        // Search navigation
        if (searchOpen) {
            if (doUp   && searchSel > 0)                             { searchSel--; rebuildSearchUI(); }
            if (doDown && searchSel < currentSearchResults.size()-1)  { searchSel++; rebuildSearchUI(); }
            if (doEnter && !currentSearchResults.isEmpty())           { flyToNode(currentSearchResults.get(searchSel)); }
        }

        // Refresh conn counts only when the edge list has changed
        if (connCountsDirty) rebuildConnCounts();

        boolean inLocal = (localRoot != null);
        Set<Long> localVis = new HashSet<>(localVisible);

        // Smooth zoom
        if (scroll != 0 && !searchOpen) {
            double factor = Math.exp(scroll * 0.12);
            zoomTarget = Math.max(0.08, Math.min(8.0, zoomTarget * factor));
        }
        double zoomDiff = zoomTarget - zoom;
        if (Math.abs(zoomDiff) > 0.0005) {
            double oldZoom = zoom;
            zoom += zoomDiff * (1.0 - Math.exp(-12.0 * dt));
            double ratio = zoom / oldZoom;
            panX = zoomAnchorX - (zoomAnchorX - panX) * ratio;
            panY = zoomAnchorY - (zoomAnchorY - panY) * ratio;
        }

        // Camera glide (fly-to)
        if (hasFlyTo) {
            double tx = canvas.getWidth()  * 0.5 - flyToX * zoom;
            double ty = canvas.getHeight() * 0.5 - flyToY * zoom;
            double dx = tx - panX, dy = ty - panY;
            if (Math.sqrt(dx*dx + dy*dy) > 0.5) {
                double k = 1.0 - Math.exp(-9.0 * dt);
                panX += dx * k;
                panY += dy * k;
                panVelX = 0; panVelY = 0;
            } else {
                panX = tx; panY = ty;
                hasFlyTo = false;
            }
        }

        // Press: hit-test nodes, edges, panning
        if (pressed && !searchOpen) {
            int hitIdx = -1;
            for (int i = nodes.size() - 1; i >= 0; i--) {
                Node n = nodes.get(i);
                if (inLocal && !localVis.contains(n.id)) continue;
                int conns = connCounts.getOrDefault(n.id, 0);
                double r = Utils.nodeRadius(conns) + 8.0 / zoom;
                double dx = n.x - toWorldX(pointerX), dy = n.y - toWorldY(pointerY);
                if (Math.sqrt(dx*dx + dy*dy) < r) { hitIdx = i; break; }
            }
            if (hitIdx >= 0) {
                nodes.get(hitIdx).dragging = true;
                physicsAwake = true; // wake physics when user interacts
            } else {
                // Edge hit-test
                boolean edgeHit = false;
                for (Edge e : edges) {
                    if (inLocal && (!localVis.contains(e.sourceId) || !localVis.contains(e.targetId))) continue;
                    Node src = nodeById.get(e.sourceId);
                    Node tgt = nodeById.get(e.targetId);
                    if (src == null || tgt == null) continue;
                    double dist = Utils.pointSegmentDist(
                        pointerX, pointerY,
                        toScreenX(src.x), toScreenY(src.y),
                        toScreenX(tgt.x), toScreenY(tgt.y));
                    if (dist < 8.0) {
                        selectedEdgeSrc = e.sourceId;
                        selectedEdgeTgt = e.targetId;
                        selectedEdgeClickX = pointerX;
                        selectedEdgeClickY = pointerY;
                        edgeHit = true;
                        break;
                    }
                }
                if (!edgeHit) {
                    selectedEdgeSrc = null;
                    selectedEdgeTgt = null;
                    isPanning = true;
                    panVelX = 0; panVelY = 0;
                }
            }
        }

        // Pan
        if (!primaryDown) isPanning = false;
        if (isPanning && (mdx != 0 || mdy != 0)) {
            panX += mdx; panY += mdy;
            if (dt > 0) {
                panVelX = panVelX * 0.7 + (mdx / dt) * 0.3;
                panVelY = panVelY * 0.7 + (mdy / dt) * 0.3;
            }
        } else if (!isPanning && (panVelX * panVelX + panVelY * panVelY) > 4.0) {
            panX += panVelX * dt;
            panY += panVelY * dt;
            double decay = Math.exp(-5.0 * dt);
            panVelX *= decay;
            panVelY *= decay;
        } else if (!isPanning) {
            panVelX = 0; panVelY = 0;
        }

        // Node drag + click detection
        Long clickedId = null;
        for (Node node : nodes) {
            if (!node.dragging) continue;
            if (primaryDown) {
                double oldX = node.x, oldY = node.y;
                node.x = toWorldX(pointerX);
                node.y = toWorldY(pointerY);
                if (dt > 0) {
                    double fvx = (node.x - oldX) / dt;
                    double fvy = (node.y - oldY) / dt;
                    node.vx = node.vx * 0.6 + fvx * 0.4;
                    node.vy = node.vy * 0.6 + fvy * 0.4;
                }
            } else {
                node.dragging = false;
                double dx = pointerX - pressOriginX, dy = pointerY - pressOriginY;
                if (Math.sqrt(dx*dx + dy*dy) < 6.0) {
                    clickedId = node.id;
                    node.vx = 0; node.vy = 0;
                } else {
                    node.dirty = true;
                    double speed = Math.sqrt(node.vx*node.vx + node.vy*node.vy);
                    if (speed > 400.0) { node.vx = node.vx / speed * 400.0; node.vy = node.vy / speed * 400.0; }
                }
            }
        }

        // Physics
        int n = nodes.size();
        double[] fx = new double[n], fy = new double[n];

        if (physicsAwake) {
            // Barnes-Hut O(n log n) repulsion
            QuadTree tree = QuadTree.build(nodes);
            if (tree != null) {
                for (int i = 0; i < n; i++) {
                    tree.force(nodes.get(i), REPULSION, MIN_DIST, fx, fy, i);
                }
            }
        }

        if (physicsAwake) {
            for (Edge e : edges) {
                Integer ii = physicsIdx.get(e.sourceId), jj = physicsIdx.get(e.targetId);
                if (ii == null || jj == null) continue;
                double dx = nodes.get(jj).x - nodes.get(ii).x;
                double dy = nodes.get(jj).y - nodes.get(ii).y;
                double dist = Math.sqrt(dx*dx + dy*dy);
                if (dist < 0.5) continue;
                double rest = Utils.edgeRestLen(e.sourceId, e.targetId);
                double disp = dist - rest;
                if (Math.abs(disp) < SPRING_DEAD) continue;
                double f = SPRING_K * disp;
                double ux = dx / dist, uy = dy / dist;
                fx[ii] += ux * f; fy[ii] += uy * f;
                fx[jj] -= ux * f; fy[jj] -= uy * f;
            }
        }

        double damp = Math.exp(-DAMPING * dt);
        List<long[]> toSave = new ArrayList<>();
        boolean anyActive = false;

        for (int i = 0; i < n; i++) {
            Node node = nodes.get(i);
            if (node.dragging) { anyActive = true; continue; }
            node.vx = (node.vx + fx[i] * dt) * damp;
            node.vy = (node.vy + fy[i] * dt) * damp;
            double speed = Math.sqrt(node.vx*node.vx + node.vy*node.vy);
            if (speed > STOP_VEL) {
                node.x += node.vx * dt;
                node.y += node.vy * dt;
                anyActive = true;
            } else {
                node.vx = 0; node.vy = 0;
                if (node.dirty) {
                    node.dirty = false;
                    toSave.add(new long[]{ node.id, Double.doubleToLongBits(node.x), Double.doubleToLongBits(node.y) });
                }
            }
        }
        for (long[] entry : toSave) {
            savePosition(entry[0], Double.longBitsToDouble(entry[1]), Double.longBitsToDouble(entry[2]));
        }
        physicsAwake = anyActive;

        // Toast tick
        if (toastTimer > 0) toastTimer -= dt;

        // Handle node click
        if (clickedId != null) {
            if (!inLocal) {
                enterLocal(clickedId);
            } else if (Objects.equals(localRoot, clickedId)) {
                openWritingEdit(clickedId);
            } else if (localVis.contains(clickedId)) {
                expandLocal(clickedId);
            }
        }

        // Hovered node
        boolean anyDragging = nodes.stream().anyMatch(nd -> nd.dragging);
        Long hoveredId = null;
        if (!anyDragging) {
            double bestDist = Double.MAX_VALUE;
            for (Node nd : nodes) {
                if (inLocal && !localVis.contains(nd.id)) continue;
                int conns = connCounts.getOrDefault(nd.id, 0);
                double r = Utils.nodeRadius(conns) + 10.0 / zoom;
                double dx = nd.x - toWorldX(pointerX), dy = nd.y - toWorldY(pointerY);
                double d = Math.sqrt(dx*dx + dy*dy);
                if (d < r && d < bestDist) { bestDist = d; hoveredId = nd.id; }
            }
        }

        // Canvas-level click detection (after node click so we don't double-fire)
        if (released && clickedId == null) {
            double moveDist = Math.sqrt(
                Math.pow(pointerX - pressOriginX, 2) + Math.pow(pointerY - pressOriginY, 2));
            if (moveDist < 6.0) {
                handleCanvasClick(pointerX, pointerY, inLocal);
            }
        }

        // Render
        renderGraph(connCounts, inLocal, localVis, hoveredId);

        // Cursor
        Cursor cursor = hoveredId != null ? Cursor.HAND
                      : isPanning          ? Cursor.OPEN_HAND
                      :                      Cursor.DEFAULT;
        canvas.setCursor(cursor);
    }

    private void handleCanvasClick(double cx, double cy, boolean inLocal) {
        double w = canvas.getWidth(), h = canvas.getHeight();

        // Back button (top-left, approximate hit area)
        if (inLocal && cx < 160 && cy < 36) {
            localBack();
            return;
        }

        // Help toggle ("? help" at 20, 52 or close button inside help panel)
        if (!helpOpen && cx >= 15 && cx <= 70 && cy >= 45 && cy <= 66) {
            helpOpen = true;
            return;
        }
        if (helpOpen && cx >= 18 && cx <= 318 && cy >= helpPanelBottom - 30 && cy <= helpPanelBottom) {
            helpOpen = false;
            return;
        }

        // Delete all (bottom-right corner)
        if (!confirmDeleteAll && cx >= w - 160 && cy >= h - 30) {
            confirmDeleteAll = true;
            return;
        }
        if (confirmDeleteAll) {
            if (cx >= w - 200 && cx <= w - 80 && cy >= h - 30) {
                // Confirm delete all
                try { db.deleteAllNodes(); db.deleteAllEdges(); } catch (SQLException ex) { ex.printStackTrace(); }
                nodes.clear(); edges.clear();
                localRoot = null; localVisible.clear(); navHistory.clear();
                selectedEdgeSrc = null; selectedEdgeTgt = null;
                confirmDeleteAll = false;
            } else if (cx >= w - 70 && cy >= h - 30) {
                confirmDeleteAll = false;
            }
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    private static Color rgba(int r, int g, int b, int a) {
        return Color.rgb(r, g, b, a / 255.0);
    }

    private void drawText(String text, double x, double y, double size,
                           Color color, TextAlignment align, VPos baseline) {
        gc.setFont(Font.font("System", size));
        gc.setFill(color);
        gc.setTextAlign(align);
        gc.setTextBaseline(baseline);
        gc.fillText(text, x, y);
    }

    private void drawPanel(double x, double y, double w, double h, Color fill, double arc) {
        gc.setFill(fill);
        gc.fillRoundRect(x, y, w, h, arc, arc);
    }

    private void renderGraph(Map<Long, Integer> connCounts, boolean inLocal,
                              Set<Long> localVis, Long hoveredId) {
        double w = canvas.getWidth(), h = canvas.getHeight();
        gc.clearRect(0, 0, w, h);
        gc.setFill(Color.rgb(4, 4, 6));
        gc.fillRect(0, 0, w, h);

        if (!initialized) {
            initialized = true;
            zoom = 0.65; zoomTarget = 0.65;
            if (nodes.isEmpty()) {
                panX = w * 0.5; panY = h * 0.5;
            } else {
                double cx = 0, cy = 0;
                for (Node n : nodes) { cx += n.x; cy += n.y; }
                cx /= nodes.size(); cy /= nodes.size();
                panX = w * 0.5 - cx * zoom;
                panY = h * 0.5 - cy * zoom;
            }
        }

        // Empty state hint
        if (nodes.isEmpty()) {
            drawText("Ctrl+N  new node  ·  Ctrl+I  import vault",
                w * 0.5, h * 0.5 - 10, 14,
                rgba(255, 255, 255, 22),
                TextAlignment.CENTER, VPos.CENTER);
        }

        // Edges
        for (Edge e : edges) {
            if (inLocal && (!localVis.contains(e.sourceId) || !localVis.contains(e.targetId))) continue;
            Node src = nodeById.get(e.sourceId);
            Node tgt = nodeById.get(e.targetId);
            if (src == null || tgt == null) continue;
            double ssx = toScreenX(src.x), ssy = toScreenY(src.y);
            double tsx = toScreenX(tgt.x), tsy = toScreenY(tgt.y);
            // Cull edges where both endpoints are off-screen in the same direction
            if ((ssx < 0 && tsx < 0) || (ssx > w && tsx > w) ||
                (ssy < 0 && tsy < 0) || (ssy > h && tsy > h)) continue;
            boolean isRootEdge = inLocal && (Objects.equals(localRoot, src.id) || Objects.equals(localRoot, tgt.id));
            int alpha = inLocal ? (isRootEdge ? 80 : 35) : 28;
            gc.setStroke(rgba(210, 210, 225, alpha));
            gc.setLineWidth(isRootEdge ? 1.0 : 0.7);
            gc.strokeLine(ssx, ssy, tsx, tsy);
        }

        // Nodes
        for (Node node : nodes) {
            if (inLocal && !localVis.contains(node.id)) continue;
            int conns = connCounts.getOrDefault(node.id, 0);
            double r = Utils.nodeRadius(conns) * zoom;
            double sx = toScreenX(node.x), sy = toScreenY(node.y);
            if (sx + r < 0 || sx - r > w || sy + r < 0 || sy - r > h) continue;
            boolean isRoot = inLocal && Objects.equals(localRoot, node.id);
            int alpha = inLocal ? (isRoot ? 255 : 190) : 210;

            gc.setFill(rgba(225, 224, 228, alpha));
            gc.fillOval(sx - r, sy - r, r * 2, r * 2);

            if (isRoot) {
                double pr = r + 4.0 * zoom;
                gc.setStroke(rgba(255, 255, 255, 50));
                gc.setLineWidth(1.0);
                gc.strokeOval(sx - pr, sy - pr, pr * 2, pr * 2);
            }

            boolean showLabel = inLocal || Objects.equals(hoveredId, node.id);
            if (showLabel) {
                double sz;
                if (inLocal) {
                    sz = isRoot ? Math.max(14.0, Math.min(18.0, 14.0 * Math.sqrt(zoom)))
                                : Math.max(12.0, Math.min(14.5, 12.0 * Math.sqrt(zoom)));
                } else {
                    sz = Math.max(12.5, Math.min(16.0, 12.5 * Math.sqrt(zoom)));
                }
                int la = isRoot ? 230 : 160;
                drawText(node.title, sx, sy - r - 7, sz,
                    rgba(255, 255, 255, la),
                    TextAlignment.CENTER, VPos.BOTTOM);
            }
        }

        // Search dimming overlay
        if (searchOpen) {
            gc.setFill(rgba(4, 4, 6, 180));
            gc.fillRect(0, 0, w, h);
        }

        // Local mode UI
        if (inLocal) {
            String backLabel;
            if (!navHistory.isEmpty()) {
                long prevId = navHistory.peek();
                String prevTitle = nodes.stream().filter(n -> n.id == prevId)
                    .findFirst().map(n -> n.title).orElse("back");
                backLabel = "← " + prevTitle;
            } else {
                backLabel = "← Full graph";
            }
            boolean backHover = pointerX < 160 && pointerY < 36;
            drawText(backLabel, 20, 20, 12,
                backHover ? rgba(200, 198, 215, 200) : rgba(120, 118, 135, 160),
                TextAlignment.LEFT, VPos.TOP);

        }

        // Help
        if (writing) return;
        if (!helpOpen) {
            boolean helpHintHover = pointerX >= 15 && pointerX <= 70 && pointerY >= 45 && pointerY <= 66;
            drawText("? help", 20, 52, 11,
                rgba(255, 255, 255, helpHintHover ? 50 : 22),
                TextAlignment.LEFT, VPos.TOP);
        } else {
            drawHelpPanel();
        }

        // Toast
        if (toastMsg != null && toastTimer > 0) {
            int alpha = (int) Math.min(255, Math.min(toastTimer * 255.0, (toastTimer) * 100.0));
            if (alpha > 0) {
                gc.setFont(Font.font("System", 13));
                gc.setTextAlign(TextAlignment.CENTER);
                gc.setTextBaseline(VPos.CENTER);
                // Approximate pill width
                double tw = toastMsg.length() * 7.5 + 28;
                double tx = w * 0.5 - tw / 2.0, ty = h - 51;
                drawPanel(tx, ty, tw, 30, rgba(18, 17, 24, alpha), 8);
                gc.setFill(rgba(220, 219, 228, alpha));
                gc.fillText(toastMsg, w * 0.5, ty + 15);
            }
        }

        // Delete all (bottom-right)
        if (!confirmDeleteAll) {
            drawText("Delete all nodes", w - 20, h - 20, 11,
                rgba(255, 255, 255, 22),
                TextAlignment.RIGHT, VPos.BOTTOM);
        } else {
            drawText("Confirm delete all", w - 80, h - 20, 11,
                rgba(200, 55, 55, 200),
                TextAlignment.RIGHT, VPos.BOTTOM);
            drawText("  Cancel", w - 20, h - 20, 11,
                rgba(55, 53, 68, 200),
                TextAlignment.RIGHT, VPos.BOTTOM);
        }

        // Living Connections panel
        if (selectedEdgeSrc != null && selectedEdgeTgt != null) {
            Node src = nodes.stream().filter(n -> n.id == selectedEdgeSrc).findFirst().orElse(null);
            Node tgt = nodes.stream().filter(n -> n.id == selectedEdgeTgt).findFirst().orElse(null);
            if (src != null && tgt != null) {
                List<String> reasons = Utils.connectionReasons(src, tgt);
                double pw = 300, ph = reasons.isEmpty() ? 90 : 70 + reasons.size() * 22;
                double px = Math.max(8, Math.min(w - pw - 8, selectedEdgeClickX - pw * 0.5));
                double py = Math.min(selectedEdgeClickY + 18, h - ph - 8);
                drawPanel(px, py, pw, ph, Color.rgb(10, 10, 14), 8);

                double lineY = py + 20;
                drawText(src.title + "  ─  " + tgt.title, px + 18, lineY, 13,
                    rgba(200, 199, 210, 255), TextAlignment.LEFT, VPos.TOP);
                lineY += 28;
                if (reasons.isEmpty()) {
                    drawText("Linked by title reference", px + 18, lineY, 12,
                        rgba(85, 83, 95, 255), TextAlignment.LEFT, VPos.TOP);
                } else {
                    drawText("Connected via", px + 18, lineY, 11,
                        rgba(70, 68, 80, 255), TextAlignment.LEFT, VPos.TOP);
                    lineY += 18;
                    for (String r : reasons) {
                        gc.setFont(Font.font("Monospaced", 13));
                        gc.setFill(rgba(160, 162, 180, 255));
                        gc.setTextAlign(TextAlignment.LEFT);
                        gc.setTextBaseline(VPos.TOP);
                        gc.fillText(r, px + 18, lineY);
                        lineY += 22;
                    }
                }
                drawText("Esc to close", px + 18, py + ph - 16, 10,
                    rgba(40, 39, 48, 255), TextAlignment.LEFT, VPos.TOP);
            }
        }
    }

    private void drawHelpPanel() {
        double px = 18, py = 48, pw = 300, pad = 20;
        double y = py + pad;

        // Measure approximate height first
        int rows = 17; // number of key rows
        double estimatedH = pad * 2 + 14 + 10 + rows * 18 + 4 * 22 + 28;
        drawPanel(px, py, pw, estimatedH, rgba(10, 10, 14, 245), 10);

        gc.setFont(Font.font("System", 14));
        gc.setFill(rgba(200, 199, 210, 255));
        gc.setTextAlign(TextAlignment.LEFT);
        gc.setTextBaseline(VPos.TOP);
        gc.fillText("Bibbo", px + pad, y);
        y += 24;

        Color keyCol  = rgba(200, 199, 215, 255);
        Color descCol = rgba(90, 88, 105, 255);
        Color headCol = rgba(55, 53, 68, 255);

        y = drawHelpSection(y, px + pad, "WRITING", headCol, keyCol, descCol, new String[][]{
            {"Ctrl + N",   "new node"},
            {"Esc",        "save & return to graph"},
            {"[[Title]]",  "link to another node"},
        });
        y = drawHelpSection(y, px + pad, "GRAPH", headCol, keyCol, descCol, new String[][]{
            {"Click node",       "enter local view"},
            {"Click node again", "open writing mode"},
            {"Click neighbor",   "expand web"},
            {"Click edge",       "see why connected"},
            {"Scroll",           "zoom in / out"},
            {"Drag",             "pan canvas"},
            {"Drag node",        "move node"},
        });
        y = drawHelpSection(y, px + pad, "NAVIGATE", headCol, keyCol, descCol, new String[][]{
            {"Ctrl + K",  "search nodes"},
            {"Esc",       "back / close panel"},
        });
        y = drawHelpSection(y, px + pad, "DATA", headCol, keyCol, descCol, new String[][]{
            {"Ctrl + E",  "export to .md files"},
            {"Ctrl + I",  "import .md folder"},
        });

        y += 8;
        gc.setStroke(rgba(25, 24, 32, 255));
        gc.setLineWidth(1);
        gc.strokeLine(px + pad, y, px + pw - pad, y);
        y += 8;

        boolean closeHover = pointerX >= px && pointerX <= px + pw && pointerY >= y && pointerY <= y + 20;
        drawText("Close  Esc", px + pad, y, 11,
            closeHover ? rgba(120, 118, 135, 200) : rgba(55, 53, 68, 200),
            TextAlignment.LEFT, VPos.TOP);
        helpPanelBottom = y + 20;
    }

    private double drawHelpSection(double y, double x, String heading,
                                    Color headCol, Color keyCol, Color descCol,
                                    String[][] rows) {
        drawText(heading, x, y, 10, headCol, TextAlignment.LEFT, VPos.TOP);
        y += 14;
        for (String[] row : rows) {
            gc.setFont(Font.font("Monospaced", 12));
            gc.setFill(keyCol);
            gc.setTextAlign(TextAlignment.LEFT);
            gc.setTextBaseline(VPos.TOP);
            gc.fillText(row[0], x, y);
            gc.setFont(Font.font("System", 12));
            gc.setFill(descCol);
            gc.fillText(row[1], x + 115, y);
            y += 17;
        }
        y += 10;
        return y;
    }
}
