use std::collections::HashMap;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph};
use ratatui::DefaultTerminal;
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::config::Settings;
use crate::core::generate_image_core;
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

const TABS: [&str; 4] = ["Generate", "Edit", "Engines", "About"];

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

struct PickerState {
    entries: Vec<String>,
    selected: usize,
    scroll: usize,
    height: usize,
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
            for (i, r) in results.iter().enumerate() {
                if let Some(e) = &r.error {
                    self.push_log(LogKind::Error, format!("Image {} error: {e}", i + 1));
                } else if let Some(p) = &r.saved_path {
                    self.push_log(LogKind::Success, format!("✅ Image {} saved to {p}", i + 1));
                } else if let Some(u) = &r.image_url {
                    self.push_log(LogKind::Info, format!("Image {} URL: {u}", i + 1));
                } else if let Some(t) = &r.text_content {
                    self.push_log(LogKind::Info, format!("💬 Model response: {t}"));
                } else {
                    self.push_log(LogKind::Warn, format!("Image {}: no payload.", i + 1));
                }
            }
            self.status = if results.is_empty() {
                "Generation finished (no results)".to_string()
            } else {
                "Generation finished".to_string()
            };
        }
    }

    // ---- event handling ----

    fn handle_key(&mut self, code: KeyCode) -> bool {
        if self.edit_picker.is_some() {
            return self.handle_picker_key(code);
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
        let entries: Vec<String> = crate::utils::list_output_images(&self.settings.output_dir)
            .into_iter()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            })
            .collect();
        if entries.is_empty() {
            self.push_log(
                LogKind::Warn,
                "No images found in the output directory — generate one first".to_string(),
            );
            return;
        }
        self.edit_picker = Some(PickerState {
            entries,
            selected: 0,
            scroll: 0,
            height: 0,
        });
    }

    fn picker_move(&mut self, delta: isize) {
        let Some(picker) = self.edit_picker.as_mut() else {
            return;
        };
        if picker.entries.is_empty() {
            return;
        }
        let len = picker.entries.len();
        picker.selected = (picker.selected as isize + delta).clamp(0, len as isize - 1) as usize;
        let height = picker.height.max(1);
        if picker.selected < picker.scroll {
            picker.scroll = picker.selected;
        } else if picker.selected >= picker.scroll + height {
            picker.scroll = picker.selected + 1 - height;
        }
        let max_scroll = len.saturating_sub(picker.height);
        picker.scroll = picker.scroll.min(max_scroll);
    }

    fn picker_select(&mut self) {
        let Some(picker) = self.edit_picker.as_ref() else {
            return;
        };
        if picker.entries.is_empty() {
            self.edit_picker = None;
            return;
        }
        let name = picker.entries[picker.selected.min(picker.entries.len() - 1)].clone();
        self.edit_source = name.clone();
        if self.edit_editing == Some(EDIT_ROW_SOURCE) {
            self.edit_editing = None;
        }
        self.edit_picker = None;
        self.push_log(LogKind::Info, format!("Source image: {name}"));
    }

    fn picker_cancel(&mut self) {
        self.edit_picker = None;
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
        match self.tab {
            0 => self.draw_generate_tab(frame, layout[1]),
            1 => self.draw_edit_tab(frame, layout[1]),
            2 => self.draw_engines_tab(frame, layout[1]),
            _ => self.draw_about_tab(frame, layout[1]),
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
        let mut keys = match self.tab {
            0 => {
                if self.editing.is_some() {
                    "Esc done | ←/→ move | Backspace del | type to insert".to_string()
                } else {
                    "q Quit | Tab switch tab | ↑/↓ navigate | Enter edit/Generate | Space toggle"
                        .to_string()
                }
            }
            1 => {
                if self.edit_picker.is_some() {
                    "↑/↓ move | Enter select | Esc cancel".to_string()
                } else if self.edit_editing.is_some() || self.edit_refs_editing {
                    "Esc done | ←/→ move | Backspace del | type to insert".to_string()
                } else {
                    "q Quit | Tab switch tab | ↑/↓ navigate | Enter edit/Edit | e edit source path | Space toggle"
                        .to_string()
                }
            }
            2 => "q Quit | Tab switch tab | f fetch models".to_string(),
            _ => "q Quit | Tab switch tab".to_string(),
        };
        keys.push_str(&format!("   |  {}", self.status));
        let style = Style::default().fg(Color::DarkGray);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(keys, style))).alignment(Alignment::Left),
            area,
        );
    }
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
                    // 'f' fetches models on the engines tab.
                    if code == KeyCode::Char('f') && self.tab == 2 && self.editing.is_none() {
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
            vec!["seed_b.png".to_string(), "seed_a.png".to_string()]
        );
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
        let selected = app.edit_picker.as_ref().expect("picker open").selected;
        assert_eq!(selected, 1, "selection clamps at the list end");
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
