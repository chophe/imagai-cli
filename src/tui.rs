use std::collections::HashMap;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::DefaultTerminal;
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::config::Settings;
use crate::core::generate_image_core;
use crate::history::{self, MAX_TREE_DEPTH};
use crate::models::ImageGenerationRequest;

const ROWS: usize = 11;
const ROW_PROMPT: usize = 0;
const ROW_ENGINE: usize = 1;
const ROW_SIZE: usize = 2;
const ROW_QUALITY: usize = 3;
const ROW_STYLE: usize = 4;
const ROW_COUNT: usize = 5;
const ROW_OUTPUT: usize = 6;
const ROW_AUTO: usize = 7;
const ROW_RANDOM: usize = 8;
const ROW_VERBOSE: usize = 9;
const ROW_SUBMIT: usize = 10;

const EDIT_ROWS: usize = 5;
const EDIT_ROW_SOURCE: usize = 0;
const EDIT_ROW_PROMPT: usize = 1;
const EDIT_ROW_ENGINE: usize = 2;
const EDIT_ROW_OUTPUT: usize = 3;
const EDIT_ROW_SUBMIT: usize = 4;

// History sits at index 2, between Edit and Engines, because forking always
// continues in the Edit tab. HAZARD: inserting a tab here means re-indexing
// both `draw`'s and `draw_footer`'s dispatch — they go through `tab_surface`,
// which is exhaustive over 0..=4 with no catch-all arm so a future tab cannot
// silently land on About (T-02-08).
const TABS: [&str; 5] = ["Generate", "Edit", "History", "Engines", "About"];

/// The History tab's key hints (the Copywriting Contract's footer line).
const HISTORY_FOOTER_HINT: &str =
    "q Quit | Tab switch tab | ↑/↓ navigate | Enter fork from node | p pick from history";

/// The first entry of the Edit tab's source picker (D-18).
const PICK_FROM_HISTORY: &str = " Pick from history… ";

/// The surface a tab index renders. Extracted from `draw`'s match so the tab
/// table is exhaustive and provable in a unit test without a terminal: an
/// out-of-range index yields `None` and renders nothing, rather than silently
/// falling through to About (T-02-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TabSurface {
    Generate,
    Edit,
    History,
    Engines,
    About,
}

fn tab_surface(tab: usize) -> Option<TabSurface> {
    match tab {
        0 => Some(TabSurface::Generate),
        1 => Some(TabSurface::Edit),
        2 => Some(TabSurface::History),
        3 => Some(TabSurface::Engines),
        4 => Some(TabSurface::About),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogKind {
    Info,
    Success,
    Error,
    Warn,
}

struct LogLine {
    kind: LogKind,
    text: String,
}

/// What a picker selection means. The Edit tab's picker either writes the
/// chosen file into `edit_source`, or — on its first row — hands off to the
/// History tab. The discriminator is this field on the one picker type, not a
/// second overlay (UI-SPEC rule 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickerOrigin {
    /// A plain source choice: the first row is ` Pick from history… `.
    Source,
    /// Opened while a pick from history was already pending: the picker lists
    /// files only, because the hand-off row has already been used.
    History,
}

struct PickerState {
    entries: Vec<String>,
    selected: usize,
    scroll: usize,
    height: usize,
    origin: PickerOrigin,
}

/// The History tab's state: the loaded view, the flattened rows, and the
/// `selected`/`scroll`/`height` triple the picker uses (UI-SPEC rule 3 — one
/// scrolling idiom, not a second). `loaded` is the refresh flag: the view is
/// (re)loaded on entering the tab and after a successful edit or generation, so
/// a fork's new node appears without restarting the TUI.
struct HistoryState {
    rows: Vec<HistoryRow>,
    selected: usize,
    scroll: usize,
    height: usize,
    loaded: bool,
}

/// One row of the flattened tree: the rendered line plus the payload the Node
/// detail pane shows. `filename`/`node` are `None` for a row that carries no
/// node (the `… n more` line and the empty-state row).
#[derive(Clone)]
struct HistoryRow {
    line: Line<'static>,
    node: Option<history::HistoryNode>,
}

/// A minimal single-line text editor with a byte-indexed cursor.
#[derive(Debug, Clone, Default)]
struct Editor {
    content: String,
    cursor: usize,
}

impl Editor {
    fn new(s: &str) -> Self {
        Editor {
            content: s.to_string(),
            cursor: s.len(),
        }
    }

    fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = self.content[..self.cursor]
            .char_indices()
            .map(|(i, _)| i)
            .next_back()
            .unwrap_or(0);
        self.content.remove(prev);
        self.cursor = prev;
    }

    fn delete(&mut self) {
        if self.cursor >= self.content.len() {
            return;
        }
        self.content.remove(self.cursor);
    }

    fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor = self.content[..self.cursor]
            .char_indices()
            .map(|(i, _)| i)
            .next_back()
            .unwrap_or(0);
    }

    fn move_right(&mut self) {
        if self.cursor >= self.content.len() {
            return;
        }
        let n = self.content[self.cursor..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        self.cursor += n;
    }
}

struct App {
    settings: Settings,
    tab: usize,
    focus: usize,
    editing: Option<usize>,
    editor: Editor,
    edit_focus: usize,
    edit_editing: Option<usize>,
    edit_picker: Option<PickerState>,
    edit_refs_editing: bool,
    history: Option<HistoryState>,
    history_pick_for_edit: bool,

    prompt: String,
    engine: String,
    size: String,
    quality: String,
    style: String,
    count: String,
    output: String,
    auto_filename: bool,
    random_filename: bool,
    verbose: bool,
    edit_prompt: String,
    edit_source: String,
    edit_engine: String,
    edit_output: String,
    edit_refs: String,

    logs: Vec<LogLine>,
    generating: bool,
    rx: Option<mpsc::UnboundedReceiver<Vec<crate::models::ImageGenerationResponse>>>,

    engines_models: HashMap<String, Vec<String>>,
    fetching_models: bool,
    models_rx: Option<ModelsFetch>,
    status: String,
}

/// Receiver channel for the asynchronous "fetch models" task.
type ModelsFetch = mpsc::UnboundedReceiver<Vec<(String, Result<Vec<String>, String>)>>;

impl App {
    fn new(settings: &Settings) -> Self {
        App {
            settings: settings.clone(),
            tab: 0,
            focus: ROW_PROMPT,
            editing: None,
            editor: Editor::default(),
            edit_focus: EDIT_ROW_SOURCE,
            edit_editing: None,
            edit_picker: None,
            edit_refs_editing: false,
            history: None,
            history_pick_for_edit: false,
            prompt: String::new(),
            engine: settings.default_engine.clone().unwrap_or_default(),
            size: "1024x1024".to_string(),
            quality: "standard".to_string(),
            style: "vivid".to_string(),
            count: "1".to_string(),
            output: String::new(),
            auto_filename: false,
            random_filename: false,
            verbose: false,
            edit_prompt: String::new(),
            edit_source: String::new(),
            edit_engine: settings.default_engine.clone().unwrap_or_default(),
            edit_output: String::new(),
            edit_refs: String::new(),
            logs: Vec::new(),
            generating: false,
            rx: None,
            engines_models: HashMap::new(),
            fetching_models: false,
            models_rx: None,
            status: "Ready".to_string(),
        }
    }

    fn push_log(&mut self, kind: LogKind, text: String) {
        self.logs.push(LogLine { kind, text });
        if self.logs.len() > 500 {
            let drop = self.logs.len() - 500;
            self.logs.drain(..drop);
        }
    }

    // ---- form accessors ----

    fn text_field(&self, row: usize) -> &str {
        match row {
            ROW_PROMPT => &self.prompt,
            ROW_ENGINE => &self.engine,
            ROW_SIZE => &self.size,
            ROW_QUALITY => &self.quality,
            ROW_STYLE => &self.style,
            ROW_COUNT => &self.count,
            ROW_OUTPUT => &self.output,
            _ => "",
        }
    }

    fn text_field_mut(&mut self, row: usize) -> &mut String {
        match row {
            ROW_PROMPT => &mut self.prompt,
            ROW_ENGINE => &mut self.engine,
            ROW_SIZE => &mut self.size,
            ROW_QUALITY => &mut self.quality,
            ROW_STYLE => &mut self.style,
            ROW_COUNT => &mut self.count,
            ROW_OUTPUT => &mut self.output,
            _ => unreachable!(),
        }
    }

    fn bool_field(&self, row: usize) -> Option<bool> {
        match row {
            ROW_AUTO => Some(self.auto_filename),
            ROW_RANDOM => Some(self.random_filename),
            ROW_VERBOSE => Some(self.verbose),
            _ => None,
        }
    }

    fn toggle_bool(&mut self, row: usize) {
        match row {
            ROW_AUTO => self.auto_filename = !self.auto_filename,
            ROW_RANDOM => self.random_filename = !self.random_filename,
            ROW_VERBOSE => self.verbose = !self.verbose,
            _ => {}
        }
    }

    fn row_label(row: usize) -> &'static str {
        match row {
            ROW_PROMPT => "Prompt",
            ROW_ENGINE => "Engine",
            ROW_SIZE => "Size",
            ROW_QUALITY => "Quality",
            ROW_STYLE => "Style",
            ROW_COUNT => "Count",
            ROW_OUTPUT => "Output",
            ROW_AUTO => "Auto-filename",
            ROW_RANDOM => "Random-filename",
            ROW_VERBOSE => "Verbose",
            ROW_SUBMIT => "Generate",
            _ => "",
        }
    }

    fn edit_text_field(&self, row: usize) -> &str {
        match row {
            EDIT_ROW_SOURCE => &self.edit_source,
            EDIT_ROW_PROMPT => &self.edit_prompt,
            EDIT_ROW_ENGINE => &self.edit_engine,
            EDIT_ROW_OUTPUT => &self.edit_output,
            _ => "",
        }
    }

    fn edit_text_field_mut(&mut self, row: usize) -> &mut String {
        match row {
            EDIT_ROW_SOURCE => &mut self.edit_source,
            EDIT_ROW_PROMPT => &mut self.edit_prompt,
            EDIT_ROW_ENGINE => &mut self.edit_engine,
            EDIT_ROW_OUTPUT => &mut self.edit_output,
            _ => unreachable!(),
        }
    }

    fn edit_row_label(row: usize) -> &'static str {
        match row {
            EDIT_ROW_SOURCE => "Source",
            EDIT_ROW_PROMPT => "Change",
            EDIT_ROW_ENGINE => "Engine",
            EDIT_ROW_OUTPUT => "Output",
            EDIT_ROW_SUBMIT => "Edit",
            _ => "",
        }
    }

    // ---- actions ----

    fn start_generation(&mut self) {
        if self.generating {
            self.status = "Already generating...".to_string();
            return;
        }
        let Some(request) = self.build_request() else {
            return;
        };
        let settings = self.settings.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        self.rx = Some(rx);
        self.generating = true;
        self.status = format!("Generating with engine '{}'…", request.engine);
        self.push_log(
            LogKind::Info,
            format!(
                "🖼️ Generating {} image(s) with engine '{}'…",
                request.n, request.engine
            ),
        );
        tokio::spawn(async move {
            let results = generate_image_core(&request, &settings).await;
            let _ = tx.send(results);
        });
    }

    fn build_request(&mut self) -> Option<ImageGenerationRequest> {
        let prompt = self.prompt.trim().to_string();
        if prompt.is_empty() {
            self.status = "Prompt cannot be empty".to_string();
            self.push_log(LogKind::Error, "Prompt cannot be empty".to_string());
            return None;
        }
        let engine = {
            let e = self.engine.trim().to_string();
            if e.is_empty() {
                match &self.settings.default_engine {
                    Some(d) => d.clone(),
                    None => {
                        self.status = "No engine specified and none configured".to_string();
                        self.push_log(
                            LogKind::Error,
                            "No engine specified and no default engine configured.".to_string(),
                        );
                        return None;
                    }
                }
            } else {
                e
            }
        };
        if self.settings.get_engine(&engine).is_none() {
            let avail = self.settings.engine_names().join(", ");
            self.status = format!("Engine '{engine}' not configured");
            self.push_log(
                LogKind::Error,
                format!("Engine '{engine}' is not configured. Available: {avail}"),
            );
            return None;
        }
        let n = self.count.trim().parse::<u32>().unwrap_or(1).max(1);
        Some(ImageGenerationRequest {
            prompt,
            engine,
            output_filename: {
                let o = self.output.trim().to_string();
                if o.is_empty() {
                    None
                } else {
                    Some(o)
                }
            },
            size: self.size.trim().to_string(),
            quality: self.quality.trim().to_string(),
            n,
            style: self.style.trim().to_string(),
            response_format: "b64_json".to_string(),
            extra_params: HashMap::new(),
            verbose: self.verbose,
            auto_filename: self.auto_filename,
            random_filename: self.random_filename,
            // Edit behaviour arrives with plan 01-04; the fields exist so the
            // crate compiles against the shared DTO.
            source_image: None,
            ref_images: Vec::new(),
        })
    }

    fn start_edit(&mut self) {
        if self.generating {
            self.status = "Already generating...".to_string();
            return;
        }
        let Some(request) = self.build_edit_request() else {
            return;
        };
        let settings = self.settings.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        self.rx = Some(rx);
        self.generating = true;
        let source = request.source_image.clone().unwrap_or_default();
        self.status = format!("Editing with engine '{}'…", request.engine);
        self.push_log(
            LogKind::Info,
            format!(
                "🎨 Editing with engine '{}' from source '{source}'…",
                request.engine
            ),
        );
        tokio::spawn(async move {
            let results = generate_image_core(&request, &settings).await;
            let _ = tx.send(results);
        });
    }

    fn build_edit_request(&mut self) -> Option<ImageGenerationRequest> {
        let prompt = self.edit_prompt.trim().to_string();
        if prompt.is_empty() {
            self.status = "Change instruction cannot be empty".to_string();
            self.push_log(
                LogKind::Error,
                "Change instruction cannot be empty".to_string(),
            );
            return None;
        }
        let engine = {
            let e = self.edit_engine.trim().to_string();
            if e.is_empty() {
                match &self.settings.default_engine {
                    Some(d) => d.clone(),
                    None => {
                        self.status = "No engine specified and none configured".to_string();
                        self.push_log(
                            LogKind::Error,
                            "No engine specified and no default engine configured.".to_string(),
                        );
                        return None;
                    }
                }
            } else {
                e
            }
        };
        if self.settings.get_engine(&engine).is_none() {
            let avail = self.settings.engine_names().join(", ");
            self.status = format!("Engine '{engine}' not configured");
            self.push_log(
                LogKind::Error,
                format!("Engine '{engine}' is not configured. Available: {avail}"),
            );
            return None;
        }
        let source_image = {
            let s = self.edit_source.trim().to_string();
            if s.is_empty() {
                let picked = crate::utils::list_output_images(&self.settings.output_dir)
                    .into_iter()
                    .next();
                match picked {
                    Some(p) => {
                        self.push_log(
                            LogKind::Info,
                            format!("Using most recent image: {}", p.to_string_lossy()),
                        );
                        p
                    }
                    None => {
                        self.status = "No images found in the output directory".to_string();
                        self.push_log(
                            LogKind::Error,
                            "No images found in the output directory — generate one first"
                                .to_string(),
                        );
                        return None;
                    }
                }
            } else {
                let rel = self.settings.output_dir.join(&s);
                let candidate = if rel.is_file() {
                    rel
                } else {
                    std::path::PathBuf::from(&s)
                };
                if !candidate.is_file() {
                    self.status = format!("Source image '{s}' not found");
                    self.push_log(
                        LogKind::Error,
                        format!("Source image '{s}' not found or unreadable"),
                    );
                    return None;
                }
                candidate
            }
        };
        let wanted_refs: Vec<String> = self
            .edit_refs
            .split(',')
            .map(|e| e.trim().to_string())
            .filter(|e| !e.is_empty())
            .collect();
        let mut ref_images = Vec::new();
        for name in wanted_refs {
            let candidate = self.settings.output_dir.join(&name);
            if std::fs::metadata(&candidate).is_err() {
                self.status = format!("Reference image '{name}' not found");
                self.push_log(
                    LogKind::Error,
                    format!("Reference image '{name}' not found"),
                );
                return None;
            }
            ref_images.push(candidate.to_string_lossy().to_string());
        }
        let defaults = ImageGenerationRequest::default();
        Some(ImageGenerationRequest {
            prompt,
            engine,
            output_filename: {
                let o = self.edit_output.trim().to_string();
                if o.is_empty() {
                    None
                } else {
                    Some(o)
                }
            },
            size: defaults.size,
            quality: defaults.quality,
            n: 1,
            style: defaults.style,
            response_format: defaults.response_format,
            extra_params: HashMap::new(),
            verbose: false,
            auto_filename: false,
            random_filename: false,
            source_image: Some(source_image.to_string_lossy().to_string()),
            ref_images,
        })
    }

    fn drain_messages(&mut self) {
        let mut results: Option<Vec<crate::models::ImageGenerationResponse>> = None;
        if let Some(rx) = &mut self.rx {
            match rx.try_recv() {
                Ok(r) => results = Some(r),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
                Err(_) => {
                    self.generating = false;
                    self.rx = None;
                    self.status = "Generation task ended unexpectedly".to_string();
                }
            }
        }
        if let Some(results) = results {
            self.generating = false;
            self.rx = None;
            let mut saved = false;
            for (i, r) in results.iter().enumerate() {
                if let Some(e) = &r.error {
                    self.push_log(LogKind::Error, format!("Image {} error: {e}", i + 1));
                } else if let Some(p) = &r.saved_path {
                    // The hook in `generate_image_core` recorded a node for
                    // this result, so the tree gains it here.
                    saved = true;
                    self.push_log(LogKind::Success, format!("✅ Image {} saved to {p}", i + 1));
                } else if let Some(u) = &r.image_url {
                    self.push_log(LogKind::Info, format!("Image {} URL: {u}", i + 1));
                } else if let Some(t) = &r.text_content {
                    self.push_log(LogKind::Info, format!("💬 Model response: {t}",));
                } else {
                    self.push_log(LogKind::Warn, format!("Image {}: no payload.", i + 1));
                }
            }
            self.status = if results.is_empty() {
                "Generation finished (no results)".to_string()
            } else {
                "Generation finished".to_string()
            };
            // A saved result refreshes the tree without a restart; a failed one
            // leaves the view alone and lets the error log speak.
            if saved {
                self.load_history();
            }
        }
    }

    // ---- event handling ----

    fn handle_key(&mut self, code: KeyCode) -> bool {
        if self.edit_picker.is_some() {
            return self.handle_picker_key(code);
        }
        if self.tab == 2 {
            return self.handle_history_key(code);
        }
        if self.tab == 1 {
            return self.handle_edit_key(code);
        }
        // Quit always available.
        if code == KeyCode::Char('q') && self.editing.is_none() {
            return true;
        }

        if let Some(row) = self.editing {
            // ---- editing a text field ----
            match code {
                KeyCode::Esc => {
                    self.commit_edit(row);
                    self.editing = None;
                }
                KeyCode::Enter => {
                    self.commit_edit(row);
                    self.editing = None;
                    self.focus = (self.focus + 1) % ROWS;
                }
                KeyCode::Left => self.editor.move_left(),
                KeyCode::Right => self.editor.move_right(),
                KeyCode::Home => self.editor.cursor = 0,
                KeyCode::End => self.editor.cursor = self.editor.content.len(),
                KeyCode::Backspace => self.editor.backspace(),
                KeyCode::Delete => self.editor.delete(),
                KeyCode::Char(c) => self.editor.insert_char(c),
                _ => {}
            }
            return false;
        }

        match code {
            KeyCode::Char('q') => true,
            KeyCode::Tab => {
                self.focus = (self.focus + 1) % ROWS;
                false
            }
            KeyCode::BackTab => {
                self.focus = (self.focus + ROWS - 1) % ROWS;
                false
            }
            KeyCode::Up => {
                self.focus = (self.focus + ROWS - 1) % ROWS;
                false
            }
            KeyCode::Down => {
                self.focus = (self.focus + 1) % ROWS;
                false
            }
            KeyCode::Char('k') => {
                self.focus = (self.focus + ROWS - 1) % ROWS;
                false
            }
            KeyCode::Char('j') => {
                self.focus = (self.focus + 1) % ROWS;
                false
            }
            KeyCode::Enter => {
                if self.focus == ROW_SUBMIT {
                    self.start_generation();
                } else if self.bool_field(self.focus).is_some() {
                    self.toggle_bool(self.focus);
                } else {
                    self.begin_edit(self.focus);
                }
                false
            }
            KeyCode::Char(' ') => {
                if self.focus == ROW_SUBMIT {
                    self.start_generation();
                } else if self.bool_field(self.focus).is_some() {
                    self.toggle_bool(self.focus);
                }
                false
            }
            _ => false,
        }
    }

    fn begin_edit(&mut self, row: usize) {
        self.editor = Editor::new(self.text_field(row));
        self.editing = Some(row);
    }

    fn commit_edit(&mut self, row: usize) {
        if let Some(editing_row) = self.editing {
            if editing_row == row {
                *self.text_field_mut(row) = self.editor.content.clone();
            }
        }
    }

    fn handle_edit_key(&mut self, code: KeyCode) -> bool {
        if let Some(row) = self.edit_editing {
            match code {
                KeyCode::Esc => {
                    self.commit_edit_field(row);
                    self.edit_editing = None;
                }
                KeyCode::Enter => {
                    self.commit_edit_field(row);
                    self.edit_editing = None;
                    self.edit_focus = (self.edit_focus + 1) % EDIT_ROWS;
                }
                KeyCode::Left => self.editor.move_left(),
                KeyCode::Right => self.editor.move_right(),
                KeyCode::Home => self.editor.cursor = 0,
                KeyCode::End => self.editor.cursor = self.editor.content.len(),
                KeyCode::Backspace => self.editor.backspace(),
                KeyCode::Delete => self.editor.delete(),
                KeyCode::Char(c) => self.editor.insert_char(c),
                _ => {}
            }
            return false;
        }
        if self.edit_refs_editing {
            match code {
                KeyCode::Esc => {
                    self.edit_refs = self.editor.content.clone();
                    self.edit_refs_editing = false;
                }
                KeyCode::Enter => {
                    self.edit_refs = self.editor.content.clone();
                    self.edit_refs_editing = false;
                }
                KeyCode::Left => self.editor.move_left(),
                KeyCode::Right => self.editor.move_right(),
                KeyCode::Home => self.editor.cursor = 0,
                KeyCode::End => self.editor.cursor = self.editor.content.len(),
                KeyCode::Backspace => self.editor.backspace(),
                KeyCode::Delete => self.editor.delete(),
                KeyCode::Char(c) => self.editor.insert_char(c),
                _ => {}
            }
            return false;
        }
        match code {
            KeyCode::Char('q') => true,
            KeyCode::Esc => {
                self.edit_editing = None;
                self.edit_refs_editing = false;
                false
            }
            KeyCode::Enter => {
                if self.edit_focus == EDIT_ROW_SUBMIT {
                    self.start_edit();
                } else if self.edit_focus == EDIT_ROW_SOURCE {
                    self.open_source_picker();
                } else {
                    self.begin_edit_field(self.edit_focus);
                }
                false
            }
            KeyCode::Char('e') if self.edit_focus == EDIT_ROW_SOURCE => {
                self.begin_edit_field(EDIT_ROW_SOURCE);
                false
            }
            KeyCode::Char(' ') => {
                if self.edit_focus == EDIT_ROW_SUBMIT {
                    self.start_edit();
                }
                false
            }
            KeyCode::Tab => {
                self.edit_focus = (self.edit_focus + 1) % EDIT_ROWS;
                false
            }
            KeyCode::BackTab => {
                self.edit_focus = (self.edit_focus + EDIT_ROWS - 1) % EDIT_ROWS;
                false
            }
            KeyCode::Up => {
                self.edit_focus = (self.edit_focus + EDIT_ROWS - 1) % EDIT_ROWS;
                false
            }
            KeyCode::Down => {
                self.edit_focus = (self.edit_focus + 1) % EDIT_ROWS;
                false
            }
            KeyCode::Char('k') => {
                self.edit_focus = (self.edit_focus + EDIT_ROWS - 1) % EDIT_ROWS;
                false
            }
            KeyCode::Char('j') => {
                self.edit_focus = (self.edit_focus + 1) % EDIT_ROWS;
                false
            }
            KeyCode::Char('r') => {
                self.editor = Editor::new(&self.edit_refs);
                self.edit_refs_editing = true;
                false
            }
            _ => false,
        }
    }

    fn begin_edit_field(&mut self, row: usize) {
        self.editor = Editor::new(self.edit_text_field(row));
        self.edit_editing = Some(row);
    }

    fn commit_edit_field(&mut self, row: usize) {
        if self.edit_editing == Some(row) {
            *self.edit_text_field_mut(row) = self.editor.content.clone();
        }
    }

    fn open_source_picker(&mut self) {
        let images: Vec<String> = crate::utils::list_output_images(&self.settings.output_dir)
            .into_iter()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            })
            .collect();
        if images.is_empty() {
            self.push_log(
                LogKind::Warn,
                "No images found in the output directory — generate one first".to_string(),
            );
            return;
        }
        // With no pick pending the first row hands off to the History tab
        // (D-18); while a pick is already pending that row would loop, so the
        // picker lists files only.
        let origin = if self.history_pick_for_edit {
            PickerOrigin::History
        } else {
            PickerOrigin::Source
        };
        let mut entries = Vec::with_capacity(images.len() + 1);
        if origin == PickerOrigin::Source {
            entries.push(PICK_FROM_HISTORY.to_string());
        }
        entries.extend(images);
        self.edit_picker = Some(PickerState {
            entries,
            selected: 0,
            scroll: 0,
            height: 0,
            origin,
        });
    }

    /// Move `selected` by `delta` and keep `scroll` in step. One scroller, two
    /// callers: the Edit tab's source picker and the History tab's tree — the
    /// clamping, the scroll-window arithmetic and the tail clamp are identical
    /// because they are the same function.
    fn scroll_selection(
        selected: &mut usize,
        scroll: &mut usize,
        height: usize,
        len: usize,
        delta: isize,
    ) {
        if len == 0 {
            return;
        }
        *selected = (*selected as isize + delta).clamp(0, len as isize - 1) as usize;
        let height = height.max(1);
        if *selected < *scroll {
            *scroll = *selected;
        } else if *selected >= *scroll + height {
            *scroll = *selected + 1 - height;
        }
        let max_scroll = len.saturating_sub(height);
        *scroll = (*scroll).min(max_scroll);
    }

    fn picker_move(&mut self, delta: isize) {
        let Some(picker) = self.edit_picker.as_mut() else {
            return;
        };
        if picker.entries.is_empty() {
            return;
        }
        let len = picker.entries.len();
        Self::scroll_selection(
            &mut picker.selected,
            &mut picker.scroll,
            picker.height,
            len,
            delta,
        );
    }

    fn picker_select(&mut self) {
        let (index, name, origin) = {
            let Some(picker) = self.edit_picker.as_ref() else {
                return;
            };
            if picker.entries.is_empty() {
                self.edit_picker = None;
                return;
            }
            let index = picker.selected.min(picker.entries.len() - 1);
            (index, picker.entries[index].clone(), picker.origin)
        };
        if index == 0 && origin == PickerOrigin::Source {
            // The history row: choosing it means "pick a node from the tree",
            // so the picker hands off instead of writing a source.
            self.edit_picker = None;
            self.open_history_pick();
            return;
        }
        self.edit_source = name.clone();
        if self.edit_editing == Some(EDIT_ROW_SOURCE) {
            self.edit_editing = None;
        }
        self.edit_picker = None;
        // Completing a pick that started on the History tab.
        self.history_pick_for_edit = false;
        self.push_log(LogKind::Info, format!("Source image: {name}"));
    }

    fn picker_cancel(&mut self) {
        self.edit_picker = None;
    }

    // ---- history tab ----

    /// Load the lineage view and rebuild the flattened rows.
    ///
    /// An unreadable store is reported through the log pane — never `unwrap`,
    /// and never a silent empty tree (the Copywriting Contract's third error
    /// sentence arrives verbatim from `history::read_records`); the previous
    /// state stays on screen.
    fn load_history(&mut self) {
        match history::load_view(&self.settings.output_dir) {
            Ok(view) => {
                let missing = view.missing;
                let rows = history_rows(&view);
                let height = self.history.as_ref().map(|h| h.height).unwrap_or(0);
                self.history = Some(HistoryState {
                    rows,
                    selected: 0,
                    scroll: 0,
                    height,
                    loaded: true,
                });
                if missing > 0 {
                    self.push_log(
                        LogKind::Warn,
                        format!(
                            "{missing} history node(s) point at files no longer in the output directory"
                        ),
                    );
                }
            }
            Err(e) => {
                self.push_log(LogKind::Error, e.to_string());
            }
        }
    }

    fn history_move(&mut self, delta: isize) {
        let Some(state) = self.history.as_mut() else {
            return;
        };
        let len = state.rows.len();
        Self::scroll_selection(
            &mut state.selected,
            &mut state.scroll,
            state.height,
            len,
            delta,
        );
    }

    /// Fork from the selected node: put its filename in the Edit tab's source
    /// field, switch to the Edit tab, and push the exact log `picker_select`
    /// emits so the two paths are indistinguishable to the user (D-18).
    ///
    /// A node whose file is gone is refused with the Copywriting Contract's
    /// deleted-file sentence, and a filename carrying a separator is reduced to
    /// its final component before it reaches `edit_source` (T-02-07) — the same
    /// reduction `build_edit_request` applies when it joins onto `output_dir`.
    fn history_select(&mut self) {
        let Some(node) = self
            .history
            .as_ref()
            .and_then(|h| h.rows.get(h.selected))
            .and_then(|row| row.node.clone())
        else {
            // A `… n more` line or the empty-state row carries no node.
            return;
        };
        let filename = std::path::Path::new(&node.filename)
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from);
        match filename {
            Some(filename) if !node.missing => {
                self.edit_source = filename.clone();
                self.tab = 1;
                self.history_pick_for_edit = false;
                self.push_log(LogKind::Info, format!("Source image: {filename}"));
            }
            _ => {
                let dir = self.settings.output_dir.to_string_lossy().to_string();
                self.push_log(
                    LogKind::Error,
                    format!(
                        "History node {} points at '{}', which is no longer in {dir}.",
                        node.id, node.filename
                    ),
                );
            }
        }
    }

    /// The Edit tab's picker asked to pick from history: show the tree and mark
    /// that a pick is pending.
    fn open_history_pick(&mut self) {
        self.tab = 2;
        self.history_pick_for_edit = true;
        self.load_history();
    }

    fn handle_history_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.history_move(-1),
            KeyCode::Down | KeyCode::Char('j') => self.history_move(1),
            KeyCode::Enter => self.history_select(),
            // UI-SPEC: `p` forks via the Edit-tab picker path — go there with
            // the pick still pending, so choosing a source completes the flow.
            KeyCode::Char('p') => {
                self.tab = 1;
                self.open_source_picker();
            }
            KeyCode::Esc => {
                // Leaving without a choice must not touch `edit_source`.
                if self.history_pick_for_edit {
                    self.tab = 1;
                    self.history_pick_for_edit = false;
                }
            }
            KeyCode::Char('q') => return true,
            _ => {}
        }
        false
    }

    fn draw_history_tab(&mut self, frame: &mut Frame, area: Rect) {
        // First frame on this tab loads the view; `load_history` refreshes it
        // after a generation or edit.
        if self.history.as_ref().map(|h| !h.loaded).unwrap_or(true) {
            self.load_history();
        }
        let layout = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        self.draw_history_tree(frame, layout[0]);
        self.draw_history_detail(frame, layout[1]);
    }

    fn draw_history_tree(&mut self, frame: &mut Frame, area: Rect) {
        let (rows, selected, scroll) = {
            let Some(state) = self.history.as_ref() else {
                return;
            };
            (state.rows.clone(), state.selected, state.scroll)
        };
        let visible = area.height.saturating_sub(2) as usize;
        if visible == 0 {
            return;
        }
        let len = rows.len();
        let start = scroll.min(len.saturating_sub(1));
        let end = (start + visible).min(len);
        if let Some(state) = self.history.as_mut() {
            state.height = visible;
        }
        let block = Block::bordered().title(" History ");
        if len == 0 {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "No history yet — generate or edit an image first.",
                    Style::default().fg(Color::DarkGray),
                )))
                .block(block),
                area,
            );
            return;
        }
        let mut lines: Vec<Line> = Vec::with_capacity(end - start + 1);
        for (i, row) in rows[start..end].iter().enumerate() {
            let selected_here = start + i == selected;
            let mut spans = Vec::with_capacity(row.line.spans.len() + 1);
            let style = if selected_here {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            spans.push(Span::styled(
                if selected_here { "▶ " } else { "  " }.to_string(),
                style,
            ));
            for span in &row.line.spans {
                spans.push(Span::styled(span.content.to_string(), style));
            }
            lines.push(Line::from(spans));
        }
        if let Some(remaining) = remaining_line(end, len) {
            lines.push(Line::from(Span::styled(
                remaining,
                Style::default().fg(Color::DarkGray),
            )));
        }
        frame.render_widget(Paragraph::new(Text::from(lines)).block(block), area);
    }

    fn draw_history_detail(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered().title(" Node ");
        let Some(node) = self
            .history
            .as_ref()
            .and_then(|h| h.rows.get(h.selected))
            .and_then(|row| row.node.clone())
        else {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "No node selected",
                    Style::default().fg(Color::DarkGray),
                )))
                .block(block),
                area,
            );
            return;
        };

        let label_w = 10usize;
        let label_style = Style::default().fg(Color::Gray);
        let value_style = Style::default().fg(Color::White);
        let row = |label: &str, value: String, style: Style| -> Line<'static> {
            Line::from(vec![
                Span::styled(format!("{:<label_w$}", label), label_style),
                Span::raw("  "),
                Span::styled(value, style),
            ])
        };

        let mut lines: Vec<Line> = Vec::new();
        lines.push(row("id", node.id.clone(), value_style));
        if node.missing {
            lines.push(row(
                "file",
                format!("✗ {} (missing from disk)", node.filename),
                Style::default().fg(Color::Yellow),
            ));
        } else {
            lines.push(row("file", node.filename.clone(), value_style));
        }
        let prompt = if node.prompt.is_empty() {
            "(unknown prompt)".to_string()
        } else {
            node.prompt.clone()
        };
        lines.push(row("prompt", prompt, value_style));
        let model = if node.model.is_empty() {
            "(unknown model)".to_string()
        } else {
            node.model.clone()
        };
        lines.push(row("model", model, value_style));
        lines.push(row("created", node.created.clone(), value_style));
        lines.push(row(
            "parent",
            node.parent.clone().unwrap_or_else(|| "—".to_string()),
            value_style,
        ));
        lines.push(row(
            "children",
            node.children.len().to_string(),
            value_style,
        ));
        lines.push(Line::from(""));
        // The one CTA of this surface (Copywriting Contract).
        lines.push(Line::from(Span::styled(
            " [ Fork edit ] ",
            Style::default().fg(Color::Green),
        )));

        let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true });
        frame.render_widget(paragraph.block(block), area);
    }

    fn handle_picker_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.picker_move(-1),
            KeyCode::Down | KeyCode::Char('j') => self.picker_move(1),
            KeyCode::Enter => self.picker_select(),
            KeyCode::Esc => self.picker_cancel(),
            _ => {}
        }
        false
    }

    // ---- drawing ----

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let layout = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

        self.draw_header(frame, layout[0]);
        match tab_surface(self.tab) {
            Some(TabSurface::Generate) => self.draw_generate_tab(frame, layout[1]),
            Some(TabSurface::Edit) => self.draw_edit_tab(frame, layout[1]),
            Some(TabSurface::History) => self.draw_history_tab(frame, layout[1]),
            Some(TabSurface::Engines) => self.draw_engines_tab(frame, layout[1]),
            Some(TabSurface::About) => self.draw_about_tab(frame, layout[1]),
            // An out-of-range index renders nothing rather than the wrong
            // surface: `self.tab` is always `% TABS.len()`, so this arm is
            // unreachable in practice and exists only for exhaustiveness.
            None => {}
        }
        self.draw_footer(frame, layout[2]);
    }

    fn draw_header(&mut self, frame: &mut Frame, area: Rect) {
        let mut title_spans: Vec<Span> = vec![Span::styled(
            "🎨 Imagai",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )];
        title_spans.push(Span::raw("   "));
        for (i, t) in TABS.iter().enumerate() {
            let style = if i == self.tab {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            title_spans.push(Span::styled(format!(" {} ", t), style));
            title_spans.push(Span::raw(" "));
        }
        let title = Line::from(title_spans);
        let block = Block::bordered()
            .title(title)
            .title_alignment(Alignment::Left);
        frame.render_widget(Paragraph::new("").block(block), area);
    }

    fn draw_generate_tab(&mut self, frame: &mut Frame, area: Rect) {
        let layout = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        self.draw_form(frame, layout[0]);
        self.draw_logs(frame, layout[1]);
    }

    fn draw_form(&mut self, frame: &mut Frame, area: Rect) {
        let inner_w = area.width.saturating_sub(2) as usize;
        let label_w = 16usize;
        let value_w = inner_w.saturating_sub(label_w + 3).max(1);

        let mut lines: Vec<Line> = Vec::with_capacity(ROWS + 2);
        for row in 0..ROWS {
            let focused = self.focus == row;
            let editing_here = self.editing == Some(row);

            let label_style = if focused {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let mut spans: Vec<Span> = Vec::new();
            let label = format!("{:<label_w$}", Self::row_label(row));
            spans.push(Span::styled(label, label_style));
            spans.push(Span::raw("  "));

            if row == ROW_SUBMIT {
                let btn = if self.generating {
                    " [ Working… ] "
                } else {
                    " [ Generate ] "
                };
                let style = if focused {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                };
                spans.push(Span::styled(btn, style));
            } else if let Some(val) = self.bool_field(row) {
                let marker = if val { "☑" } else { "☐" };
                let style = if focused {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Yellow)
                };
                spans.push(Span::styled(
                    format!("{marker} {}", Self::row_label(row)),
                    style,
                ));
            } else {
                // text field
                let content = if editing_here {
                    self.editor.content.clone()
                } else {
                    self.text_field(row).to_string()
                };
                let cursor = if editing_here {
                    self.editor.cursor
                } else {
                    content.len()
                };
                let (rendered, rendered_chars) = render_value_line(&content, cursor, editing_here);
                let pad = value_w.saturating_sub(rendered_chars);
                let value_style = if focused {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                for span in rendered.spans {
                    let mut s = span.clone();
                    s.style = value_style.patch(s.style);
                    spans.push(s);
                }
                if pad > 0 {
                    spans.push(Span::styled(" ".repeat(pad), value_style));
                }
            }

            lines.push(Line::from(spans));
        }

        let block = Block::bordered().title(format!(
            " Generate {}",
            if self.generating {
                "[working…]".to_string()
            } else {
                String::new()
            }
        ));
        frame.render_widget(Paragraph::new(Text::from(lines)).block(block), area);
    }

    fn draw_edit_tab(&mut self, frame: &mut Frame, area: Rect) {
        let layout = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        self.draw_edit_form(frame, layout[0]);
        self.draw_logs(frame, layout[1]);
    }

    fn draw_edit_form(&mut self, frame: &mut Frame, area: Rect) {
        let inner_w = area.width.saturating_sub(2) as usize;
        let label_w = 16usize;
        let value_w = inner_w.saturating_sub(label_w + 3).max(1);

        let mut lines: Vec<Line> = Vec::with_capacity(EDIT_ROWS + 3);
        for row in 0..EDIT_ROWS {
            let focused = self.edit_focus == row;
            let editing_here = self.edit_editing == Some(row);

            let label_style = if focused {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let mut spans: Vec<Span> = Vec::new();
            let label = format!("{:<label_w$}", Self::edit_row_label(row));
            spans.push(Span::styled(label, label_style));
            spans.push(Span::raw("  "));

            if row == EDIT_ROW_SUBMIT {
                let btn = if self.generating {
                    " [ Working… ] "
                } else {
                    " [ Edit ] "
                };
                let style = if focused {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                };
                spans.push(Span::styled(btn, style));
            } else {
                // text field
                let content = if editing_here {
                    self.editor.content.clone()
                } else {
                    self.edit_text_field(row).to_string()
                };
                let cursor = if editing_here {
                    self.editor.cursor
                } else {
                    content.len()
                };
                let (rendered, rendered_chars) = render_value_line(&content, cursor, editing_here);
                let pad = value_w.saturating_sub(rendered_chars);
                let value_style = if focused {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                for span in rendered.spans {
                    let mut s = span.clone();
                    s.style = value_style.patch(s.style);
                    spans.push(s);
                }
                if pad > 0 {
                    spans.push(Span::styled(" ".repeat(pad), value_style));
                }
            }

            lines.push(Line::from(spans));
        }

        // Reference images have no dedicated focus row: they are shown here
        // and edited with `r`. The provider receives them in listed order.
        lines.push(Line::from(""));
        {
            let content = if self.edit_refs_editing {
                self.editor.content.clone()
            } else {
                self.edit_refs.clone()
            };
            let shown = if !self.edit_refs_editing && content.is_empty() {
                "(none — press r to add)".to_string()
            } else {
                content
            };
            let cursor = if self.edit_refs_editing {
                self.editor.cursor.min(shown.len())
            } else {
                shown.len()
            };
            let (rendered, _) = render_value_line(&shown, cursor, self.edit_refs_editing);
            let mut spans = vec![Span::styled(
                format!("{:<label_w$}", "Refs"),
                Style::default().fg(Color::Gray),
            )];
            spans.push(Span::raw("  "));
            for span in rendered.spans {
                spans.push(span.clone());
            }
            lines.push(Line::from(spans));
        }

        let block = Block::bordered().title(format!(
            " Edit {}",
            if self.generating {
                "[working…]".to_string()
            } else {
                String::new()
            }
        ));
        frame.render_widget(Paragraph::new(Text::from(lines)).block(block), area);
        self.draw_source_picker(frame, area);
    }

    fn draw_source_picker(&mut self, frame: &mut Frame, area: Rect) {
        if self.edit_picker.is_none() {
            return;
        }
        if area.width < 24 || area.height < 8 {
            return;
        }
        // Read-only pass first so the borrow ends before the height writeback.
        let (entries, selected, scroll) = {
            let picker = self.edit_picker.as_ref().expect("picker open");
            (picker.entries.clone(), picker.selected, picker.scroll)
        };
        if entries.is_empty() {
            return;
        }
        let len = entries.len();
        let width = (area.width * 2 / 3).clamp(20, area.width);
        let height = ((len.max(3) + 2) as u16).min(area.height).max(5);
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        let popup = Rect::new(x, y, width, height);
        let visible = height.saturating_sub(2) as usize;
        if visible == 0 {
            return;
        }
        let start = scroll.min(len.saturating_sub(1));
        let mut end = (start + visible).min(len);
        if end < len {
            end = end.saturating_sub(1).max(start);
        }
        if let Some(picker) = self.edit_picker.as_mut() {
            picker.height = visible;
        }
        let mut lines: Vec<Line> = Vec::new();
        for (i, name) in entries[start..end].iter().enumerate() {
            if start + i == selected {
                lines.push(Line::from(vec![
                    Span::styled(
                        "▶ ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        name.clone(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("  {name}"),
                    Style::default().fg(Color::Gray),
                )));
            }
        }
        if end < len {
            lines.push(Line::from(Span::styled(
                format!("… {} more", len - end),
                Style::default().fg(Color::DarkGray),
            )));
        }
        let block = Block::bordered().title(" Select source image ");
        frame.render_widget(Paragraph::new(Text::from(lines)).block(block), popup);
    }

    fn draw_logs(&mut self, frame: &mut Frame, area: Rect) {
        let visible = area.height.saturating_sub(2) as usize;
        let start = self.logs.len().saturating_sub(visible);
        let lines: Vec<Line> = self.logs[start..]
            .iter()
            .map(|l| {
                let style = match l.kind {
                    LogKind::Success => Style::default().fg(Color::Green),
                    LogKind::Error => Style::default().fg(Color::Red),
                    LogKind::Warn => Style::default().fg(Color::Yellow),
                    LogKind::Info => Style::default().fg(Color::Cyan),
                };
                Line::from(Span::styled(l.text.clone(), style))
            })
            .collect();
        let block = Block::bordered().title(" Log ");
        if lines.is_empty() {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "No output yet.",
                    Style::default().fg(Color::DarkGray),
                )))
                .block(block),
                area,
            );
        } else {
            frame.render_widget(Paragraph::new(Text::from(lines)).block(block), area);
        }
    }

    fn draw_engines_tab(&mut self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();
        let header = format!(
            "{:<22} {:<14} {:<30} {:<24}",
            "Engine Name", "API Key Set", "Base URL", "Default Model"
        );
        lines.push(Line::from(Span::styled(
            header,
            Style::default().add_modifier(Modifier::BOLD).underlined(),
        )));

        let names = self.settings.engine_names();
        if names.is_empty() {
            lines.push(Line::from(Span::styled(
                "No engines configured. Check your .env file.",
                Style::default().fg(Color::Yellow),
            )));
        } else {
            for name in names {
                let cfg = &self.settings.engines[&name];
                let key_status = if cfg.key_set() {
                    "✅ Set"
                } else {
                    "⚠️ Not Set"
                };
                let base_url = cfg
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "N/A (Official OpenAI)".to_string());
                let model = cfg
                    .model
                    .clone()
                    .unwrap_or_else(|| "Not specified".to_string());
                let row = format!(
                    "{:<22} {:<14} {:<30} {:<24}",
                    name, key_status, base_url, model
                );
                let mut spans = Vec::new();
                // color columns
                let name_style = Style::default().fg(Color::Cyan);
                let key_style = if cfg.key_set() {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Yellow)
                };
                spans.push(Span::styled(format!("{:<22}", name), name_style));
                spans.push(Span::styled(format!("{:<14}", key_status), key_style));
                spans.push(Span::styled(
                    format!("{:<30}", base_url),
                    Style::default().fg(Color::Green),
                ));
                spans.push(Span::styled(model, Style::default().fg(Color::Yellow)));
                lines.push(Line::from(spans));
                let _ = row;
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press 'f' to fetch available models for engines with a configured key.",
            Style::default().fg(Color::DarkGray),
        )));

        if let Some(models) = self.render_models() {
            lines.extend(models);
        }

        let block = Block::bordered().title(" Engines ");
        frame.render_widget(Paragraph::new(Text::from(lines)).block(block), area);
    }

    fn render_models(&self) -> Option<Vec<Line<'static>>> {
        if self.engines_models.is_empty() {
            return None;
        }
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "📚 Available models:",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )),
        ];
        let mut keys: Vec<&String> = self.engines_models.keys().collect();
        keys.sort();
        for name in keys {
            lines.push(Line::from(Span::styled(
                format!("● {name}"),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            let ids = &self.engines_models[name];
            if ids.is_empty() {
                lines.push(Line::from(Span::styled(
                    "   (none)",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                for id in ids {
                    lines.push(Line::from(Span::styled(
                        format!("   • {id}"),
                        Style::default().fg(Color::Gray),
                    )));
                }
            }
        }
        Some(lines)
    }

    fn fetch_models(&mut self) {
        if self.fetching_models {
            return;
        }
        let mut engines: Vec<(String, Option<String>, String)> = Vec::new();
        for name in self.settings.engine_names() {
            let cfg = &self.settings.engines[&name];
            if cfg.key_set() {
                engines.push((name.clone(), cfg.base_url.clone(), cfg.api_key.clone()));
            }
        }
        if engines.is_empty() {
            self.status = "No engines with a configured API key.".to_string();
            self.push_log(
                LogKind::Warn,
                "No engines with a configured API key to fetch models.".to_string(),
            );
            return;
        }
        self.fetching_models = true;
        self.status = "Fetching models…".to_string();
        self.push_log(LogKind::Info, "Fetching available models…".to_string());
        let (tx, rx) = mpsc::unbounded_channel::<Vec<(String, Result<Vec<String>, String>)>>();
        tokio::spawn(async move {
            let mut all = Vec::new();
            for (name, base_url, api_key) in engines {
                let r = crate::provider::fetch_models(base_url.as_deref(), &api_key).await;
                all.push((name, r.map_err(|e| e.to_string())));
            }
            let _ = tx.send(all);
        });
        let rx = rx;
        self.models_rx = Some(rx);
    }

    fn drain_models(&mut self) {
        if let Some(rx) = &mut self.models_rx {
            match rx.try_recv() {
                Ok(all) => {
                    self.fetching_models = false;
                    self.models_rx = None;
                    self.engines_models.clear();
                    for (name, res) in all {
                        match res {
                            Ok(ids) => {
                                let filtered: Vec<String> = ids
                                    .into_iter()
                                    .filter(|m| crate::cli::is_image_model(m))
                                    .collect();
                                let count = filtered.len();
                                self.engines_models.insert(name.clone(), filtered);
                                self.push_log(
                                    LogKind::Info,
                                    format!("Loaded {count} model(s) for '{name}'."),
                                );
                            }
                            Err(e) => {
                                self.push_log(
                                    LogKind::Error,
                                    format!("Failed to fetch models for '{name}': {e}"),
                                );
                            }
                        }
                    }
                    self.status = "Models loaded".to_string();
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
                Err(_) => {
                    self.fetching_models = false;
                    self.models_rx = None;
                    self.status = "Model fetch ended".to_string();
                }
            }
        }
    }

    fn draw_about_tab(&mut self, frame: &mut Frame, area: Rect) {
        let text = Text::from(vec![
            Line::from(Span::styled(
                "🎨 Imagai",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "A CLI/TUI tool to generate images using various AI APIs.",
                Style::default().fg(Color::White),
            )),
            Line::from("Supports OpenAI-compatible backends: DALL-E, GPT-image, Gemini/Imagen,"),
            Line::from("Stable Diffusion / Stability models, OpenRouter vision chat, and more."),
            Line::from(""),
            Line::from(Span::styled(
                "Configuration:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  IMAGAI__DEFAULT_ENGINE=<engine name>"),
            Line::from("  IMAGAI__ENGINES__<NAME>__API_KEY=<key>"),
            Line::from("  IMAGAI__ENGINES__<NAME>__BASE_URL=<url>"),
            Line::from("  IMAGAI__ENGINES__<NAME>__MODEL=<model>"),
            Line::from("  IMAGAI__OUTPUT_DIR=<dir>   (default: generated_images)"),
            Line::from(""),
            Line::from(Span::styled(
                "CLI usage:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  imagai generate -p 'a cat wearing a hat' --engine openai_dalle3"),
            Line::from("  imagai generate -p 'futuristic city' --auto-filename"),
            Line::from("  imagai edit --image <path> -p \"make it sunset\""),
            Line::from("  imagai list-engines [--all]"),
            Line::from("  imagai tui"),
        ]);
        let block = Block::bordered().title(" About ");
        frame.render_widget(Paragraph::new(text).block(block), area);
    }

    fn draw_footer(&mut self, frame: &mut Frame, area: Rect) {
        let keys = tab_footer_hints(
            self.tab,
            self.editing.is_some(),
            self.edit_picker.is_some(),
            self.edit_editing.is_some() || self.edit_refs_editing,
        );
        let keys = format!("{keys}   |  {}", self.status);
        let style = Style::default().fg(Color::DarkGray);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(keys, style))).alignment(Alignment::Left),
            area,
        );
    }
}

/// The key hints for a tab. Extracted from `draw_footer` so the per-tab match
/// is exhaustive over 0..=4 with no catch-all arm (T-02-08) and is testable
/// without a terminal.
fn tab_footer_hints(tab: usize, editing: bool, picker_open: bool, field_editing: bool) -> String {
    match tab_surface(tab) {
        Some(TabSurface::Generate) => {
            if editing {
                "Esc done | ←/→ move | Backspace del | type to insert".to_string()
            } else {
                "q Quit | Tab switch tab | ↑/↓ navigate | Enter edit/Generate | Space toggle"
                    .to_string()
            }
        }
        Some(TabSurface::Edit) => {
            if picker_open {
                "↑/↓ move | Enter select | Esc cancel".to_string()
            } else if field_editing {
                "Esc done | ←/→ move | Backspace del | type to insert".to_string()
            } else {
                "q Quit | Tab switch tab | ↑/↓ navigate | Enter edit/Edit | e edit source path | Space toggle"
                    .to_string()
            }
        }
        Some(TabSurface::History) => HISTORY_FOOTER_HINT.to_string(),
        Some(TabSurface::Engines) => "q Quit | Tab switch tab | f fetch models".to_string(),
        Some(TabSurface::About) => "q Quit | Tab switch tab".to_string(),
        None => String::new(),
    }
}

/// Flatten the view's trees depth-first, in the order the CLI renderer walks
/// them: roots newest-first and children oldest-first, which is the order
/// `history::load_view` already built (02-02-PLAN.md — one shared
/// `history::HistoryView`, one output order). Each row uses the same
/// connector/indent/`✗ `/id8/prompt-field grammar as `render_history_text`
/// because both call `history::history_row_text`.
fn flatten_nodes(nodes: &[history::HistoryNode], depth: usize, out: &mut Vec<HistoryRow>) {
    // The same shared cap `history::load_view` applies, so the flattener and
    // the read path agree on how deep a hand-editable sidecar may go (CR-04).
    if depth >= MAX_TREE_DEPTH {
        return;
    }
    let last = nodes.len().saturating_sub(1);
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == last;
        out.push(HistoryRow {
            line: Line::from(history::history_row_text(node, depth, is_last)),
            node: Some(node.clone()),
        });
        flatten_nodes(&node.children, depth + 1, out);
    }
}

/// The flattened rows of a whole view — pure, so the tree grammar and the
/// scroll window are unit-testable without a terminal.
fn history_rows(view: &history::HistoryView) -> Vec<HistoryRow> {
    let mut rows = Vec::new();
    flatten_nodes(&view.roots, 0, &mut rows);
    rows
}

/// The trailing `… {n} more` line when a list is clipped; `None` otherwise.
fn remaining_line(shown: usize, total: usize) -> Option<String> {
    (shown < total).then(|| format!("… {} more", total - shown))
}

/// Render a text value line with an optional block cursor.
/// Returns the rendered line plus the number of chars it occupies.
fn render_value_line(content: &str, cursor: usize, editing: bool) -> (Line<'static>, usize) {
    let cursor = cursor.min(content.len());
    if !editing {
        let chars = content.chars().count();
        return (Line::from(Span::raw(content.to_string())), chars);
    }
    let (before, rest) = content.split_at(cursor);
    let (ch, after) = match rest.chars().next() {
        Some(c) => {
            let clen = c.len_utf8();
            (&rest[..clen], &rest[clen..])
        }
        None => (" ", ""),
    };
    let spans = vec![
        Span::raw(before.to_string()),
        Span::styled(
            ch.to_string(),
            Style::default()
                .bg(Color::White)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(after.to_string()),
    ];
    let chars = content.chars().count();
    (Line::from(spans), chars)
}

pub async fn run(settings: &Settings) -> anyhow::Result<()> {
    let mut app = App::new(settings);
    let mut terminal = ratatui::init();
    let result = app.run_loop(&mut terminal).await;
    ratatui::restore();
    result
}

impl App {
    async fn run_loop(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        loop {
            terminal.draw(|f| self.draw(f))?;
            self.drain_messages();
            self.drain_models();

            if !event::poll(Duration::from_millis(100))? {
                continue;
            }
            let ev = event::read()?;
            match ev {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    // Tab key switches tabs.
                    let code = key.code;
                    if code == KeyCode::Tab
                        && self.editing.is_none()
                        && self.edit_editing.is_none()
                        && self.edit_picker.is_none()
                        && !self.edit_refs_editing
                    {
                        self.tab = (self.tab + 1) % TABS.len();
                        if self.tab == 2 {
                            self.load_history();
                        }
                        continue;
                    }
                    if code == KeyCode::BackTab
                        && self.editing.is_none()
                        && self.edit_editing.is_none()
                        && self.edit_picker.is_none()
                        && !self.edit_refs_editing
                    {
                        self.tab = (self.tab + TABS.len() - 1) % TABS.len();
                        continue;
                    }
                    // 'f' fetches models on the engines tab (index 3).
                    if code == KeyCode::Char('f') && self.tab == 3 && self.editing.is_none() {
                        self.fetch_models();
                        continue;
                    }
                    if self.handle_key(code) {
                        return Ok(());
                    }
                }
                Event::Paste(text)
                    if self.editing.is_some()
                        || self.edit_editing.is_some()
                        || self.edit_refs_editing =>
                {
                    for c in text.chars() {
                        self.editor.insert_char(c);
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn edit_test_settings(dir: &std::path::Path) -> Settings {
        Settings {
            output_dir: dir.to_path_buf(),
            default_engine: Some("mock".to_string()),
            engines: HashMap::from([(
                "mock".to_string(),
                crate::config::EngineConfig {
                    api_key: "test".to_string(),
                    base_url: None,
                    model: Some("gpt-image-1".to_string()),
                    format: None,
                },
            )]),
        }
    }

    #[test]
    fn edit_row_table_has_no_unreachable_index() {
        let tmp = TempDir::new().unwrap();
        let settings = edit_test_settings(tmp.path());
        let mut app = App::new(&settings);
        for row in 0..EDIT_ROWS {
            assert!(
                !App::edit_row_label(row).is_empty(),
                "row {row} has a label"
            );
            let _ = app.edit_text_field(row);
        }
        for row in 0..EDIT_ROW_SUBMIT {
            let _ = app.edit_text_field_mut(row);
        }
    }

    #[test]
    fn edit_row_labels_name_the_edit_fields() {
        let labels: Vec<_> = (0..EDIT_ROWS).map(App::edit_row_label).collect();
        assert_eq!(labels, vec!["Source", "Change", "Engine", "Output", "Edit"]);
    }

    /// Plain text of a history row, so the built lines can be asserted.
    fn row_text(row: &HistoryRow) -> String {
        row.line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>()
    }

    fn row_texts(rows: &[HistoryRow]) -> Vec<String> {
        rows.iter().map(row_text).collect()
    }

    /// Seed a sidecar into `dir` and load the real view through it, so the
    /// tests exercise the store rather than a hand-built struct.
    fn seeded_view(dir: &std::path::Path) -> history::HistoryView {
        use base64::Engine;
        std::fs::write(
            dir.join(".imagai-history.json"),
            serde_json::to_string(&[
                serde_json::json!({
                    "id": "1a2b3c4d", "parent": null, "filename": "cat.png",
                    "prompt": "a cat wearing a hat", "model": "dall-e-3",
                    "created": "2026-10-09T11:40:00"
                }),
                serde_json::json!({
                    "id": "2b3c4d5e", "parent": "1a2b3c4d", "filename": "cat-edit.png",
                    "prompt": "make it sunset", "model": "gpt-image-1",
                    "created": "2026-10-09T11:41:00"
                }),
                serde_json::json!({
                    "id": "3c4d5e6f", "parent": "1a2b3c4d", "filename": "cat-edit-1.png",
                    "prompt": "add a party hat", "model": "gpt-image-1",
                    "created": "2026-10-09T11:42:00"
                }),
            ])
            .expect("serialize store"),
        )
        .expect("seed sidecar");
        // One PNG so the first node reads as present; the other two are missing.
        let png = base64::engine::general_purpose::STANDARD
            .decode(PIXEL_PNG_B64)
            .expect("decode PIXEL_PNG_B64");
        std::fs::write(dir.join("cat.png"), png).expect("seed png");
        history::load_view(dir).expect("view")
    }

    #[test]
    fn tabs_has_history_at_index_two() {
        assert_eq!(TABS.len(), 5, "Generate, Edit, History, Engines, About");
        assert_eq!(TABS[2], "History");
        // Both dispatchers cover 0..=4 explicitly, with no catch-all arm
        // (T-02-08); an out-of-range index has no surface rather than About.
        for (tab, name) in TABS.iter().enumerate() {
            assert!(
                tab_surface(tab).is_some(),
                "tab {tab} ({name}) has a renderer"
            );
        }
        assert!(
            tab_surface(5).is_none(),
            "no surface for an out-of-range tab"
        );
        assert_eq!(tab_surface(2), Some(TabSurface::History));
        assert_eq!(tab_surface(4), Some(TabSurface::About));
        // The History tab's footer hint is the contract string, and the other
        // tabs keep theirs.
        assert_eq!(
            tab_footer_hints(2, false, false, false),
            HISTORY_FOOTER_HINT
        );
        assert_eq!(
            tab_footer_hints(3, false, false, false),
            "q Quit | Tab switch tab | f fetch models"
        );
        assert_eq!(
            tab_footer_hints(4, false, false, false),
            "q Quit | Tab switch tab"
        );
    }

    #[test]
    fn history_tree_lines_use_the_contract_connectors() {
        let tmp = TempDir::new().unwrap();
        let view = seeded_view(tmp.path());
        let rows = history_rows(&view);
        let texts = row_texts(&rows);
        assert_eq!(texts.len(), 3, "root plus two children");
        // Depth 0: no connector.
        assert!(
            texts[0].starts_with("1a2b3c4d  "),
            "no connector at depth 0: {:?}",
            texts[0]
        );
        // Non-last child: three spaces of indent plus `+- `.
        assert!(
            texts[1].starts_with("   +- ✗ 2b3c4d5e"),
            "non-last connector, then the missing marker: {:?}",
            texts[1]
        );
        // Last child: three spaces of indent plus `` `- ``.
        assert!(
            texts[2].starts_with("   `- ✗ 3c4d5e6f"),
            "last-child connector, then the missing marker: {:?}",
            texts[2]
        );
        // The prompt column is a fixed 32-character field, and the timestamp
        // renders as YYYY-MM-DD HH:MM.
        assert!(texts[0].contains("a cat wearing a hat"));
        assert!(texts[0].ends_with("2026-10-09 11:40"), "{:?}", texts[0]);
        assert!(texts[0].contains("cat.png"), "{:?}", texts[0]);
    }

    #[test]
    fn history_tree_marks_missing_nodes() {
        let tmp = TempDir::new().unwrap();
        let view = seeded_view(tmp.path());
        let texts = row_texts(&history_rows(&view));
        // cat.png exists, so the root is not marked; the two children are.
        assert!(!texts[0].contains("✗ "), "root present: {:?}", texts[0]);
        for text in &texts[1..] {
            let marker = text.find("✗ ").expect("missing marker");
            let id = text
                .find("2b3c")
                .or_else(|| text.find("3c4d"))
                .expect("id8");
            assert!(marker < id, "✗ sits before the id8: {:?}", text);
        }
    }

    #[test]
    fn history_scroll_marks_remaining_rows() {
        let tmp = TempDir::new().unwrap();
        let view = seeded_view(tmp.path());
        let rows = history_rows(&view);
        // One row visible out of three: two remain.
        assert_eq!(remaining_line(1, rows.len()), Some("… 2 more".to_string()));
        // Nothing clipped means no trailing line at all.
        assert_eq!(remaining_line(rows.len(), rows.len()), None);
    }

    #[test]
    fn history_enter_forks_selected_node_into_edit() {
        let tmp = TempDir::new().unwrap();
        seed_png(tmp.path(), "cat.png");
        std::fs::write(
            tmp.path().join(".imagai-history.json"),
            serde_json::to_string(&[serde_json::json!({
                "id": "1a2b3c4d", "parent": null, "filename": "cat.png",
                "prompt": "a cat", "model": "dall-e-3", "created": "2026-10-09T11:40:00"
            })])
            .expect("serialize store"),
        )
        .expect("seed sidecar");
        let settings = edit_test_settings(tmp.path());
        let mut app = App::new(&settings);
        app.load_history();
        assert!(
            !app.handle_history_key(KeyCode::Enter),
            "Enter does not quit"
        );
        assert_eq!(app.edit_source, "cat.png");
        assert_eq!(app.tab, 1);
        let last = app.logs.last().expect("info logged");
        assert_eq!(last.text, "Source image: cat.png");
    }

    #[test]
    fn history_enter_refuses_missing_node() {
        let tmp = TempDir::new().unwrap();
        let view = seeded_view(tmp.path());
        assert!(view.missing > 0, "the fixture has missing nodes");
        let settings = edit_test_settings(tmp.path());
        let mut app = App::new(&settings);
        app.tab = 2;
        app.load_history();
        // Select the first child, whose file is gone.
        app.history_move(1);
        app.edit_source = "keep-me.png".to_string();
        assert!(
            !app.handle_history_key(KeyCode::Enter),
            "Enter does not quit"
        );
        let last = app.logs.last().expect("error logged");
        assert!(
            last.text.contains("no longer in"),
            "contract sentence: {}",
            last.text
        );
        assert_eq!(app.edit_source, "keep-me.png", "source untouched");
        assert_eq!(app.tab, 2, "stays on the History tab");
    }

    #[test]
    fn picker_history_entry_moves_to_history_tab() {
        let tmp = TempDir::new().unwrap();
        seed_png(tmp.path(), "seed_a.png");
        let settings = edit_test_settings(tmp.path());
        let mut app = App::new(&settings);
        app.edit_source = "keep-me.png".to_string();
        app.open_source_picker();
        let entries = app
            .edit_picker
            .as_ref()
            .expect("picker open")
            .entries
            .clone();
        assert_eq!(
            entries[0], PICK_FROM_HISTORY,
            "the history row is the picker's first entry"
        );
        assert!(
            !app.handle_picker_key(KeyCode::Enter),
            "Enter does not quit"
        );
        assert_eq!(app.tab, 2);
        assert!(app.history_pick_for_edit);
        assert_eq!(
            app.edit_source, "keep-me.png",
            "the picker hands off without touching the source"
        );
    }

    #[test]
    fn history_enter_completes_pending_pick() {
        let tmp = TempDir::new().unwrap();
        seed_png(tmp.path(), "cat.png");
        std::fs::write(
            tmp.path().join(".imagai-history.json"),
            serde_json::to_string(&[serde_json::json!({
                "id": "1a2b3c4d", "parent": null, "filename": "cat.png",
                "prompt": "a cat", "model": "dall-e-3", "created": "2026-10-09T11:40:00"
            })])
            .expect("serialize store"),
        )
        .expect("seed sidecar");
        let settings = edit_test_settings(tmp.path());
        let mut app = App::new(&settings);
        app.open_history_pick();
        assert!(app.history_pick_for_edit);
        assert!(
            !app.handle_history_key(KeyCode::Enter),
            "Enter does not quit"
        );
        assert_eq!(app.edit_source, "cat.png");
        assert_eq!(app.tab, 1);
        assert!(!app.history_pick_for_edit, "the flag is cleared");
    }

    #[test]
    fn picker_and_history_escape_leave_source_unchanged() {
        let tmp = TempDir::new().unwrap();
        seed_png(tmp.path(), "cat.png");
        let settings = edit_test_settings(tmp.path());
        let mut app = App::new(&settings);
        app.edit_source = "keep-me.png".to_string();

        // Esc in the picker.
        app.open_source_picker();
        app.picker_move(1);
        assert!(!app.handle_picker_key(KeyCode::Esc), "Esc does not quit");
        assert_eq!(app.edit_source, "keep-me.png");
        assert!(app.edit_picker.is_none());

        // Esc on the History tab with a pick pending.
        app.open_history_pick();
        assert!(!app.handle_history_key(KeyCode::Esc), "Esc does not quit");
        assert_eq!(app.edit_source, "keep-me.png");
        assert_eq!(app.tab, 1, "back to the Edit tab");
        assert!(!app.history_pick_for_edit);
    }

    fn write_file(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, b"x").unwrap();
        p
    }

    fn set_mtime_ago(p: &std::path::Path, secs: u64) {
        let t = std::time::SystemTime::now() - std::time::Duration::from_secs(secs);
        std::fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn picker_lists_newest_first() {
        let tmp = TempDir::new().unwrap();
        let a = write_file(tmp.path(), "seed_a.png");
        let b = write_file(tmp.path(), "seed_b.png");
        set_mtime_ago(&a, 60);
        set_mtime_ago(&b, 10);
        let settings = edit_test_settings(tmp.path());
        let mut app = App::new(&settings);
        app.open_source_picker();
        let picker = app.edit_picker.as_ref().expect("picker open");
        assert_eq!(
            picker.entries,
            vec![
                PICK_FROM_HISTORY.to_string(),
                "seed_b.png".to_string(),
                "seed_a.png".to_string()
            ],
            "the history row leads, then the images newest-first"
        );
        assert_eq!(picker.origin, PickerOrigin::Source);
    }

    #[test]
    fn picker_selection_lands_in_source_field() {
        let tmp = TempDir::new().unwrap();
        let a = write_file(tmp.path(), "seed_a.png");
        let b = write_file(tmp.path(), "seed_b.png");
        set_mtime_ago(&a, 60);
        set_mtime_ago(&b, 10);
        let settings = edit_test_settings(tmp.path());
        let mut app = App::new(&settings);
        app.open_source_picker();
        app.picker_move(1);
        app.picker_move(1);
        app.picker_move(1);
        let selected = app.edit_picker.as_ref().expect("picker open").selected;
        assert_eq!(selected, 2, "selection clamps at the list end");
        let expected = app.edit_picker.as_ref().unwrap().entries[selected].clone();
        app.picker_select();
        assert_eq!(app.edit_source, expected);
        assert!(app.edit_picker.is_none());
    }

    #[test]
    fn picker_escape_leaves_source_unchanged() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "seed_a.png");
        write_file(tmp.path(), "seed_b.png");
        let settings = edit_test_settings(tmp.path());
        let mut app = App::new(&settings);
        app.edit_source = "keep-me.png".to_string();
        app.open_source_picker();
        app.picker_move(1);
        app.picker_cancel();
        assert_eq!(app.edit_source, "keep-me.png");
        assert!(app.edit_picker.is_none());
    }

    const PIXEL_PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    fn seed_png(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(PIXEL_PNG_B64)
            .unwrap();
        let p = dir.join(name);
        std::fs::write(&p, &bytes).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_modified(std::time::SystemTime::now())
            .unwrap();
        p
    }

    #[test]
    fn build_edit_request_rejects_empty_instruction() {
        let tmp = TempDir::new().unwrap();
        let settings = edit_test_settings(tmp.path());
        let mut app = App::new(&settings);
        app.edit_prompt = "   ".to_string();
        assert!(app.build_edit_request().is_none());
        let last = app.logs.last().expect("error logged");
        assert!(
            last.text.contains("Change instruction cannot be empty"),
            "{}",
            last.text
        );
    }

    #[test]
    fn build_edit_request_carries_source_and_refs_in_order() {
        let tmp = TempDir::new().unwrap();
        let source = seed_png(tmp.path(), "source.png");
        seed_png(tmp.path(), "ref-a.png");
        seed_png(tmp.path(), "ref-b.png");
        // The auto-pick takes the newest file, so stamp the source newer
        // than the refs instead of relying on write timing.
        let future = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        std::fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_modified(future)
            .unwrap();
        let settings = edit_test_settings(tmp.path());
        let mut app = App::new(&settings);
        app.edit_prompt = "make it sunset".to_string();
        app.edit_refs = "ref-a.png, ref-b.png,".to_string();
        let req = app.build_edit_request().expect("request built");
        assert_eq!(
            req.source_image.unwrap(),
            source.to_string_lossy().to_string()
        );
        assert_eq!(req.ref_images.len(), 2);
        assert_eq!(
            req.ref_images[0],
            tmp.path().join("ref-a.png").to_string_lossy().to_string()
        );
        assert_eq!(
            req.ref_images[1],
            tmp.path().join("ref-b.png").to_string_lossy().to_string()
        );
        assert_eq!(req.prompt, "make it sunset");
    }

    #[test]
    fn build_edit_request_rejects_missing_reference() {
        let tmp = TempDir::new().unwrap();
        seed_png(tmp.path(), "source.png");
        let settings = edit_test_settings(tmp.path());
        let mut app = App::new(&settings);
        app.edit_prompt = "make it sunset".to_string();
        app.edit_refs = "absent.png".to_string();
        assert!(app.build_edit_request().is_none());
        let last = app.logs.last().expect("error logged");
        assert!(last.text.contains("absent.png"), "{}", last.text);
    }

    #[test]
    fn build_edit_request_rejects_unknown_engine() {
        let tmp = TempDir::new().unwrap();
        seed_png(tmp.path(), "source.png");
        let settings = edit_test_settings(tmp.path());
        let mut app = App::new(&settings);
        app.edit_prompt = "make it sunset".to_string();
        app.edit_engine = "nope".to_string();
        assert!(app.build_edit_request().is_none());
        let last = app.logs.last().expect("error logged");
        assert!(last.text.contains("not configured"), "{}", last.text);
    }
}
