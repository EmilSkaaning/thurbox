//! A plugin's write, end to end: from Luau, through the granted binding, into the
//! database — and refused when the capability is absent.
//!
//! An integration test rather than a unit test inside `src/plugin/`, and not by
//! preference: `tests/architecture_rules.rs` forbids `plugin` from referencing
//! `storage` in *any* form, tests included, and proving that a write lands needs
//! both halves. Here the two meet in a consumer of the public API, which is also
//! how a real host wires them.

#![cfg(feature = "plugins")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use thurbox::plugin::{ExecutionBounds, PluginHost};
use thurbox::session::automation::{AutomationAction, AutomationSchedule};
use thurbox::session::plugin_mutations::KernelWriter;
use thurbox::session::task::TaskStatus;
use thurbox::storage::automations::NewAutomation;
use thurbox::storage::plugins::DbKernelWriter;
use thurbox::storage::tasks::NewTask;
use thurbox::storage::Database;

/// A plugin whose `init` performs one mutation, so starting the host is the whole
/// exercise: `init` is the only entry point that needs no pane, no key and no
/// command, which keeps the test about the binding rather than about routing.
fn write_plugin(root: &Path, name: &str, capabilities: &str, body: &str) {
    let dir = root.join(name);
    fs::create_dir_all(&dir).expect("plugin dir");
    fs::write(
        dir.join("plugin.toml"),
        format!("name = \"{name}\"\napi_version = 1\ncapabilities = [{capabilities}]\n"),
    )
    .expect("manifest");
    fs::write(
        dir.join("init.luau"),
        format!("local thurbox = require(\"@thurbox\")\nreturn {{ init = function() {body} end }}"),
    )
    .expect("entry");
}

/// A factory opening the test database — the shape `main.rs` uses, with a path
/// that is not the user's.
fn writer_factory(path: PathBuf) -> thurbox::session::plugin_mutations::KernelWriterFactory {
    Arc::new(move || {
        Database::open(&path)
            .ok()
            .map(|db| Box::new(DbKernelWriter::from_db(db)) as Box<dyn KernelWriter>)
    })
}

/// Start every plugin under `plugins_root`, able to write to `db_path`.
fn run_host(plugins_root: &Path, db_path: &Path) -> PluginHost {
    let mut host = PluginHost::from_discovery(
        thurbox::plugin::discovery::discover_in(&[], Some(plugins_root)),
        ExecutionBounds::default(),
    )
    .with_kernel_writer(writer_factory(db_path.to_path_buf()));
    host.start_all();
    host
}

struct Fixture {
    _tmp: tempfile::TempDir,
    plugins: PathBuf,
    db_path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let plugins = tmp.path().join("plugins");
        fs::create_dir_all(&plugins).expect("plugins dir");
        let db_path = tmp.path().join("thurbox.db");
        // Created up front so the schema exists before a plugin writes.
        Database::open(&db_path).expect("database");
        Self {
            _tmp: tmp,
            plugins,
            db_path,
        }
    }

    fn db(&self) -> Database {
        Database::open(&self.db_path).expect("database")
    }
}

#[test]
fn a_granted_plugin_sets_a_task_status_and_deletes_a_task() {
    let f = Fixture::new();
    let db = f.db();
    let done = db.create_task(&NewTask::local("finish it")).expect("task");
    let gone = db.create_task(&NewTask::local("drop it")).expect("task");
    drop(db);

    write_plugin(
        &f.plugins,
        "writer",
        "\"tasks-write\"",
        &format!("thurbox.setTaskStatus({done}, \"done\") thurbox.deleteTask({gone})"),
    );
    let host = run_host(&f.plugins, &f.db_path);
    assert_eq!(host.problems(), &[] as &[String], "plugin should have run");

    let db = f.db();
    assert_eq!(
        db.get_task(done).unwrap().expect("still there").status,
        TaskStatus::Done
    );
    assert!(
        db.get_task(gone).unwrap().is_none(),
        "a deleted task is gone from the list"
    );
    // Soft-deleted, like the native pane's delete key — and audited, because the
    // binding goes through the same storage operation the pane uses.
    let audit = db
        .get_audit_log(
            Some(thurbox::storage::audit::EntityType::Task),
            Some(&gone.to_string()),
            50,
        )
        .expect("audit log");
    assert!(
        !audit.is_empty(),
        "a plugin's deletion is auditable, like the pane's"
    );
}

#[test]
fn a_plugin_without_the_capability_has_no_binding_to_call() {
    let f = Fixture::new();
    let db = f.db();
    let id = db.create_task(&NewTask::local("untouched")).expect("task");
    drop(db);

    // Declaring nothing: denial is the binding's absence, so the call is an
    // attempt to index a nil value rather than a refusal.
    write_plugin(
        &f.plugins,
        "reader",
        "",
        &format!("thurbox.setTaskStatus({id}, \"done\")"),
    );
    let host = run_host(&f.plugins, &f.db_path);

    let status = host.status("reader").expect("discovered");
    assert!(
        status.state.is_failed(),
        "calling an absent binding must fail the plugin, not the write"
    );
    assert_eq!(
        f.db().get_task(id).unwrap().expect("still there").status,
        TaskStatus::Todo,
        "nothing may have changed"
    );
}

#[test]
fn a_granted_plugin_enables_disables_and_runs_an_automation() {
    let f = Fixture::new();
    let db = f.db();
    let id = db
        .create_automation(&NewAutomation {
            name: "nightly".to_string(),
            enabled: false,
            schedule: AutomationSchedule::Cron {
                expr: "0 3 * * *".to_string(),
            },
            timezone: None,
            action: AutomationAction::Exec {
                command: "true".to_string(),
            },
            prompt: String::new(),
            next_run_at: None,
        })
        .expect("automation");
    drop(db);

    write_plugin(
        &f.plugins,
        "scheduler",
        "\"automations-write\"",
        &format!("thurbox.setAutomationEnabled({id}, true)"),
    );
    let host = run_host(&f.plugins, &f.db_path);
    assert_eq!(host.problems(), &[] as &[String]);

    let enabled = f.db().get_automation(id).unwrap().expect("still there");
    assert!(enabled.enabled);
    assert!(
        enabled.next_run_at.is_some(),
        "enabling must schedule the next occurrence, as the native pane does"
    );

    // Disabling clears it again, and running marks it due for the kernel to fire.
    let f2 = Fixture::new();
    let db = f2.db();
    let id = db
        .create_automation(&NewAutomation {
            name: "hourly".to_string(),
            enabled: true,
            schedule: AutomationSchedule::Cron {
                expr: "0 * * * *".to_string(),
            },
            timezone: None,
            action: AutomationAction::Exec {
                command: "true".to_string(),
            },
            prompt: String::new(),
            next_run_at: Some(u64::MAX),
        })
        .expect("automation");
    drop(db);
    write_plugin(
        &f2.plugins,
        "runner",
        "\"automations-write\"",
        &format!("thurbox.runAutomation({id})"),
    );
    run_host(&f2.plugins, &f2.db_path);

    let due = f2.db().get_automation(id).unwrap().expect("still there");
    assert!(
        due.next_run_at.expect("due") < u64::MAX,
        "running marks it due; the kernel is what fires it"
    );
}

#[test]
fn a_granted_plugin_deletes_an_automation() {
    let f = Fixture::new();
    let db = f.db();
    let id = db
        .create_automation(&NewAutomation {
            name: "doomed".to_string(),
            enabled: true,
            schedule: AutomationSchedule::Once { at: u64::MAX },
            timezone: None,
            action: AutomationAction::Exec {
                command: "true".to_string(),
            },
            prompt: String::new(),
            next_run_at: Some(u64::MAX),
        })
        .expect("automation");
    drop(db);

    write_plugin(
        &f.plugins,
        "cleaner",
        "\"automations-write\"",
        &format!("thurbox.deleteAutomation({id})"),
    );
    run_host(&f.plugins, &f.db_path);

    assert!(f.db().get_automation(id).unwrap().is_none());
}

/// The capability list is per kind, so a plugin holding one write must not reach
/// the other's records — asserted against the database, not only against the
/// module table.
#[test]
fn a_task_writer_cannot_touch_an_automation() {
    let f = Fixture::new();
    let db = f.db();
    let id = db
        .create_automation(&NewAutomation {
            name: "safe".to_string(),
            enabled: true,
            schedule: AutomationSchedule::Once { at: u64::MAX },
            timezone: None,
            action: AutomationAction::Exec {
                command: "true".to_string(),
            },
            prompt: String::new(),
            next_run_at: Some(u64::MAX),
        })
        .expect("automation");
    drop(db);

    write_plugin(
        &f.plugins,
        "confused",
        "\"tasks-write\"",
        &format!("thurbox.deleteAutomation({id})"),
    );
    let host = run_host(&f.plugins, &f.db_path);

    assert!(host
        .status("confused")
        .expect("discovered")
        .state
        .is_failed());
    assert!(
        f.db().get_automation(id).unwrap().is_some(),
        "one write grant must not carry the other"
    );
}

/// A status the kernel does not define is refused, rather than defaulted: writing
/// `todo` when the plugin asked for something else would be a wrong write.
#[test]
fn an_unknown_task_status_is_refused_and_changes_nothing() {
    let f = Fixture::new();
    let db = f.db();
    let id = db.create_task(&NewTask::local("keep")).expect("task");
    drop(db);

    write_plugin(
        &f.plugins,
        "typo",
        "\"tasks-write\"",
        &format!("thurbox.setTaskStatus({id}, \"finished\")"),
    );
    let host = run_host(&f.plugins, &f.db_path);

    assert!(host.status("typo").expect("discovered").state.is_failed());
    assert_eq!(
        f.db().get_task(id).unwrap().expect("still there").status,
        TaskStatus::Todo
    );
}

/// A host built without a writer is read-only whatever a manifest asked for, which
/// is what every existing caller (and every test host) gets.
#[test]
fn a_host_with_no_writer_grants_a_binding_that_cannot_write() {
    let f = Fixture::new();
    let db = f.db();
    let id = db.create_task(&NewTask::local("keep")).expect("task");
    drop(db);

    write_plugin(
        &f.plugins,
        "hopeful",
        "\"tasks-write\"",
        &format!("thurbox.setTaskStatus({id}, \"done\")"),
    );
    let mut host = PluginHost::from_discovery(
        thurbox::plugin::discovery::discover_in(&[], Some(&f.plugins)),
        ExecutionBounds::default(),
    );
    host.start_all();

    assert!(host
        .status("hopeful")
        .expect("discovered")
        .state
        .is_failed());
    assert_eq!(
        f.db().get_task(id).unwrap().expect("still there").status,
        TaskStatus::Todo
    );
}
