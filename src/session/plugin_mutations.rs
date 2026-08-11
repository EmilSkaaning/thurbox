//! The write seam between a plugin VM and thurbox's records.
//!
//! The mirror of [`super::plugin_store`], for the same two reasons: `plugin` may
//! not import `storage` (architecture rule), and a `rusqlite::Connection` cannot
//! be shared across threads — so the trait lives here in the pure-data layer,
//! `storage` implements it, and a VM builds one **on its own thread**.
//!
//! Every method changes exactly one existing record, addressed by its id, and
//! reports whether that record was there. Nothing here creates a record, edits
//! its text, names a program, or executes anything: the operation list is closed
//! on purpose, because a plugin's reach has to be reviewable from its manifest
//! and a general mechanism would make it unreviewable.
//!
//! **One trait for two capabilities.** The trait is host-side plumbing, not the
//! grant surface: what a plugin holds is decided by which *bindings*
//! `plugin::capabilities` inserts, and denial is the binding's absence. Two
//! traits would double the factory threaded through the runtime, the lifecycle,
//! the service host and four call sites to express a distinction the capability
//! check already makes.

use std::sync::Arc;

use super::task::TaskStatus;

/// A plugin's view of the records it may change.
///
/// Task and automation ids are the kernel's own row ids, which is what the
/// published snapshots already carry — so a pane acts on the row it drew rather
/// than on something it had to resolve.
pub trait KernelWriter: Send {
    /// Set a task's status. `false` when no such (undeleted) task exists.
    fn set_task_status(&self, id: i64, status: TaskStatus) -> Result<bool, String>;

    /// Soft-delete a task, as the tasks pane's delete key does. `false` when it
    /// was already gone.
    fn delete_task(&self, id: i64) -> Result<bool, String>;

    /// Enable or disable an automation, recomputing or clearing its next
    /// occurrence to match. `false` when no such automation exists.
    fn set_automation_enabled(&self, id: i64, enabled: bool) -> Result<bool, String>;

    /// Mark an automation due so the **kernel** fires it on its next pass.
    ///
    /// Deliberately not "run it": the plugin's thread never executes an action,
    /// spawns a process, or opens a session. Marking a run that is already
    /// pending is the same request, not a second run.
    fn run_automation(&self, id: i64) -> Result<bool, String>;

    /// Delete an automation and its run history. `false` when it was already
    /// gone.
    fn delete_automation(&self, id: i64) -> Result<bool, String>;
}

/// Builds a [`KernelWriter`] on the calling thread.
///
/// A factory rather than a writer, for [`super::plugin_store::PluginStoreFactory`]'s
/// reason: the implementation owns a database connection, connections are not
/// shared across threads, and `None` means storage is unavailable — a plugin
/// granted a write capability then finds its binding failing rather than the host
/// refusing to start.
pub type KernelWriterFactory = Arc<dyn Fn() -> Option<Box<dyn KernelWriter>> + Send + Sync>;

/// Parse a status name a plugin passed, naming the accepted values on failure.
///
/// Distinct from [`TaskStatus::from_db`], which defaults an unknown value to
/// `Todo` because a stored row must always resolve to something. A plugin's
/// argument is different: silently writing `todo` when it asked for `Done` would
/// be a wrong write rather than a resilient read.
pub fn parse_task_status(name: &str) -> Result<TaskStatus, String> {
    match name {
        "todo" => Ok(TaskStatus::Todo),
        "in_progress" => Ok(TaskStatus::InProgress),
        "done" => Ok(TaskStatus::Done),
        other => Err(format!(
            "unknown task status `{other}` (expected `todo`, `in_progress` or `done`)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_the_kernel_stores_is_parsable() {
        for status in [TaskStatus::Todo, TaskStatus::InProgress, TaskStatus::Done] {
            assert_eq!(parse_task_status(status.as_str()), Ok(status));
        }
    }

    #[test]
    fn an_unknown_status_names_what_is_accepted() {
        let err = parse_task_status("finished").expect_err("not a status");
        assert!(err.contains("finished"), "{err}");
        assert!(err.contains("in_progress"), "{err}");
    }

    /// The human label is not the wire name, and accepting it would let
    /// `in progress` write a status the pane never displays.
    #[test]
    fn a_display_label_is_not_accepted() {
        assert!(parse_task_status("in progress").is_err());
    }
}
