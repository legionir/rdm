//! Application root: polls the metadata database, renders the panels and
//! turns [`UiAction`]s into `rdm` library calls.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Color32, Context, RichText};

use rdm::models::DownloadState;

use crate::backend::{Backend, BackendEvent};
use crate::logging::LogControl;
use crate::settings::SettingsStore;
use crate::state::{DetailTab, GuiState, UiAction};
use crate::util;

pub struct RdmGuiApp {
    backend: Backend,
    settings: SettingsStore,
    state: GuiState,
    last_poll: Instant,
    last_selected: Option<i64>,
    theme_applied: bool,
    logging: Option<LogControl>,
    log_level: String,
    shutting_down: bool,
}

impl RdmGuiApp {
    pub fn new(
        data_dir: PathBuf,
        logging: Option<LogControl>,
        forced_level: Option<&'static str>,
    ) -> anyhow::Result<Self> {
        let backend = Backend::new(&data_dir)?;
        let mut settings = SettingsStore::new(&data_dir);
        // The command line wins over whatever the file says.
        settings.settings_mut().data_dir = data_dir.display().to_string();
        let state = GuiState::new(
            settings.settings().to_request(),
            data_dir.display().to_string(),
        );
        let mut app = RdmGuiApp {
            backend,
            settings,
            state,
            last_poll: Instant::now(),
            last_selected: None,
            theme_applied: false,
            logging,
            log_level: String::new(),
            shutting_down: false,
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
        let selection_changed = self.last_selected != self.state.selected;
        self.last_selected = self.state.selected;
        if let Some(id) = self.state.selected {
            let wants_chunks = matches!(self.state.detail_tab, DetailTab::Chunks);
            let wants_events = matches!(self.state.detail_tab, DetailTab::Events);
            let wants_json = matches!(self.state.detail_tab, DetailTab::Json);
            if wants_chunks || selection_changed {
                self.state.chunks = self.backend.chunks(id).unwrap_or_default();
            }
            if wants_events || selection_changed {
                self.state.events = self.backend.events(id, 50).unwrap_or_default();
            }
            if wants_json || selection_changed {
                self.state.json = match self.backend.snapshot_json(id) {
                    Ok(json) => json,
                    Err(err) => format!("// {err:#}"),
                };
            }
        } else {
            self.state.chunks.clear();
            self.state.events.clear();
            self.state.json.clear();
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

    /// Move captured `tracing` output into the App log tab.
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
                self.state.form = form;
                self.state.form_error = None;
                self.state.show_add = true;
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
                let defaults = self.settings.settings().to_request();
                let limit = self.max_concurrent();
                match self.backend.restart(id, &defaults, limit) {
                    Ok(_) => self
                        .state
                        .push_log("warn", format!("restarting #{id} from scratch")),
                    Err(err) => self.state.push_log("error", format!("{err:#}")),
                }
            }
            UiAction::AskRemove(id) => {
                let label = self
                    .state
                    .downloads
                    .iter()
                    .find(|r| r.id == id)
                    .map(|r| format!("{} ({})", r.public_id, r.filename))
                    .unwrap_or_else(|| format!("#{id}"));
                if self.settings.settings().confirm_remove {
                    self.state.pending_remove = Some((id, label));
                } else {
                    let purge = self.settings.settings().purge_on_remove;
                    self.report(self.backend.remove(id, purge));
                    self.refresh(true);
                }
            }
            UiAction::Remove { id, purge } => {
                self.state.pending_remove = None;
                self.report(self.backend.remove(id, purge));
                self.refresh(true);
            }
            UiAction::RemoveCompleted => {
                let purge = self.settings.settings().purge_on_remove;
                match self.backend.remove_completed(purge) {
                    Ok(n) => self.state.push_log("info", format!("removed {n} record(s)")),
                    Err(err) => self.state.push_log("error", format!("{err:#}")),
                }
                self.refresh(true);
            }
            UiAction::PauseAll => match self.backend.pause_all() {
                Ok(n) => self
                    .state
                    .push_log("info", format!("pause requested for {n} download(s)")),
                Err(err) => self.state.push_log("error", format!("{err:#}")),
            },
            UiAction::ResumeAll => {
                let defaults = self.settings.settings().to_request();
                let limit = self.max_concurrent();
                match self.backend.resume_all(&defaults, limit) {
                    Ok(n) => self.state.push_log("info", format!("resuming {n} download(s)")),
                    Err(err) => self.state.push_log("error", format!("{err:#}")),
                }
            }
            UiAction::Select(id) => {
                self.state.selected = Some(id);
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
                        self.refresh(true);
                    }
                    Err(err) => self.state.push_log("error", format!("{err:#}")),
                }
            }
            UiAction::CancelPending(seq) => match self.backend.cancel_pending(seq) {
                Some(job) => self
                    .state
                    .push_log("warn", format!("removed {} from the queue", job.url)),
                None => self.state.push_log("warn", "that job already started"),
            },
            UiAction::ClearQueue => {
                let n = self.backend.clear_queue();
                self.state
                    .push_log("warn", format!("dropped {n} queued job(s)"));
            }
            UiAction::ClearLog => self.state.log.clear(),
        }
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

    // ------------------------------------------------------------- rendering

    fn confirm_dialog(&mut self, ctx: &Context) -> Vec<UiAction> {
        let mut actions = Vec::new();
        let Some((id, label)) = self.state.pending_remove.clone() else {
            return actions;
        };
        let mut purge = self.settings.settings().purge_on_remove;
        let mut close = false;
        egui::Window::new("Remove download")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Remove {label} from the database?"));
                ui.checkbox(&mut purge, "also delete the downloaded file");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new("Remove").color(Color32::from_rgb(239, 68, 68)))
                        .clicked()
                    {
                        actions.push(UiAction::Remove { id, purge });
                    }
                    if ui.button("Keep").clicked() {
                        close = true;
                    }
                });
            });
        if close {
            self.state.pending_remove = None;
        }
        if purge != self.settings.settings().purge_on_remove {
            self.settings.settings_mut().purge_on_remove = purge;
        }
        actions
    }
}

impl eframe::App for RdmGuiApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if !self.theme_applied {
            self.theme_applied = true;
            apply_theme(ctx, self.settings.settings().dark_mode);
        }

        if self.settings.poll_external_change() {
            self.state.settings_dirty = false;
            apply_theme(ctx, self.settings.settings().dark_mode);
            self.state
                .push_log("info", "settings.toml changed on disk — reloaded");
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
        let wanted_level = self.settings.settings().log_level.clone();
        if wanted_level != self.log_level {
            self.set_log_level(&wanted_level);
        }
        self.refresh(false);

        let mut actions: Vec<UiAction> = Vec::new();
        let active_jobs = self.backend.active_jobs();
        let queued = self.state.queue.len();

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("rdm");
                ui.label(
                    RichText::new("Rust Download Manager")
                        .small()
                        .color(Color32::from_gray(140)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(self.backend.db_path().display().to_string())
                            .small()
                            .color(Color32::from_gray(120)),
                    );
                });
            });
            ui.add_space(2.0);
            actions.extend(crate::views::toolbar::show(
                ui,
                &mut self.state,
                active_jobs,
                queued,
            ));
            ui.add_space(4.0);
        });

        egui::SidePanel::right("settings-panel")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading("⚙ Settings");
                ui.separator();
                let path = self.settings.path().display().to_string();
                actions.extend(crate::views::settings_view::show(
                    ui,
                    self.settings.settings_mut(),
                    &path,
                    &mut self.state,
                ));
            });

        egui::TopBottomPanel::bottom("status-bar").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let color = if self.state.status_is_error {
                    Color32::from_rgb(239, 68, 68)
                } else {
                    Color32::from_gray(170)
                };
                ui.label(RichText::new(&self.state.status).small().color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (done, waiting, running, failed, cancelled) = self.state.counts();
                    ui.label(
                        RichText::new(format!(
                            "{} record(s) · {done} done · {running} running · {waiting} waiting · {failed} failed · {cancelled} cancelled",
                            self.state.downloads.len()
                        ))
                        .small()
                        .color(Color32::from_gray(140)),
                    );
                });
            });
            ui.add_space(2.0);
        });

        egui::TopBottomPanel::bottom("details-panel")
            .resizable(true)
            .default_height(260.0)
            .min_height(120.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                actions.extend(crate::views::details::show(ui, &mut self.state));
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            actions.extend(crate::views::download_list::show(ui, &mut self.state));
        });

        actions.extend(crate::views::add_download::show(ctx, &mut self.state));
        actions.extend(self.confirm_dialog(ctx));

        for action in actions {
            self.apply(action, ctx);
        }

        if ctx.input(|i| i.viewport().close_requested()) && !self.shutting_down {
            self.shutting_down = true;
            let asked = self.backend.shutdown(Duration::from_secs(5));
            if asked > 0 {
                self.state
                    .push_log("warn", format!("paused {asked} running download(s) on exit"));
            }
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
        ctx.request_repaint_after(Duration::from_millis(if busy { refresh.min(500) } else { refresh }));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if !self.shutting_down {
            self.shutting_down = true;
            self.backend.shutdown(Duration::from_secs(5));
        }
    }
}

fn apply_theme(ctx: &Context, dark: bool) {
    ctx.set_visuals(if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    });
}
