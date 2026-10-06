//! Settings — a real OS window with one tab per section.
//!
//! ## Why it stopped being a sidebar
//!
//! Settings used to open as a right-hand sidebar, which made it a *panel of the
//! main window*: it could not be moved or kept next to something else, it
//! competed with the download table for width, and the ✕ drawn inside it was
//! one of the glyphs that rendered as an empty box in the running app. It is
//! now an egui *deferred viewport*: an ordinary decorated window with the
//! system's own close and minimise buttons in the top-right corner, freely
//! moveable and resizeable, with the settings grouped into three tabs —
//! *Downloads*, *Application*, *Desktop integration*.
//!
//! ## How the edits cross the window boundary
//!
//! A deferred viewport renders in a closure that must be `Send + Sync +
//! 'static`, so it cannot borrow the app. The window therefore carries a small
//! [`Shared`] block behind a mutex:
//!
//! ```text
//! app (main window)                  settings window (own viewport)
//! ─────────────────────              ───────────────────────────────
//! SettingsStore  ──seed──►  Shared.draft  ──read/written by──►  user
//!       ▲                         │
//!       └─── synchronised ────────┘
//!             every frame, plus the queued actions
//! ```
//!
//! The draft is synchronised into the store on every frame where it differs —
//! exactly the old behaviour, where the sidebar edited the store in place, so
//! the rest of the app (for instance the tooltip that follows the clipboard
//! switch) sees an edit immediately, and Save/Reload/Apply keep their meaning.
//! Reload re-seeds the draft from the store, so it really discards edits.
//! Closing the window keeps the edits, like before.
//!
//! Tabs exist because the window has room for all three sections at once: the
//! two directory pickers and nine numeric fields used to be one long scroll
//! competing with the desktop switches. Warning signs stay visible in every
//! tab: the unsaved-changes banner and the two path labels are in the window's
//! footer, not inside a tab.

use std::sync::{Arc, Mutex};

use egui::{Align, Context, Layout, Ui, ViewportBuilder, ViewportId};

use crate::settings::{AppSettings, SettingsStore};
use crate::state::{GuiState, UiAction};
use crate::theme::{self, components, Icon, Sizes, Spacing};
use crate::util;
use crate::ux;

/// One settings section: a tab in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// Where files land and the defaults every new download starts from.
    Downloads,
    /// Metadata directory, concurrency, refresh, confirmations, theme, log.
    Application,
    /// Tray, clipboard pre-fill and the floating drop target.
    Desktop,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Downloads, Tab::Application, Tab::Desktop];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Downloads => "Downloads",
            Tab::Application => "Application",
            Tab::Desktop => "Desktop integration",
        }
    }

    fn tooltip(self) -> &'static str {
        match self {
            Tab::Downloads => "Where files land and the defaults every new download starts from",
            Tab::Application => "Metadata directory, concurrency, theme and the engine log",
            Tab::Desktop => "Tray, clipboard pre-fill and the floating drop target",
        }
    }
}

/// The state the settings window owns while it is on screen.
struct Shared {
    /// The edited copy of the settings.
    draft: AppSettings,
    data_dir_input: String,
    settings_path: String,
    db_path: String,
    /// The window holds edits that are not written to `settings.toml` yet.
    dirty: bool,
    /// Actions the window wants the app to run (Save, Reload, Apply…).
    actions: Vec<UiAction>,
    /// The window's own ✕ (or Alt-F4) was used.
    close_requested: bool,
    /// Copy the store into the draft on the next frame (on open, and on
    /// Reload).
    seed: bool,
    /// The active tab. It lives here, not in the struct, because the viewport
    /// closure is an `Fn`: it may read captured values, not mutate them.
    tab: Tab,
    /// The store has been copied into the draft at least once. Until then
    /// nothing here may be written back into the app's view-model: the block is
    /// still empty, and an empty metadata field would wipe the app's.
    seeded: bool,
}

impl Shared {
    fn new() -> Self {
        Shared {
            draft: AppSettings::default(),
            data_dir_input: String::new(),
            settings_path: String::new(),
            db_path: String::new(),
            dirty: false,
            actions: Vec::new(),
            close_requested: false,
            seed: true,
            tab: Tab::Downloads,
            seeded: false,
        }
    }
}

/// The Settings window. Owned by the app; [`SettingsWindow::show`] is called
/// once per frame whatever the window's state.
pub struct SettingsWindow {
    shared: Arc<Mutex<Shared>>,
    tab: Tab,
    /// The viewport is on screen, so a close has to be sent explicitly when
    /// the app hides it (Esc, the toolbar toggle, the tray menu).
    shown: bool,
}

impl Default for SettingsWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsWindow {
    pub fn new() -> Self {
        SettingsWindow {
            shared: Arc::new(Mutex::new(Shared::new())),
            tab: Tab::Downloads,
            shown: false,
        }
    }

    /// The viewport id of the settings window.
    pub fn id() -> ViewportId {
        ViewportId::from_hash_of("rdm-settings-window")
    }

    /// Show (or close) the window and carry edits and actions across.
    pub fn show(
        &mut self,
        ctx: &Context,
        store: &mut SettingsStore,
        state: &mut GuiState,
        db_path: &str,
    ) -> Vec<UiAction> {
        let id = Self::id();

        if !state.show_settings {
            if self.shown {
                ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Close);
                self.shown = false;
            }
            return self.collect(store, state);
        }

        // Fresh values for the frame: the path fields can change while the
        // window is closed, and Reload re-seeds the draft from the store.
        if let Ok(mut shared) = self.shared.lock() {
            if shared.seed {
                shared.seed = false;
                shared.seeded = true;
                shared.draft = store.settings().clone();
                shared.dirty = state.settings_dirty;
            }
            shared.data_dir_input = state.data_dir_input.clone();
            shared.settings_path = store.path().display().to_string();
            shared.db_path = db_path.to_string();
        }

        let shared = Arc::clone(&self.shared);
        let palette = theme::palette_ctx(ctx);
        let sizes = Sizes::default();
        let spacing = Spacing::default();
        let builder = ViewportBuilder::default()
            .with_title("rdm — Settings")
            // A real window: the system draws the title bar, so the ✕ in the
            // top-right corner is the operating system's own control.
            .with_decorations(true)
            .with_resizable(true)
            .with_inner_size([sizes.form_width, 560.0])
            .with_min_inner_size([sizes.form_width * 0.8, 360.0])
            .with_icon(Arc::new(crate::icon::window_icon()));

        let dark = store.settings().dark_mode;
        ctx.show_viewport_deferred(id, builder, move |ctx, _class| {
            // The dark-theme switch lives in this window, so the style has to
            // follow it here as well as in the main window.
            if ctx.style().visuals.dark_mode != dark {
                theme::install(ctx, dark);
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(palette.surface))
                .show(ctx, |ui| {
                    let Ok(mut shared) = shared.lock() else {
                        return;
                    };
                    if ctx.input(|i| i.viewport().close_requested()) {
                        shared.close_requested = true;
                    }
                    tab_strip(ui, &mut shared.tab, &spacing);
                    ui.separator();
                    ui.add_space(spacing.xs);
                    let before = shared.draft.clone();
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .id_salt("settings-window-scroll")
                        .show(ui, |ui| {
                            let Shared {
                                draft,
                                data_dir_input,
                                actions,
                                tab,
                                ..
                            } = &mut *shared;
                            actions.extend(tab_body(ui, *tab, draft, data_dir_input));
                        });
                    if shared.draft != before {
                        shared.dirty = true;
                    }
                    ui.separator();
                    let footer_actions = footer(ui, &mut shared);
                    shared.actions.extend(footer_actions);
                });
        });
        self.shown = true;

        // The ✕ may have arrived with this frame's input.
        let closed = self
            .shared
            .lock()
            .map(|mut shared| {
                if shared.close_requested {
                    shared.close_requested = false;
                    shared.seed = true; // re-seed the next time it opens
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false);
        if closed {
            state.show_settings = false;
        }

        self.collect(store, state)
    }

    /// Synchronise the draft into the store and hand the queued actions to the
    /// app loop.
    fn collect(&mut self, store: &mut SettingsStore, state: &mut GuiState) -> Vec<UiAction> {
        let Ok(mut shared) = self.shared.lock() else {
            return Vec::new();
        };
        if shared.draft != *store.settings() {
            *store.settings_mut() = shared.draft.clone();
            shared.dirty = true;
        }
        if shared.seeded {
            // Only write back once the window has really been filled; before
            // that the fields hold nothing.
            state.data_dir_input = shared.data_dir_input.clone();
            state.settings_dirty = shared.dirty;
            self.tab = shared.tab;
        }

        let actions = std::mem::take(&mut shared.actions);
        // Reload means “throw my edits away”: re-copy the store on the next
        // frame, once the app has actually re-read the file.
        if actions.contains(&UiAction::ReloadSettings) {
            shared.seed = true;
        }
        actions
    }
}

/// One row of tabs; the active one is highlighted.
fn tab_strip(ui: &mut Ui, tab: &mut Tab, spacing: &Spacing) {
    ui.add_space(spacing.xs);
    ui.horizontal(|ui| {
        for candidate in Tab::ALL {
            ui.selectable_value(tab, candidate, candidate.title())
                .on_hover_text(candidate.tooltip());
        }
    });
}

/// The body of the active tab.
///
/// `data_dir_input` is the metadata-directory text field: it is applied
/// separately, through `Apply data directory`, so it is the one value that is
/// not part of the draft.
fn tab_body(
    ui: &mut Ui,
    tab: Tab,
    settings: &mut AppSettings,
    data_dir_input: &mut String,
) -> Vec<UiAction> {
    match tab {
        Tab::Downloads => downloads_tab(ui, settings, data_dir_input),
        Tab::Application => application_tab(ui, settings, data_dir_input),
        Tab::Desktop => desktop_tab(ui, settings, data_dir_input),
    }
}

/// `Downloads`: output directory and the per-download defaults.
fn downloads_tab(
    ui: &mut Ui,
    settings: &mut AppSettings,
    _data_dir_input: &mut String,
) -> Vec<UiAction> {
    let palette = theme::palette_of(ui);
    let sizes = Sizes::default();
    let spacing = Spacing::default();

    components::section_title(ui, "Defaults for new downloads");
    ui.add_space(spacing.md);

    ui.label("Download directory");
    ui.horizontal(|ui| {
        let avail = ui.available_width();
        components::text_edit(
            ui,
            &mut settings.download_dir,
            (avail - sizes.picker_reserve).max(sizes.picker_min),
            "current directory",
        );
        if components::icon_button(
            ui,
            Icon::Folder,
            "Choose a folder in the system file explorer",
        )
        .clicked()
        {
            let start = util::existing_dir(&settings.download_dir);
            if let Some(dir) = util::pick_folder(start.as_deref(), "Download directory") {
                settings.download_dir = dir.display().to_string();
            }
        }
    });
    if let Some(hint) = ux::doubled_separator_hint(&settings.download_dir) {
        components::hint(ui, &palette, hint);
    }

    ui.add_space(spacing.md);
    ui.label("Connections");
    ui.add(egui::Slider::new(&mut settings.connections, 1..=128));

    ui.add_space(spacing.xs);
    ui.label("Retries per chunk");
    ui.add(egui::DragValue::new(&mut settings.retries).range(0..=100));

    ui.add_space(spacing.md);
    ui.label("Minimum chunk size");
    components::text_edit(ui, &mut settings.chunk_size, ui.available_width(), "1MiB");

    ui.add_space(spacing.xs);
    ui.label("Speed limit");
    components::text_edit(
        ui,
        &mut settings.max_speed,
        ui.available_width(),
        "unlimited, e.g. 5MB/s",
    );

    ui.add_space(spacing.xs);
    ui.label("Connect timeout (s)");
    ui.add(egui::DragValue::new(&mut settings.timeout_secs).range(1..=3600));

    ui.add_space(spacing.xs);
    ui.label("User agent");
    components::text_edit(ui, &mut settings.user_agent, ui.available_width(), "rdm/0.1.0");

    ui.add_space(spacing.xl);
    components::section_title(ui, "Where new downloads are checked");
    ui.add_space(spacing.md);
    ui.label("Checksum / user-agent defaults are per download — see the New download form.");
    Vec::new()
}

/// `Application`: process-wide behaviour.
fn application_tab(
    ui: &mut Ui,
    settings: &mut AppSettings,
    data_dir_input: &mut String,
) -> Vec<UiAction> {
    let palette = theme::palette_of(ui);
    let sizes = Sizes::default();
    let spacing = Spacing::default();
    let mut actions = Vec::new();

    components::section_title(ui, "Metadata and scheduling");
    ui.add_space(spacing.md);

    ui.label("Metadata directory (--data-dir)");
    ui.horizontal(|ui| {
        let avail = ui.available_width();
        components::text_edit(
            ui,
            data_dir_input,
            (avail - sizes.picker_reserve).max(sizes.picker_min),
            ".rdm",
        );
        if components::icon_button(
            ui,
            Icon::Folder,
            "Choose a folder in the system file explorer",
        )
        .clicked()
        {
            let start = util::existing_dir(data_dir_input);
            if let Some(dir) = util::pick_folder(start.as_deref(), "Metadata directory") {
                *data_dir_input = dir.display().to_string();
            }
        }
    });
    if let Some(hint) = ux::doubled_separator_hint(data_dir_input) {
        components::hint(ui, &palette, hint);
    }
    if ui
        .button("Apply data directory")
        .on_hover_text("Reopens metadata.db from another folder")
        .clicked()
    {
        actions.push(UiAction::ApplyDataDir);
    }

    ui.add_space(spacing.md);
    ui.label("Max concurrent downloads")
        .on_hover_text("0 = unlimited; extra downloads wait in the queue");
    ui.add(egui::Slider::new(&mut settings.max_concurrent, 0..=16));

    ui.add_space(spacing.xs);
    ui.label("Table refresh (ms)");
    ui.add(egui::Slider::new(&mut settings.refresh_ms, 100..=5000));

    ui.add_space(spacing.xs);
    ui.checkbox(&mut settings.confirm_remove, "Confirm destructive actions")
        .on_hover_text(
            "Ask before removing downloads or restarting one from scratch; \
             the dialog always names the consequence",
        );
    ui.checkbox(
        &mut settings.purge_on_remove,
        "Delete files too when removing",
    );
    ui.checkbox(&mut settings.dark_mode, "Dark theme");

    ui.add_space(spacing.md);
    ui.label("Engine log verbosity")
        .on_hover_text("Same levels as the CLI's -v / -vv / -vvv; RUST_LOG still wins");
    egui::ComboBox::from_id_salt("log-level")
        .selected_text(settings.log_level.clone())
        .show_ui(ui, |ui| {
            for level in crate::logging::LEVELS {
                ui.selectable_value(&mut settings.log_level, level.to_string(), level);
            }
        });
    actions
}

/// `Desktop integration`: the three switches the tray also toggles.
fn desktop_tab(
    ui: &mut Ui,
    settings: &mut AppSettings,
    _data_dir_input: &mut String,
) -> Vec<UiAction> {
    let spacing = Spacing::default();

    components::section_title(ui, "Desktop integration");
    ui.add_space(spacing.md);
    ui.checkbox(
        &mut settings.clipboard_prefill,
        "Fill the URL from the clipboard",
    )
    .on_hover_text(
        "When “New download” opens, a link on the clipboard is inserted \
         and selected, so Start (or Enter) begins it immediately",
    );
    ui.checkbox(&mut settings.close_to_tray, "Keep running in the tray")
        .on_hover_text(
            "Closing the window hides it instead of quitting; the transfer \
             keeps running and the tray menu brings it back. The tray's \
             “Quit rdm” really exits",
        );
    ui.checkbox(&mut settings.drop_target_enabled, "Floating drop target")
        .on_hover_text(
            "A small always-on-top box above the taskbar clock: drop a link \
             on it (from a browser) and the New download form opens with it",
        );

    ui.add_space(spacing.xl);
    components::section_title(ui, "Same three switches in the tray");
    ui.add_space(spacing.md);
    ui.label(
        "The tray menu toggles these too, so they stay reachable while the main \
         window is hidden.",
    );
    Vec::new()
}

/// The window footer: Save / Reload, the unsaved-changes banner and the paths.
fn footer(ui: &mut Ui, shared: &mut Shared) -> Vec<UiAction> {
    let palette = theme::palette_of(ui);
    let spacing = Spacing::default();
    let mut actions = Vec::new();

    ui.add_space(spacing.xs);
    ui.horizontal(|ui| {
        if ui
            .button("Save")
            .on_hover_text("Writes settings.toml")
            .clicked()
        {
            actions.push(UiAction::SaveSettings);
        }
        if ui
            .button("Reload")
            .on_hover_text("Discards the edits and re-reads settings.toml")
            .clicked()
        {
            shared.seed = true;
            actions.push(UiAction::ReloadSettings);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if shared.dirty {
                components::banner(
                    ui,
                    &palette,
                    components::Level::Warning,
                    "unsaved changes",
                );
            }
        });
    });
    ui.add_space(spacing.xs);
    components::hint(ui, &palette, &shared.settings_path);
    components::hint(ui, &palette, &shared.db_path);
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (tempfile::TempDir, SettingsStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path(), false);
        (dir, store)
    }

    fn window_with(draft: AppSettings) -> SettingsWindow {
        let window = SettingsWindow::new();
        {
            let mut shared = window.shared.lock().unwrap();
            shared.data_dir_input = draft.data_dir.clone();
            shared.draft = draft;
            shared.seed = false;
            shared.seeded = true;
        }
        window
    }

    #[test]
    fn every_section_has_a_tab_with_a_title_and_tooltip() {
        assert_eq!(Tab::ALL.len(), 3, "one tab per section");
        for tab in Tab::ALL {
            assert!(!tab.title().is_empty(), "{tab:?} has no title");
            assert!(!tab.tooltip().is_empty(), "{tab:?} has no tooltip");
        }
        // The desktop switches — the point of the increment — have their own
        // tab, and it is not the first one.
        assert_eq!(Tab::Desktop.title(), "Desktop integration");
        assert_eq!(Tab::ALL[0], Tab::Downloads);
    }

    #[test]
    fn the_help_text_keeps_up_with_the_window() {
        // The user asked for the desktop switches to be findable while the
        // window is hidden in the tray; the help text is what points there, so
        // it has to name the tab they live in.
        let help = crate::ux::help_sections()
            .into_iter()
            .map(|(heading, lines)| format!("{heading} {}", lines.join(" ")))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            help.contains(Tab::Desktop.title()),
            "the help text no longer names the {:?} tab",
            Tab::Desktop
        );
    }

    #[test]
    fn an_edit_reaches_the_store_and_flags_itself_dirty() {
        let (_dir, mut store) = temp_store();
        store.settings_mut().connections = 4;
        let mut window = window_with({
            let mut draft = store.settings().clone();
            draft.connections = 12;
            draft.drop_target_enabled = true;
            draft
        });
        let mut state = GuiState::new(
            store.settings().to_request(),
            store.path().display().to_string(),
        );

        window.collect(&mut store, &mut state);

        assert_eq!(store.settings().connections, 12, "the edit reached the store");
        assert!(store.settings().drop_target_enabled);
        assert!(state.settings_dirty, "unsaved changes are flagged");
        assert_eq!(
            state.data_dir_input,
            store.settings().data_dir.clone(),
            "the metadata field travels with the draft"
        );
    }

    #[test]
    fn a_clean_draft_leaves_the_store_untouched() {
        let (_dir, mut store) = temp_store();
        store.settings_mut().connections = 7;
        let mut window = window_with(store.settings().clone());
        let mut state = GuiState::new(
            store.settings().to_request(),
            store.path().display().to_string(),
        );

        let actions = window.collect(&mut store, &mut state);

        assert_eq!(store.settings().connections, 7);
        assert!(!state.settings_dirty, "an untouched window stays clean");
        assert!(actions.is_empty());
    }

    #[test]
    fn a_window_that_never_opened_leaves_the_view_model_alone() {
        let (_dir, mut store) = temp_store();
        let mut window = SettingsWindow::new();
        let mut state = GuiState::new(
            store.settings().to_request(),
            store.path().display().to_string(),
        );
        let before = state.data_dir_input.clone();

        window.collect(&mut store, &mut state);

        assert_eq!(
            state.data_dir_input, before,
            "an unseeded window must not wipe the metadata field"
        );
    }

    #[test]
    fn queued_actions_are_handed_over_once_and_reload_reseeds() {
        let (_dir, mut store) = temp_store();
        let mut window = window_with(store.settings().clone());
        let mut state = GuiState::new(
            store.settings().to_request(),
            store.path().display().to_string(),
        );
        {
            let mut shared = window.shared.lock().unwrap();
            shared.actions.push(UiAction::SaveSettings);
            shared.actions.push(UiAction::ReloadSettings);
            shared.dirty = true;
        }

        let actions = window.collect(&mut store, &mut state);
        assert_eq!(
            actions,
            vec![UiAction::SaveSettings, UiAction::ReloadSettings],
            "the app loop gets both actions, in the order they were queued"
        );
        assert!(
            window.shared.lock().unwrap().seed,
            "Reload asks for a fresh copy of the store"
        );

        let again = window.collect(&mut store, &mut state);
        assert!(again.is_empty(), "actions are handed over only once");
    }
}
