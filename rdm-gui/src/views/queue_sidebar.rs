//! Queue sidebar — jobs waiting for a free slot (toggled from the top menu).

use egui::{Align, Layout, RichText, Ui};

use crate::state::{GuiState, UiAction};
use crate::theme::{self, components, Palette, Spacing};
use crate::ux::{self, BulkAction};

pub fn show(ui: &mut Ui, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let palette = theme::palette_of(ui);
    let spacing = Spacing::default();

    ui.add_space(spacing.xs);
    ui.horizontal(|ui| {
        ui.heading("Queue");
        components::hint(ui, &palette, format!("{} queued", state.queue.len()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if components::icon_button(ui, theme::Icon::Close, "Hide the queue sidebar").clicked() {
                state.show_queue = false;
            }
            if !state.queue.is_empty()
                && ui
                    .button(BulkAction::DropQueue.to_string())
                    .on_hover_text(ux::drop_queue_tooltip())
                    .clicked()
            {
                actions.push(UiAction::AskDropQueue);
            }
        });
    });
    ui.separator();
    ui.add_space(spacing.xs);

    if state.queue.is_empty() {
        components::hint(
            ui,
            &palette,
            "Nothing queued — new downloads start straight away.",
        );
        return actions;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .id_salt("queue-scroll")
        .show(ui, |ui| {
            for (position, job) in state.queue.iter().enumerate() {
                job_row(
                    ui,
                    position,
                    job.seq,
                    &job.label,
                    &job.url,
                    &job.output,
                    &palette,
                    &mut actions,
                );
                ui.add_space(spacing.xs);
            }
        });

    actions
}

#[allow(clippy::too_many_arguments)]
fn job_row(
    ui: &mut Ui,
    position: usize,
    seq: u64,
    label: &str,
    url: &str,
    output: &str,
    palette: &Palette,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal(|ui| {
        components::meta(ui, palette, format!("{}.", position + 1));
        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
            if components::icon_button(
                ui,
                theme::Icon::Close,
                "Drop from the queue — nothing has been downloaded yet",
            ).clicked() {
                actions.push(UiAction::DropQueued(seq));
            }
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(label)
                        .small()
                        .strong()
                        .color(palette.text_primary),
                );
                ui.set_min_width(ui.available_width());
                components::hint(ui, palette, url);
                components::hint(
                    ui,
                    palette,
                    if output.is_empty() {
                        "(default directory)"
                    } else {
                        output
                    },
                );
            });
        });
    });
}
