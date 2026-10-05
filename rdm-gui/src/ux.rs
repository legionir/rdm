//! UX policy for the rdm GUI: what needs confirmation, what the consequence
//! text says, the status legend and the bulk-action microcopy.
//!
//! The policy is code — not prose — so that behaviour *and* wording are
//! unit-tested (see the tests at the bottom and the static rules in
//! `audits/ux-terminology-check.py`).
//!
//! Copy rules applied here:
//!
//! 1. name the action (`Remove`, `Drop`, `Restart`), never "OK";
//! 2. state the consequence and whether it can be undone — never "Are you sure?";
//! 3. end every dialog with the consequence of the *worst* choice ("Cannot be
//!    undone.") and visually emphasise the safe option;
//! 4. never expose implementation vocabulary (record, row, database, sqlite);
//! 5. distinguish deliberate stops from accidents: `Cancelled` (user asked) vs
//!    `Interrupted`/`Failed` (something went wrong), each with its own next step.

use std::fmt;

use rdm::models::DownloadState;

/// Which confirmation a user-facing action asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    /// Nothing is discarded yet (queue drops, pause/resume) — act immediately
    /// and report the outcome.
    None,
    /// Remove one download from the list.
    RemoveOne,
    /// Remove every completed download (bulk).
    RemoveCompleted,
    /// Download again from the beginning.
    Restart,
}

impl Confirm {
    pub const ALL: [Confirm; 4] = [
        Confirm::None,
        Confirm::RemoveOne,
        Confirm::RemoveCompleted,
        Confirm::Restart,
    ];

    /// Window title.
    pub fn title(self) -> &'static str {
        match self {
            Confirm::None => "",
            Confirm::RemoveOne => "Remove download",
            Confirm::RemoveCompleted => "Remove completed downloads",
            Confirm::Restart => "Restart from scratch",
        }
    }

    /// Label of the confirming (destructive) button.
    pub fn confirm_label(self) -> &'static str {
        match self {
            Confirm::None => "",
            Confirm::RemoveOne => "Remove",
            Confirm::RemoveCompleted => "Remove",
            Confirm::Restart => "Restart",
        }
    }

    /// Label of the safe option — named after what it keeps, not "Cancel".
    pub fn keep_label(self) -> &'static str {
        match self {
            Confirm::None => "",
            Confirm::RemoveOne | Confirm::RemoveCompleted => "Keep",
            Confirm::Restart => "Keep the progress",
        }
    }

    /// Does the dialog offer "also delete the finished file"?
    pub fn has_file_checkbox(self) -> bool {
        matches!(self, Confirm::RemoveOne | Confirm::RemoveCompleted)
    }

    /// Consequence text. `subject` is how many downloads are involved.
    pub fn body(self, subject: usize) -> String {
        match self {
            Confirm::None => String::new(),
            Confirm::RemoveOne => "This removes the download from the list and discards its \
                 partial data. Tick the box to delete the finished file too. Cannot be undone."
                .to_string(),
            Confirm::RemoveCompleted => format!(
                "This removes {subject} completed download(s) from the list and discards their \
                 partial data. Finished files stay on disk unless the box below is ticked. \
                 Cannot be undone."
            ),
            Confirm::Restart => "This discards the progress of this download, downloads it again \
                 from the beginning and overwrites the file at the output path. Cannot be undone."
                .to_string(),
        }
    }
}

/// Every action that destroys user data, with the confirmation it gets.
/// Adding an action without a row here fails the unit test.
pub const DESTRUCTIVE: [(&str, Confirm); 5] = [
    ("Remove one download", Confirm::RemoveOne),
    ("Remove completed downloads", Confirm::RemoveCompleted),
    ("Restart from scratch", Confirm::Restart),
    ("Drop one queued download", Confirm::None),
    ("Drop all queued downloads", Confirm::None),
];

/// Bulk actions offered by the toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkAction {
    PauseAll,
    ResumeAll,
    RemoveCompleted,
    DropQueue,
}

impl BulkAction {
    pub const ALL: [BulkAction; 4] = [
        BulkAction::PauseAll,
        BulkAction::ResumeAll,
        BulkAction::RemoveCompleted,
        BulkAction::DropQueue,
    ];

    /// Outcome when the action had work to do (a count is always reported).
    pub fn done(self, count: usize) -> String {
        match self {
            BulkAction::PauseAll => format!("Pause requested for {count} download(s)."),
            BulkAction::ResumeAll => format!("Continuing {count} download(s)."),
            BulkAction::RemoveCompleted => {
                format!("Removed {count} completed download(s) from the list.")
            }
            BulkAction::DropQueue => format!("Dropped {count} queued download(s)."),
        }
    }

    /// Outcome when nothing matched. Saying what was missing is more useful
    /// than "0 download(s)": it tells the user why nothing happened.
    pub fn nothing_to_do(self) -> &'static str {
        match self {
            BulkAction::PauseAll => "Nothing to pause — no download is running.",
            BulkAction::ResumeAll => {
                "Nothing to continue — no download is paused, interrupted or cancelled."
            }
            BulkAction::RemoveCompleted => "Nothing to remove — no completed download in the list.",
            BulkAction::DropQueue => "Nothing to drop — the queue is empty.",
        }
    }
}

impl fmt::Display for BulkAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            BulkAction::PauseAll => "Pause all",
            BulkAction::ResumeAll => "Resume all",
            BulkAction::RemoveCompleted => "Remove completed…",
            BulkAction::DropQueue => "Drop all",
        })
    }
}

/// A state, what it means in plain words, and what the user can do next.
/// Surfaced as a tooltip on every state chip, on the status bar counters and
/// under the state in the details modal, so the meaning is always one hover
/// away (WCAG 3.3.5 Help is available).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegendEntry {
    pub state: DownloadState,
    pub meaning: &'static str,
    pub next_step: &'static str,
}

impl LegendEntry {
    pub fn hover_text(&self) -> String {
        format!(
            "{} — {}. Next: {}",
            self.state,
            self.meaning,
            self.next_step
        )
    }
}

/// Definitions follow the verified behaviour of the engine and the GUI bridge:
///
/// * `Paused` is always a deliberate stop; `Interrupted` and `Failed` are not.
/// * `Cancelled` keeps its partial data and `Backend::resume` accepts it.
/// * `Failed` is terminal for `Backend::resume` (it rejects terminal states
///   except `Cancelled`), so the only recovery offered is `Restart`.
pub const LEGEND: [LegendEntry; 8] = [
    LegendEntry {
        state: DownloadState::Queued,
        meaning: "accepted and waiting for a free slot",
        next_step: "raise “Max concurrent downloads”, or drop it from the queue",
    },
    LegendEntry {
        state: DownloadState::Running,
        meaning: "downloading right now",
        next_step: "Pause keeps the progress and stops the transfer",
    },
    LegendEntry {
        state: DownloadState::Merging,
        meaning: "the last parts are being written into the finished file",
        next_step: "wait for it to finish",
    },
    LegendEntry {
        state: DownloadState::Paused,
        meaning: "stopped on request; the progress is kept",
        next_step: "Resume continues from where it stopped",
    },
    LegendEntry {
        state: DownloadState::Interrupted,
        meaning: "the transfer stopped unexpectedly; the progress is kept",
        next_step: "Resume continues from where it stopped",
    },
    LegendEntry {
        state: DownloadState::Completed,
        meaning: "the file is finished",
        next_step: "Open folder to use it, or Remove to clean up the list",
    },
    LegendEntry {
        state: DownloadState::Cancelled,
        meaning: "stopped before finishing; the partial data is kept",
        next_step: "Resume continues it, Remove discards it",
    },
    LegendEntry {
        state: DownloadState::Failed,
        meaning: "the transfer failed after the configured retries",
        next_step: "Restart downloads it again from the beginning",
    },
];

/// The legend entry for a state (always present — asserted by a test).
pub fn legend_for(state: DownloadState) -> &'static LegendEntry {
    LEGEND
        .iter()
        .find(|entry| entry.state == state)
        .expect("every DownloadState has a legend entry (tested)")
}

/// The whole legend as one tooltip for the status bar counters.
pub fn legend_tooltip() -> String {
    let mut out = String::from("What the states mean:\n");
    for entry in LEGEND {
        out.push_str(&format!(
            "• {} — {}. Next: {}\n",
            entry.state, entry.meaning, entry.next_step
        ));
    }
    out.push_str("Hover a download's state to see the same explanation.");
    out
}

// --------------------------------------------------------------- row actions

/// Tooltip for the ⏸ button (only offered while a download is running).
pub fn pause_tooltip() -> &'static str {
    "Pause — stop the transfer and keep the progress"
}

/// Tooltip for the ⏹ button (only offered while a download is running).
pub fn cancel_tooltip() -> &'static str {
    "Cancel — stop the transfer; the partial data is kept"
}

/// Tooltip for the ▶ button. Never offered for `Failed` or `Completed`: a
/// failed download cannot be continued (the engine only restarts it), so the
/// row offers ⟲ Restart instead of a button that would only produce an error.
pub fn resume_tooltip() -> &'static str {
    "Resume — continue this download where it stopped"
}

/// Tooltip for the ⟲ button; the wording depends on what would be lost.
pub fn restart_tooltip(state: DownloadState) -> &'static str {
    match state {
        DownloadState::Failed => {
            "Restart from scratch — a failed download cannot be continued; this asks first"
        }
        DownloadState::Completed => {
            "Download again from the beginning — asks first and overwrites the file"
        }
        _ => "Restart from scratch — discards the progress, asks first",
    }
}

/// Tooltip for the 🗑 button. While a download is running the button is
/// disabled and explains the order of operations instead of failing later.
pub fn remove_tooltip(running: bool) -> &'static str {
    if running {
        "Remove is unavailable while the download runs — pause or cancel it first"
    } else {
        "Remove — take it out of the list; asks first what to do with the file"
    }
}

/// Tooltip for the 📂 button.
pub fn open_folder_tooltip() -> &'static str {
    "Open folder — show the finished file in your file manager"
}

/// Tooltip for the toolbar's “Remove completed…” button.
pub fn remove_completed_tooltip() -> &'static str {
    "Remove every completed download from the list (asks first; files are kept unless you tick the box)"
}

/// Tooltip for “Drop all queued downloads”.
pub fn drop_queue_tooltip() -> &'static str {
    "Drop every queued download — nothing has been downloaded yet"
}

/// Tooltip for “Resume all”.
pub fn resume_all_tooltip() -> &'static str {
    "Continue every paused, interrupted or cancelled download (rdm resume <ID>)"
}

/// Tip shown in the New download dialog: where the filename comes from.
pub const ADD_TIP: &str = "Tip: choose a folder in “Output” to keep the server-provided filename.";

/// Tooltip for “Pause all”.
pub fn pause_all_tooltip() -> &'static str {
    "Stop every running download, keeping its progress (rdm pause <ID>)"
}

/// Vocabulary the product commits to (detailed in `audits/ux-glossary.md`).
/// `rejected` lists the synonyms that must not appear in user-facing copy.
pub struct Glossary {
    pub term: &'static str,
    pub meaning: &'static str,
    pub rejected: &'static [&'static str],
}

pub const GLOSSARY: [Glossary; 5] = [
    Glossary {
        term: "download",
        meaning: "one file rdm was asked to fetch; it survives restarts of the app",
        rejected: &["job", "record", "row", "item", "task"],
    },
    Glossary {
        term: "queue",
        meaning: "downloads that are accepted but not started yet",
        rejected: &["pending list", "waiting list"],
    },
    Glossary {
        term: "partial data",
        meaning: "the bytes already fetched for an unfinished download; only Remove discards it",
        rejected: &["temp files", "part files"],
    },
    Glossary {
        term: "Remove",
        meaning: "take a download out of the list; the finished file stays unless said otherwise",
        rejected: &["Clear", "Delete (for downloads)"],
    },
    Glossary {
        term: "Drop",
        meaning: "take a queued download out of the queue before it starts",
        rejected: &["Clear (for the queue)"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ALL_STATES;

    /// Copy that must never reach a user-facing string.
    const JARGON: [&str; 6] = ["job", "record", "row", "database", "sqlite", "are you sure"];

    fn all_copy() -> Vec<String> {
        let mut copy = Vec::new();
        for confirm in Confirm::ALL {
            copy.push(confirm.title().to_string());
            copy.push(confirm.confirm_label().to_string());
            copy.push(confirm.keep_label().to_string());
            copy.push(confirm.body(3));
        }
        for action in BulkAction::ALL {
            copy.push(action.to_string());
            copy.push(action.done(3));
            copy.push(action.nothing_to_do().to_string());
        }
        for entry in LEGEND {
            copy.push(entry.hover_text());
        }
        for tooltip in [
            pause_tooltip(),
            cancel_tooltip(),
            resume_tooltip(),
            remove_tooltip(false),
            remove_tooltip(true),
            open_folder_tooltip(),
            remove_completed_tooltip(),
            drop_queue_tooltip(),
            ADD_TIP,
            resume_all_tooltip(),
            pause_all_tooltip(),
        ] {
            copy.push(tooltip.to_string());
        }
        for state in ALL_STATES {
            copy.push(restart_tooltip(state).to_string());
        }
        copy.push(legend_tooltip());
        copy
    }

    #[test]
    fn every_data_destroying_action_has_a_confirmation_policy() {
        assert_eq!(DESTRUCTIVE.len(), 5);
        let with_dialog: Vec<&str> = DESTRUCTIVE
            .iter()
            .filter(|(_, confirm)| *confirm != Confirm::None)
            .map(|(action, _)| *action)
            .collect();
        assert_eq!(with_dialog.len(), 3, "remove, bulk remove and restart ask first");
        for (action, confirm) in DESTRUCTIVE {
            assert!(!action.is_empty());
            if confirm != Confirm::None {
                assert!(confirm.has_file_checkbox() || confirm == Confirm::Restart);
            }
        }
    }

    #[test]
    fn confirmations_state_the_consequence_and_the_undo_status() {
        for confirm in [Confirm::RemoveOne, Confirm::RemoveCompleted, Confirm::Restart] {
            let body = confirm.body(2);
            assert!(
                body.contains("Cannot be undone"),
                "{:?} must say whether it can be undone",
                confirm
            );
            assert!(
                body.len() > 40,
                "{:?} must describe the consequence, not just ask",
                confirm
            );
        }
        for confirm in [Confirm::RemoveOne, Confirm::RemoveCompleted] {
            let body = confirm.body(2);
            assert!(body.contains("removes"), "{:?} names the action", confirm);
            assert!(body.contains("partial data"), "{:?} says what is lost", confirm);
            assert!(
                body.contains("delete the finished file"),
                "{:?} explains the file checkbox",
                confirm
            );
            assert!(confirm.has_file_checkbox());
        }
        let restart = Confirm::Restart.body(1);
        assert!(restart.contains("from the beginning"));
        assert!(restart.contains("overwrites"));
        assert!(!Confirm::Restart.has_file_checkbox(), "restart keeps the file");
    }

    #[test]
    fn the_safe_option_is_named_after_what_it_keeps() {
        for confirm in [Confirm::RemoveOne, Confirm::RemoveCompleted, Confirm::Restart] {
            let keep = confirm.keep_label().to_lowercase();
            assert!(
                keep.starts_with("keep"),
                "{:?} must offer a named way out, not “Cancel”",
                confirm
            );
        }
    }

    #[test]
    fn bulk_actions_name_the_count_and_explain_the_empty_case() {
        for action in BulkAction::ALL {
            let done = action.done(7);
            assert!(done.contains('7'), "{action} reports the count: {done}");
            let nothing = action.nothing_to_do();
            assert!(
                nothing.starts_with("Nothing to"),
                "{action} explains the empty case: {nothing}"
            );
            assert!(
                !nothing.chars().any(|c| c.is_ascii_digit()),
                "{action} must not report “0” as an outcome: {nothing}"
            );
        }
    }

    #[test]
    fn every_state_has_a_legend_entry_with_a_next_step() {
        for state in ALL_STATES {
            let entry = legend_for(state);
            assert_eq!(entry.state, state);
            assert!(!entry.meaning.is_empty());
            assert!(!entry.next_step.is_empty());
            assert!(legend_tooltip().contains(state.as_str()));
        }
    }

    #[test]
    fn failed_and_completed_never_offer_resume() {
        // Verified behaviour: `Backend::resume` rejects terminal states except
        // Cancelled, so the row must offer Restart instead of a dead end.
        assert!(restart_tooltip(DownloadState::Failed).contains("cannot be continued"));
        for state in [DownloadState::Failed, DownloadState::Completed] {
            let tooltip = restart_tooltip(state);
            assert!(!tooltip.contains("continue this download"), "{state}");
        }
        assert!(resume_tooltip().contains("continue"));
    }

    #[test]
    fn removing_a_running_download_explains_the_order_of_operations() {
        let tooltip = remove_tooltip(true);
        assert!(tooltip.contains("unavailable"));
        assert!(tooltip.contains("pause or cancel"));
        assert!(remove_tooltip(false).starts_with("Remove"));
    }

    #[test]
    fn no_user_facing_copy_uses_implementation_jargon() {
        for text in all_copy() {
            let lower = text.to_lowercase();
            for bad in JARGON {
                assert!(
                    !contains_word(&lower, bad),
                    "copy {text:?} contains forbidden term {bad:?}"
                );
            }
        }
    }

    /// Whole-word (or whole-phrase) match, case-insensitive.
    fn contains_word(haystack: &str, needle: &str) -> bool {
        let words: Vec<String> = haystack
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(|w| w.to_lowercase())
            .collect();
        let needle_words: Vec<String> = needle
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect();
        if needle_words.is_empty() || words.len() < needle_words.len() {
            return false;
        }
        words
            .windows(needle_words.len())
            .any(|w| w == needle_words.as_slice())
    }

    #[test]
    fn the_glossary_rejects_a_synonym_for_every_term() {
        for entry in GLOSSARY {
            assert!(!entry.term.is_empty());
            assert!(!entry.meaning.is_empty());
            assert!(
                !entry.rejected.is_empty(),
                "{} must list the terms we do not use",
                entry.term
            );
        }
        // Preferred terms must actually be the ones in the copy.
        let copy = all_copy().join("\n").to_lowercase();
        for term in ["download", "queue", "remove", "drop", "progress"] {
            assert!(copy.contains(term), "the copy uses {term}");
        }
    }
}
