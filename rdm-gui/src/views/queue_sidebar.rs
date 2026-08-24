//! Queue sidebar — jobs waiting for a free slot (toggled from the top menu).

use egui::{Align, Layout, RichText, Ui};

use crate::state::{GuiState, UiAction};

pub fn show(ui: &mut Ui, state: &mut GuiState) -> Vec<UiAction> {
    let mut actions = Vec::new();

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.heading("Queue");
        ui.label(
            RichText::new(format!("{} waiting", state.queue.len()))
                .small()
                .color(ui.visuals().weak_text_color()),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .button("✕")
                .on_hover_text("Hide the queue sidebar")
                .clicked()
            {
                state.show_queue = false;
            }
            if !state.queue.is_empty() && ui.small_button("Clear queue").clicked() {
                actions.push(UiAction::ClearQueue);
            }
        });
    });
    ui.separator();
    ui.add_space(4.0);

    if state.queue.is_empty() {
        ui.label(
            RichText::new("The queue is empty — new downloads start immediately.")
                .italics()
                .color(ui.visuals().weak_text_color()),
        );
        return actions;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .id_salt("queue-scroll")
        .show(ui, |ui| {
            for (position, job) in state.queue.iter().enumerate() {
                job_row(ui, position, job.seq, &job.label, &job.url, &job.output, &mut actions);
                ui.add_space(4.0);
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
    actions: &mut Vec<UiAction>,
) {
    let weak = ui.visuals().weak_text_color();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{}.", position + 1))
                .monospace()
                .small()
                .color(weak),
        );
        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
            if ui
                .small_button("✕")
                .on_hover_text("Drop from queue")
                .clicked()
            {
                actions.push(UiAction::CancelPending(seq));
            }
            ui.vertical(|ui| {
                ui.label(RichText::new(label).small().strong());
                ui.set_min_width(ui.available_width());
                ui.label(RichText::new(url).small().color(weak));
                ui.label(
                    RichText::new(if output.is_empty() {
                        "(default directory)"
                    } else {
                        output
                    })
                    .small()
                    .color(weak),
                );
            });
        });
    });
}
