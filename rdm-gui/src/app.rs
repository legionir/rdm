//! Application root: polls the metadata database, renders the panels and
//! turns [`UiAction`]s into `rdm` library calls.
//!
//! Layout (top to bottom): toolbar -> optional Queue/Settings sidebars ->
//! download list -> optional Events/App-log box -> status bar. Details live in
//! a modal opened by double-clicking a row.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::Context;

use rdm::models::DownloadState;

use crate::backend::{Backend, BackendEvent};
use crate::clipboard;
use crate::dropzone::DropZone;
use crate::logging::LogControl;
use crate::platform::ImportBus;
use crate::tray::{Tray, TrayCommand};
use crate::settings::SettingsStore;
use crate::state::{DetailTab, FooterPanel, GuiState, PendingConfirm, UiAction};

/// How often the app asks for a frame while the window is on screen and a tray
/// exists (ms). Small enough to feel instant, large enough to cost nothing. The
/// tray's relay threads add their own wake-up per click, so this is the *floor*,
/// not the reaction time.
const TRAY_HEARTBEAT_MS: u64 = 250;

/// How often it asks while the window is parked in the tray (ms).
///
/// The window is invisible to the user — and the point of the round-3 fix is
/// that it does not need to be *running* frames to answer its tray, because the
/// relay threads carry the window commands out themselves. This slow tick is
/// what is left: enough for `frames::is_stale` to tell “idle” from “dead”, and
/// enough that the state-dependent commands are applied within a second.
const TRAY_HIDDEN_HEARTBEAT_MS: u64 = 1_000;

/// The background every viewport is cleared with.
///
/// eframe clears *every* window with this one colour, whatever is painted in it.
/// The app has two transparent windows — the floating drop target and (for the
/// duration of the animation) nothing else — so a semi-transparent clear left a
/// **dark square** around the drop target's circle instead of a floating mark.
/// `[0, 0, 0, 0]` contributes nothing; the opaque panels of the main window and
/// the settings/help windows paint their own area, so they are unchanged.
pub const CLEAR_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
use crate::theme::{self, components, Sizes, Spacing};
use crate::ux::{self, BulkAction, Confirm};
use crate::util;

pub struct RdmGuiApp {
    backend: Backend,
    settings: SettingsStore,
    state: GuiState,
    last_poll: Instant,
    last_selected: Option<i64>,
    last_detail: Option<i64>,
    last_footer: Option<FooterPanel>,
    applied_dark: Option<bool>,
    logging: Option<LogControl>,
    log_level: String,
    shutting_down: bool,
    /// Tray icon; `None` in a session without a tray host (also CI).
    tray: Option<Tray>,
    /// Links arriving from the tray or the drop target.
    imports: ImportBus,
    /// The floating drop target (“floating bottom”).
    drop_zone: DropZone,
    /// Set while the drop target should be on screen, mirrored into the settings.
    drop_target_shown: bool,
    /// `true` once “Quit” was chosen from the tray: the window close is then a
    /// real exit instead of another hide.
    quit_requested: bool,
    /// Ask the viewport to come back to the foreground (tray/import).
    reveal_requested: bool,
    /// The Settings window (its own OS window since the follow-up round).
    settings_window: crate::views::settings_view::SettingsWindow,
    /// The main window's OS handle was handed to `crate::windows` (asked once,
    /// from the first frame — `main.rs` asks the start-up context, and one of
    /// the two always has the window).
    window_handle_captured: bool,
}

impl RdmGuiApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        data_dir: PathBuf,
        data_dir_explicit: bool,
        logging: Option<LogControl>,
        forced_level: Option<&'static str>,
    ) -> anyhow::Result<Self> {
        let backend = Backend::new(&data_dir)?;
        let settings = SettingsStore::new(&data_dir, data_dir_explicit);
        let state = GuiState::new(
            settings.settings().to_request(),
            data_dir.display().to_string(),
        );
        let drop_target_shown = settings.settings().drop_target_enabled;
        let mut app = RdmGuiApp {
            backend,
            settings,
            state,
            last_poll: Instant::now(),
            last_selected: None,
            last_detail: None,
            last_footer: None,
            applied_dark: None,
            logging,
            log_level: String::new(),
            shutting_down: false,
            tray: None,
            imports: ImportBus::new(),
            drop_zone: DropZone::new(),
            drop_target_shown,
            quit_requested: false,
            reveal_requested: false,
            settings_window: crate::views::settings_view::SettingsWindow::new(),
            window_handle_captured: false,
        };
        // A `-v` flag on the command line wins over the settings file.
        let level = match forced_level {
            Some(level) => {
                app.settings.settings_mut().log_level = level.to_string();
                level.to_string()
            }
            None => app.settings.settings().log_level.clone(),
        };
        app.set_log_level(&level);
        app.state.push_log(
            "info",
            format!("metadata database: {}", app.backend.db_path().display()),
        );
        // Where to look after a report about the tray: the file keeps every
        // line, including the ones the tray relays write from their threads.
        // (`app.logging`: the parameter was moved into the app above.)
        if let Some(file) = app
            .logging
            .as_ref()
            .and_then(LogControl::log_file)
            .map(|path| path.display().to_string())
        {
            app.state.push_log("info", format!("log file: {file}"));
        }
        tracing::info!(
            "rdm-gui {} starting: data dir {}",
            env!("CARGO_PKG_VERSION"),
            data_dir.display()
        );

        // Tray: absent in sessions without a tray host; the app then closes
        // normally instead of hiding into a tray that is not there.
        //
        // The relay threads wake the UI with a repaint request — safe from any
        // thread, and it only schedules a frame. Their real escape hatch is the
        // OS-level window restore in `tray::deliver`, which needs no frame at
        // all (see `tray.rs` for the round-3 failure this covers).
        let wake_ctx = cc.egui_ctx.clone();
        let wake: Arc<crate::tray::Wake> = Arc::new(move || {
            wake_ctx.request_repaint_of(egui::ViewportId::ROOT);
        });
        app.tray = Tray::new(app.drop_target_shown, wake);
        if app.tray.is_none() {
            app.state
                .push_log("warn", "no system tray available — closing the window will quit");
        }

        app.refresh(true);
        Ok(app)
    }

    // ------------------------------------------------------------- data sync

    fn refresh(&mut self, force: bool) {
        let interval = Duration::from_millis(self.settings.settings().refresh_ms.max(100));
        if !force && self.last_poll.elapsed() < interval {
            return;
        }
        self.last_poll = Instant::now();
        match self.backend.list() {
            Ok(rows) => self.state.apply_rows(rows),
            Err(err) => self
                .state
                .push_log("error", format!("cannot read downloads: {err:#}")),
        }
        let _selection_changed = self.last_selected != self.state.selected;
        self.last_selected = self.state.selected;
        let detail_changed = self.last_detail != self.state.detail_id;
        self.last_detail = self.state.detail_id;

        // Details modal: only the tab that is visible needs fresh data.
        if let Some(id) = self.state.detail_id {
            let wants_chunks = matches!(self.state.detail_tab, DetailTab::Chunks);
            let wants_json = matches!(self.state.detail_tab, DetailTab::Json);
            if wants_chunks || detail_changed {
                self.state.chunks = self.backend.chunks(id).unwrap_or_default();
            }
            if wants_json || detail_changed {
                self.state.json = match self.backend.snapshot_json(id) {
                    Ok(json) => json,
                    Err(err) => format!("// {err:#}"),
                };
            }
        } else {
            self.state.chunks.clear();
            self.state.json.clear();
        }

        // Footer Events box shows the selected download's events.
        if self.state.footer_panel == Some(FooterPanel::Events) {
            if let Some(id) = self.state.selected {
                self.state.events = self.backend.events(id, 50).unwrap_or_default();
            } else {
                self.state.events.clear();
            }
        }
    }

    /// Apply a verbosity directive to the tracing subscriber.
    fn set_log_level(&mut self, level: &str) {
        if self.log_level == level {
            return;
        }
        self.log_level = level.to_string();
        let Some(control) = self.logging.clone() else {
            return;
        };
        match control.set_level(level) {
            Ok(()) => self
                .state
                .push_log("info", format!("engine log level set to {level}")),
            Err(err) => self
                .state
                .push_log("error", format!("invalid log level {level:?}: {err}")),
        }
    }

    /// Move captured `tracing` output into the App log box.
    fn drain_engine_logs(&mut self) {
        let Some(control) = self.logging.clone() else {
            return;
        };
        for line in control.buffer().drain() {
            self.state.push_engine_log(line.level, line.text);
        }
    }

    fn drain_backend_events(&mut self) {
        for event in self.backend.drain_events() {
            let level = match event {
                BackendEvent::Info(_) => "info",
                BackendEvent::Warn(_) => "warn",
                BackendEvent::Error(_) => "error",
            };
            let text = event.text().to_string();
            self.state.push_log(level, text);
        }
    }

    // ---------------------------------------------------------- action loop

    fn apply(&mut self, action: UiAction, ctx: &Context) {
        match action {
            UiAction::OpenAddDialog => {
                let mut form = self.settings.settings().to_request();
                form.url = self.state.form.url.clone();
                // “Copy a link, press New”: the clipboard fills the field when
                // the switch is on (Settings, Application).
                let mut prefilled = false;
                if self.settings.settings().clipboard_prefill {
                    if let Some(link) = clipboard::url() {
                        form.url = link;
                        prefilled = true;
                    }
                }
                self.state.form = form;
                self.state.form_error = None;
                self.state.show_add = true;
                self.state.focus_url = prefilled;
                if prefilled {
                    self.state
                        .push_log("info", "URL filled in from the clipboard");
                }
            }
            UiAction::SubmitNewDownload => {
                let request = self.state.form.clone();
                let limit = self.max_concurrent();
                match self.backend.start_download(&request, limit) {
                    Ok(_) => {
                        self.state.show_add = false;
                        self.state.form_error = None;
                        self.state.form.url.clear();
                        self.state.push_log("info", format!("queued {}", request.url));
                        self.refresh(true);
                    }
                    Err(err) => {
                        let text = format!("{err:#}");
                        self.state.form_error = Some(text.clone());
                        self.state.push_log("error", text);
                    }
                }
            }
            UiAction::Pause(id) => self.report(self.backend.pause(id)),
            UiAction::Cancel(id) => self.report(self.backend.cancel(id)),
            UiAction::Resume(id) => {
                let defaults = self.settings.settings().to_request();
                let limit = self.max_concurrent();
                match self.backend.resume(id, &defaults, limit) {
                    Ok(_) => self.state.push_log("info", format!("resuming #{id}")),
                    Err(err) => self.state.push_log("error", format!("{err:#}")),
                }
            }
            UiAction::Restart(id) => {
                self.state.pending_confirm = None;
                self.restart_now(id);
            }
            UiAction::AskRemove(id) => {
                let label = self.describe(id);
                if self.confirm_destructive() {
                    self.state.pending_confirm = Some(PendingConfirm::Remove { id, label });
                } else {
                    let purge = self.settings.settings().purge_on_remove;
                    self.report(self.backend.remove(id, purge));
                    self.refresh(true);
                }
            }
            UiAction::AskRestart(id) => {
                // The progress is discarded, so this asks before acting.
                let label = self.describe(id);
                if self.confirm_destructive() {
                    self.state.pending_confirm = Some(PendingConfirm::Restart { id, label });
                } else {
                    self.restart_now(id);
                }
            }
            UiAction::AskDropQueue => {
                let queued = self.state.queue.len();
                match ux::drop_all_confirm(queued) {
                    Confirm::None => self.drop_queue_now(),
                    _ => {
                        self.state.pending_confirm =
                            Some(PendingConfirm::DropQueue { subject: queued });
                    }
                }
            }
            UiAction::AskRemoveCompleted => {
                let subject = self
                    .state
                    .downloads
                    .iter()
                    .filter(|r| r.state == DownloadState::Completed)
                    .count();
                if subject == 0 {
                    self.state
                        .push_log("info", BulkAction::RemoveCompleted.nothing_to_do());
                } else if self.confirm_destructive() {
                    self.state.pending_confirm =
                        Some(PendingConfirm::RemoveCompleted { subject });
                } else {
                    self.remove_completed_now(self.settings.settings().purge_on_remove);
                }
            }
            UiAction::Remove { id, purge } => {
                self.state.pending_confirm = None;
                self.report(self.backend.remove(id, purge));
                self.refresh(true);
            }
            UiAction::RemoveCompletedConfirmed { purge } => {
                self.state.pending_confirm = None;
                self.remove_completed_now(purge);
            }
            UiAction::PauseAll => match self.backend.pause_all() {
                Ok(0) => self
                    .state
                    .push_log("info", BulkAction::PauseAll.nothing_to_do()),
                Ok(n) => self
                    .state
                    .push_log("info", BulkAction::PauseAll.done(n)),
                Err(err) => self.state.push_log("error", format!("{err:#}")),
            },
            UiAction::ResumeAll => {
                let defaults = self.settings.settings().to_request();
                let limit = self.max_concurrent();
                match self.backend.resume_all(&defaults, limit) {
                    Ok(n) => {
                        // Failed downloads cannot continue (the engine rejects
                        // them), so name them instead of skipping them silently.
                        let needs_restart = self
                            .state
                            .downloads
                            .iter()
                            .filter(|r| r.state == DownloadState::Failed)
                            .count();
                        self.state.push_log("info", ux::resume_all_outcome(n, needs_restart));
                    }
                    Err(err) => self.state.push_log("error", format!("{err:#}")),
                }
            }
            UiAction::Select(id) => {
                self.state.selected = Some(id);
                self.refresh(true);
            }
            UiAction::OpenDetails(id) => {
                self.state.selected = Some(id);
                self.state.detail_id = Some(id);
                self.refresh(true);
            }
            UiAction::Refresh => self.refresh(true),
            UiAction::CopyToClipboard(text) => {
                ctx.output_mut(|o| o.copied_text = text);
                self.state.push_log("info", "copied to clipboard");
            }
            UiAction::OpenOutputFolder(id) => {
                let target = self
                    .state
                    .downloads
                    .iter()
                    .find(|r| r.id == id)
                    .map(|r| PathBuf::from(&r.output_path));
                match target {
                    Some(path) => {
                        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
                        match util::open_in_file_manager(&dir) {
                            Ok(()) => self
                                .state
                                .push_log("info", format!("opened {}", dir.display())),
                            Err(err) => self
                                .state
                                .push_log("error", format!("cannot open folder: {err}")),
                        }
                    }
                    None => self.state.push_log("error", "no such download"),
                }
            }
            UiAction::SaveSettings => match self.settings.save() {
                Ok(()) => {
                    self.state.settings_dirty = false;
                    let path = self.settings.path().display().to_string();
                    self.state.push_log("info", format!("settings saved to {path}"));
                }
                Err(err) => self
                    .state
                    .push_log("error", format!("cannot save settings: {err}")),
            },
            UiAction::ReloadSettings => {
                self.settings.load();
                self.state.settings_dirty = false;
                self.state.push_log("info", "settings reloaded from disk");
            }
            UiAction::ApplyDataDir => {
                let dir = PathBuf::from(self.state.data_dir_input.trim());
                match self.backend.switch_data_dir(&dir) {
                    Ok(()) => {
                        self.settings.relocate(&dir);
                        self.settings.settings_mut().data_dir = dir.display().to_string();
                        self.state
                            .push_log("info", format!("using metadata in {}", dir.display()));
                        self.state.selected = None;
                        self.state.detail_id = None;
                        self.refresh(true);
                    }
                    Err(err) => self.state.push_log("error", format!("{err:#}")),
                }
            }
            UiAction::DropQueued(seq) => match self.backend.cancel_pending(seq) {
                Some(job) => self
                    .state
                    .push_log("info", format!("Dropped {} from the queue.", job.url)),
                None => self.state.push_log(
                    "info",
                    "That download already started — it is no longer in the queue.",
                ),
            },
            UiAction::DropQueue => {
                self.state.pending_confirm = None;
                self.drop_queue_now();
            }
            UiAction::ToggleHelp => {
                self.state.show_help = !self.state.show_help;
            }
            UiAction::ClearLog => self.state.log.clear(),
        }
    }

    /// React to the tray menu / icon.
    fn poll_tray(&mut self, _ctx: &Context) {
        // Commands are copied out first: handling them mutates the app, which
        // would otherwise conflict with the borrow of `self.tray`.
        let commands = match &self.tray {
            Some(tray) => tray.poll(),
            None => return,
        };
        for command in commands {
            match command {
                TrayCommand::Show => self.reveal_requested = true,
                TrayCommand::Quit => {
                    self.quit_requested = true;
                    _ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                TrayCommand::PauseAll => match self.backend.pause_all() {
                    Ok(0) => self
                        .state
                        .push_log("info", BulkAction::PauseAll.nothing_to_do()),
                    Ok(n) => self.state.push_log("info", BulkAction::PauseAll.done(n)),
                    Err(err) => self.state.push_log("error", format!("{err:#}")),
                },
                TrayCommand::ResumeAll => {
                    let defaults = self.settings.settings().to_request();
                    let limit = self.max_concurrent();
                    match self.backend.resume_all(&defaults, limit) {
                        Ok(n) => {
                            let needs_restart = self
                                .state
                                .downloads
                                .iter()
                                .filter(|r| r.state == DownloadState::Failed)
                                .count();
                            self.state
                                .push_log("info", ux::resume_all_outcome(n, needs_restart));
                        }
                        Err(err) => self.state.push_log("error", format!("{err:#}")),
                    }
                }
                TrayCommand::ToggleDropTarget => {
                    self.drop_target_shown = !self.drop_target_shown;
                    self.settings.settings_mut().drop_target_enabled = self.drop_target_shown;
                    if let Err(err) = self.settings.save() {
                        self.state
                            .push_log("error", format!("cannot save the setting: {err}"));
                    }
                    self.state.push_log(
                        "info",
                        if self.drop_target_shown {
                            "floating drop target shown above the taskbar clock"
                        } else {
                            "floating drop target hidden"
                        },
                    );
                }
                TrayCommand::NewDownload => {
                    // The clipboard is the whole point of this entry: copy a
                    // link, pick it from the tray, fill the form.
                    let prefilled = if self.settings.settings().clipboard_prefill {
                        clipboard::url()
                    } else {
                        None
                    };
                    if prefilled.is_none() {
                        self.state.push_log(
                            "info",
                            "no link on the clipboard — opening the form empty",
                        );
                    }
                    self.open_new_download(prefilled);
                }
            }
        }
        // The menu check mark follows the setting, whatever changed it.
        if let Some(tray) = &self.tray {
            tray.set_drop_target_shown(self.drop_target_shown);
        }
    }

    /// Links dropped on the floating target or on the window (queued by the
    /// drop target, or pushed by the tray). Opening the form also brings the
    /// window back from the tray.
    fn drain_imports(&mut self, _ctx: &Context) {
        // Drops on the floating target arrive with their own outcome sentence:
        // a drop that carried no readable link says so, and it tells the user
        // when the clipboard was used instead.
        let mut urls: Vec<String> = Vec::new();
        for drop in self.drop_zone.drain() {
            self.state.push_log("info", drop.note);
            if let Some(url) = drop.url {
                urls.push(url);
            }
        }
        urls.extend(self.imports.drain());
        if urls.is_empty() {
            return;
        }
        let last = urls.len();
        for url in urls {
            if last == 1 {
                self.state.push_log(
                    "info",
                    format!("Dropped link — starting the form for {url}"),
                );
            }
            self.open_new_download(Some(url));
        }
    }

    /// Show the form, optionally pre-filled, and bring the window forward.
    fn open_new_download(&mut self, url: Option<String>) {
        let mut form = self.settings.settings().to_request();
        form.url = url.unwrap_or_default();
        self.state.form = form;
        self.state.form_error = None;
        self.state.show_add = true;
        self.state.focus_url = true;
        self.reveal_requested = true;
    }

    /// Keep the floating drop target on screen while the setting is on.
    fn sync_drop_target(&mut self, ctx: &Context) {
        if !self.drop_target_shown {
            return;
        }
        if self.drop_zone.take_activation() {
            self.reveal_requested = true;
        }
        self.drop_zone.set_dark(self.settings.settings().dark_mode);
        self.drop_zone.show(ctx);
    }

    /// Does a destructive action ask before acting? Default: yes.
    fn confirm_destructive(&self) -> bool {
        self.settings.settings().confirm_remove
    }

    /// Human label for one download, e.g. `dl-8f3c2a1b (ubuntu.iso)`.
    fn describe(&self, id: i64) -> String {
        self.state
            .downloads
            .iter()
            .find(|r| r.id == id)
            .map(|r| format!("{} ({})", r.public_id, r.filename))
            .unwrap_or_else(|| format!("#{id}"))
    }

    /// Start a download again from the beginning (confirmed or not).
    fn restart_now(&mut self, id: i64) {
        let defaults = self.settings.settings().to_request();
        let limit = self.max_concurrent();
        match self.backend.restart(id, &defaults, limit) {
            Ok(_) => self.state.push_log(
                "info",
                format!("Restarting {} from the beginning…", self.describe(id)),
            ),
            Err(err) => self.state.push_log("error", format!("{err:#}")),
        }
    }

    /// Drop every queued download; nothing has been fetched yet, but the count
    /// is still reported so the action is never silent.
    fn drop_queue_now(&mut self) {
        let n = self.backend.clear_queue();
        if n == 0 {
            self.state
                .push_log("info", BulkAction::DropQueue.nothing_to_do());
        } else {
            self.state
                .push_log("info", BulkAction::DropQueue.done(n));
        }
        self.state.queue = self.backend.pending();
    }

    /// Remove every completed download; the count and the file decision are
    /// always reported back.
    fn remove_completed_now(&mut self, purge: bool) {
        match self.backend.remove_completed(purge) {
            Ok(0) => self
                .state
                .push_log("info", BulkAction::RemoveCompleted.nothing_to_do()),
            Ok(n) => self
                .state
                .push_log("info", BulkAction::RemoveCompleted.done(n)),
            Err(err) => self.state.push_log("error", format!("{err:#}")),
        }
        self.refresh(true);
    }

    /// 0 means "no limit".
    fn max_concurrent(&self) -> usize {
        self.settings.settings().max_concurrent as usize
    }

    fn report(&mut self, result: anyhow::Result<String>) {
        match result {
            Ok(message) => self.state.push_log("info", message),
            Err(err) => self.state.push_log("error", format!("{err:#}")),
        }
    }

    // --------------------------------------------------------------- keyboard

    /// Move the table selection by `delta` rows inside the visible set.
    fn move_selection(&mut self, delta: isize) {
        let rows: Vec<i64> = self.state.visible_rows().iter().map(|r| r.id).collect();
        if rows.is_empty() {
            return;
        }
        let current = self
            .state
            .selected
            .and_then(|id| rows.iter().position(|r| *r == id));
        let next = match current {
            Some(pos) => (pos as isize + delta).clamp(0, rows.len() as isize - 1) as usize,
            None if delta >= 0 => 0,
            None => rows.len() - 1,
        };
        self.state.selected = Some(rows[next]);
    }

    /// Escape closes the top-most overlay, then the side panels.
    fn close_overlay(&mut self) {
        if self.state.show_help {
            self.state.show_help = false;
        } else if self.state.show_add {
            self.state.show_add = false;
            self.state.form_error = None;
        } else if self.state.pending_confirm.is_some() {
            self.state.pending_confirm = None;
        } else if self.state.detail_id.is_some() {
            self.state.detail_id = None;
        } else if self.state.footer_panel.is_some() {
            self.state.footer_panel = None;
        } else {
            self.state.show_queue = false;
            self.state.show_settings = false;
        }
    }

    /// Keyboard design: the table is fully reachable without a mouse.
    ///
    /// The shortcuts only fire while no text field has focus, so typing in the
    /// search box or in the "New download" form is never hijacked.
    fn handle_shortcuts(&mut self, ctx: &Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let (up, down, enter, escape, find, refresh, help) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
                i.modifiers.command && i.key_pressed(egui::Key::F),
                i.key_pressed(egui::Key::F5),
                i.key_pressed(egui::Key::F1),
            )
        });
        if down {
            self.move_selection(1);
        }
        if up {
            self.move_selection(-1);
        }
        if enter {
            if let Some(id) = self.state.selected {
                self.state.detail_id = Some(id);
            }
        }
        if find {
            self.state.focus_search = true;
        }
        if refresh {
            self.refresh(true);
        }
        if help {
            self.state.show_help = !self.state.show_help;
        }
        if escape {
            self.close_overlay();
        }
    }

    // ------------------------------------------------------------- rendering

    /// The single confirmation dialog, driven entirely by [`Confirm`] policy:
    /// title, consequence text, the destructive and the safe action.
    fn confirm_dialog(&mut self, ctx: &Context) -> Vec<UiAction> {
        let mut actions = Vec::new();
        let Some(pending) = self.state.pending_confirm.clone() else {
            return actions;
        };
        let confirm = pending.policy();
        let checkbox = confirm.has_file_checkbox();
        let palette = theme::palette_ctx(ctx);
        let spacing = Spacing::default();
        let mut purge = self.settings.settings().purge_on_remove;
        let mut close = false;
        egui::Window::new(confirm.title())
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                if let Some(label) = pending.label() {
                    components::section_title(ui, label);
                    ui.add_space(spacing.xs);
                }
                components::banner(ui, &palette, components::Level::Warning, &confirm.body(pending.subject()));
                if checkbox {
                    ui.add_space(spacing.sm);
                    ui.checkbox(&mut purge, "also delete the finished file(s)");
                }
                ui.add_space(spacing.lg);
                ui.horizontal(|ui| {
                    if components::danger_button(ui, &palette, confirm.confirm_label()).clicked() {
                        actions.push(confirmed_action(&pending, purge));
                    }
                    if ui.button(confirm.keep_label()).clicked() {
                        close = true;
                    }
                });
            });
        if close {
            self.state.pending_confirm = None;
        }
        if checkbox && purge != self.settings.settings().purge_on_remove {
            self.settings.settings_mut().purge_on_remove = purge;
        }
        actions
    }
}

/// Turn the user's answer into the action that does the work.
fn confirmed_action(pending: &PendingConfirm, purge: bool) -> UiAction {
    match pending {
        PendingConfirm::Remove { id, .. } => UiAction::Remove { id: *id, purge },
        PendingConfirm::RemoveCompleted { .. } => UiAction::RemoveCompletedConfirmed { purge },
        PendingConfirm::Restart { id, .. } => UiAction::Restart(*id),
        PendingConfirm::DropQueue { .. } => UiAction::DropQueue,
    }
}

impl eframe::App for RdmGuiApp {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        // The tray's relay threads read this clock: if the UI ever stops
        // delivering frames, they bring the window back with the OS instead of
        // letting a click wait forever (see `crate::frames`).
        crate::frames::note_frame();

        // Second chance for the window handle. `main.rs` asks the start-up
        // context for it; this asks a live window the same question, so a
        // session where the first answer was empty still arms the tray's rescue
        // path. Asked once: the answer cannot change for a window.
        if !self.window_handle_captured {
            crate::windows::remember_main_window_from(&*frame);
            self.window_handle_captured = true;
            tracing::info!(
                "tray: main-window handle {}",
                if crate::windows::main_window_is_known() {
                    "captured"
                } else {
                    "NOT captured — the tray will search for the window instead"
                }
            );
        }

        if self.settings.poll_external_change() {
            self.state.settings_dirty = false;
            self.state
                .push_log("info", "settings.toml changed on disk — reloaded");
        }
        // Theme + global paddings, applied live whenever the toggle changes.
        let dark = self.settings.settings().dark_mode;
        if self.applied_dark != Some(dark) {
            theme::install(ctx, dark);
            self.applied_dark = Some(dark);
        }

        let limit = self.max_concurrent();
        let started = self.backend.pump(limit);
        if started > 0 {
            self.state
                .push_log("info", format!("started {started} queued download(s)"));
        }
        self.state.queue = self.backend.pending();

        self.drain_backend_events();
        self.drain_engine_logs();
        self.poll_tray(ctx);
        self.drain_imports(ctx);
        self.sync_drop_target(ctx);
        let wanted_level = self.settings.settings().log_level.clone();
        if wanted_level != self.log_level {
            self.set_log_level(&wanted_level);
        }
        self.refresh(false);

        // Opening the Events box should fill immediately, not on the next tick.
        if self.last_footer != self.state.footer_panel {
            self.last_footer = self.state.footer_panel;
            self.refresh(true);
        }

        self.handle_shortcuts(ctx);

        // The setting is the source of truth; the mirror is what the viewport
        // code reads, and the tray menu shows the same state.
        self.state.prefill_from_clipboard = self.settings.settings().clipboard_prefill;
        let wanted_target = self.settings.settings().drop_target_enabled;
        if wanted_target != self.drop_target_shown {
            self.drop_target_shown = wanted_target;
            if let Some(tray) = &self.tray {
                tray.set_drop_target_shown(wanted_target);
            }
        }
        let mut actions: Vec<UiAction> = Vec::new();
        let active_jobs = self.backend.active_jobs();
        let queued = self.state.queue.len();
        let sizes = Sizes::default();
        let spacing = Spacing::default();

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(spacing.sm);
            actions.extend(crate::views::toolbar::show(
                ui,
                &mut self.state,
                active_jobs,
                queued,
            ));
            ui.add_space(spacing.xs);
        });

        egui::TopBottomPanel::bottom("status-bar")
            .resizable(false)
            .show(ctx, |ui| {
                crate::views::footer::status_bar(ui, &mut self.state);
            });

        if self.state.footer_panel.is_some() {
            egui::TopBottomPanel::bottom("footer-panel")
                .resizable(true)
                .default_height(sizes.footer_default)
                .min_height(sizes.footer_min)
                .show(ctx, |ui| {
                    actions.extend(crate::views::footer::panel(ui, &mut self.state));
                });
        }

        let sidebar_max = theme::sidebar_max_width(ctx.available_rect().width());

        if self.state.show_queue {
            egui::SidePanel::left("queue-sidebar")
                .resizable(true)
                .default_width(sizes.sidebar_default)
                .min_width(sizes.sidebar_queue_min)
                .max_width(sidebar_max)
                .show(ctx, |ui| {
                    actions.extend(crate::views::queue_sidebar::show(ui, &mut self.state));
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            actions.extend(crate::views::download_list::show(ui, &mut self.state));
        });

        let db_path = self.backend.db_path().display().to_string();
        actions.extend(crate::views::settings_view::SettingsWindow::show(
            &mut self.settings_window,
            ctx,
            &mut self.settings,
            &mut self.state,
            &db_path,
        ));
        actions.extend(crate::views::add_download::show(ctx, &mut self.state));
        actions.extend(crate::views::details_modal::show(ctx, &mut self.state));
        actions.extend(crate::views::help_overlay::show(ctx, &mut self.state));
        actions.extend(self.confirm_dialog(ctx));

        // Files or links dropped on the main window take the same path as the
        // floating target: they open the form, pre-filled.
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty() {
            for report in util::dropped_links(&dropped) {
                self.state.push_log("info", report.note);
                if let Some(url) = report.url {
                    self.imports.push(url);
                }
            }
            self.reveal_requested = true;
            self.drain_imports(ctx);
        }

        for action in actions {
            self.apply(action, ctx);
        }

        // Closing the window: with a tray available and the setting on, the
        // app keeps running there (transfers included) instead of quitting.
        if ctx.input(|i| i.viewport().close_requested()) && !self.shutting_down {
            let hide = self.settings.settings().close_to_tray
                && self.tray.is_some()
                && !self.quit_requested;
            if hide {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                // Put the window in the tray *without* blanking it — see
                // `windows::hide_main_window` for the whole reason. In short: a
                // window that is not visible to Windows receives no `WM_PAINT`,
                // the frame loop stops for good, and every tray command
                // (including the one that would show the window again) waits for
                // a frame that can never come. That was the round-3 freeze, and
                // it is also why the window is not simply minimized: nothing
                // visible means nothing painted.
                //
                // The OS-level hide is undone by `windows::reveal_main_window`,
                // which the tray's relay thread can call with no frame at all.
                let in_tray = crate::windows::hide_main_window();
                // Also straight to the log file: this decision is the first
                // question any “it froze in the tray” report has to answer.
                tracing::info!("tray: hide_main_window -> {in_tray}");
                self.state.push_log(
                    "info",
                    if in_tray {
                        "window hidden — rdm keeps running in the tray (Quit there to exit)"
                    } else {
                        // The tray is there but the window could not be taken
                        // off the screen, so it stays open and visible: saying
                        // “minimized” here would describe something that never
                        // happened.
                        "could not hide the window (no window handle known) — rdm stays \
                         on screen; use Quit in the tray to exit"
                    },
                );
            } else {
                self.shutting_down = true;
                let asked = self.backend.shutdown(Duration::from_secs(5));
                if asked > 0 {
                    self.state
                        .push_log("warn", format!("paused {asked} running download(s) on exit"));
                }
            }
        }

        if self.reveal_requested {
            self.reveal_requested = false;
            tracing::info!("tray: reveal requested — restoring the window");
            // Undo the tray hide. The relay thread may already have done it
            // through the OS (that is how “Show rdm” works with no frame
            // running), and both calls are idempotent; `Focus` also covers the
            // case where only the taskbar style was missing.
            crate::windows::reveal_main_window();
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Informational,
            ));
        }

        // Keep the window live while transfers are in flight.
        let busy = self.backend.active_jobs() > 0
            || !self.state.queue.is_empty()
            || self
                .state
                .downloads
                .iter()
                .any(|r| r.state.active() || r.state == DownloadState::Merging);
        let refresh = self.settings.settings().refresh_ms.max(100);
        let mut interval = if busy { refresh.min(500) } else { refresh };
        // A window hidden in the tray only runs a frame when something asks it
        // to, and the state-dependent tray commands (*Pause all*, *Resume all*,
        // the drop-target toggle) are read inside a frame. A live tray therefore
        // keeps a heartbeat: short while the window is on screen, slow while it
        // is in the tray, where a click also wakes the app and the two commands
        // that must never wait are carried out by the relay thread itself.
        if self.tray.is_some() {
            let heartbeat = if crate::windows::main_window_is_hidden() {
                TRAY_HIDDEN_HEARTBEAT_MS
            } else {
                TRAY_HEARTBEAT_MS
            };
            interval = interval.min(heartbeat);
        }
        ctx.request_repaint_after(Duration::from_millis(interval));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if !self.shutting_down {
            self.shutting_down = true;
            self.backend.shutdown(Duration::from_secs(5));
        }
    }

    /// See [`CLEAR_COLOR`]: transparent, so the floating drop target is a mark
    /// floating on the desktop rather than a dark square.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        CLEAR_COLOR
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_background_is_transparent_so_transparent_windows_are_clean() {
        assert_eq!(
            CLEAR_COLOR[3], 0.0,
            "the clear colour must contribute no colour: eframe clears every viewport \
             with it, including the transparent floating drop target"
        );
        assert_eq!(CLEAR_COLOR, [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn the_tray_hide_path_never_blanks_the_window() {
        // Regression guard for the round-3 report — twice, because the first
        // fix passed the first half of this test and the freeze came back.
        //
        // `Visible(false)` clears `WS_VISIBLE`, and minimizing leaves nothing on
        // screen: either way Windows sends no `WM_PAINT`, the frame loop stops
        // for good, and *every* tray command — including “Show rdm” — waits for
        // a frame that can never come. The window must be taken off the screen
        // in a way that keeps it painting.
        let source = include_str!("app.rs");
        // Built from two halves so this test does not trip over its own text.
        for (forbidden, why) in [
            (
                format!("{}{}", "ViewportCommand::Visible(", "false)"),
                "the window is blanked again — that stops the frame loop and kills the tray",
            ),
            (
                format!("{}{}", "ViewportCommand::Minimized(", "true)"),
                "minimizing stops the paint stream just as reliably as blanking the window",
            ),
        ] {
            assert!(!source.contains(&forbidden), "{why}");
        }
        for needed in [
            "crate::windows::hide_main_window()",
            "crate::windows::reveal_main_window()",
            "crate::frames::note_frame()",
        ] {
            assert!(source.contains(needed), "the tray path lost `{needed}`");
        }
    }

    #[test]
    fn the_heartbeat_is_short_enough_to_feel_instant() {
        assert!(
            TRAY_HEARTBEAT_MS <= 250,
            "a click must not wait longer than a quarter second for a frame"
        );
        // In the tray the window is invisible and its commands are carried out
        // by the relay threads, so the tick may be slower — but not so slow that
        // `frames::is_stale` (used to decide whether a click needs rescuing)
        // would report a healthy app as dead.
        assert!(
            TRAY_HIDDEN_HEARTBEAT_MS >= 500,
            "an invisible window has no reason to repaint every {TRAY_HIDDEN_HEARTBEAT_MS} ms"
        );
        assert!(
            TRAY_HIDDEN_HEARTBEAT_MS + 250 < crate::frames::STALE_MS,
            "the hidden heartbeat must stay well inside the liveness window \
             ({TRAY_HIDDEN_HEARTBEAT_MS} ms vs {})",
            crate::frames::STALE_MS
        );
    }
}

// The palette installation, the sidebar cap and their unit tests now live in
// `crate::theme` — see `theme::install`, `theme::sidebar_max_width` and
// `theme::tests::sidebar_never_eats_the_window`.
