//! `thurbox-cli notify` — diagnose OS desktop notifications.
//!
//! The default (no flags) prints which delivery backend thurbox would use on
//! this host, whether it can actually deliver, whether click-to-focus is
//! supported, and the last recorded delivery error (if any). This exists
//! because the WSL failure mode used to be entirely silent — the dbus path
//! errored and only logged a `warn!` to the file logger.
//!
//! With `--test`, it fires a sample notification *synchronously* so the user
//! can confirm end-to-end that a banner/toast actually appears.

use clap::Args;
use serde_json::json;

use crate::notifications::{self, DeliveryBackend, Notification};
use crate::session::settings;
use crate::session::SessionId;

use super::output::{kv, CommandOutput};

/// `notify` subcommand arguments.
#[derive(Args, Debug)]
pub struct NotifyArgs {
    /// Fire a sample notification to confirm delivery works end-to-end.
    #[arg(long)]
    pub test: bool,
}

/// Run the `notify` command. Takes no database — it reads the process-wide
/// settings (for the configured backend) and probes the host.
pub fn run(args: NotifyArgs) -> CommandOutput {
    let s = settings::global();
    let feature_on = s.features.notifications;
    let configured = s.notifications.backend;
    let backend = notifications::detect_backend(configured);

    if args.test {
        return run_test(feature_on, backend);
    }

    let last_error = notifications::last_error();
    let mut pairs = vec![
        ("feature", on_off(feature_on)),
        ("configured", format!("{configured:?}").to_lowercase()),
        ("backend", backend.label().to_string()),
        ("deliverable", yes_no(backend.is_deliverable())),
        ("click-to-focus", yes_no(backend.supports_click_to_focus())),
    ];
    // Only meaningful on the macOS backend; elsewhere the field is absent.
    if let Some(present) = terminal_notifier_present(backend) {
        pairs.push(("terminal-notifier", yes_no(present)));
    }
    if let Some(err) = &last_error {
        pairs.push(("last_error", err.clone()));
    }

    // Lead with the most actionable line, then the detail block.
    let headline = diagnose_headline(feature_on, backend);
    let human = format!("{headline}\n\n{}", kv(&pairs));

    CommandOutput::new(
        json!({
            "feature_enabled": feature_on,
            "configured_backend": format!("{configured:?}").to_lowercase(),
            "resolved_backend": backend.label(),
            "deliverable": backend.is_deliverable(),
            "click_to_focus": backend.supports_click_to_focus(),
            "terminal_notifier": terminal_notifier_present(backend),
            "last_error": last_error,
            "summary": headline,
        }),
        human,
    )
}

fn run_test(feature_on: bool, backend: DeliveryBackend) -> CommandOutput {
    if !feature_on {
        let msg = "Notifications are disabled ([features] notifications = false); \
                   nothing to test. Enable the feature first.";
        return CommandOutput::new(
            json!({
                "feature_enabled": false,
                "tested": false,
                "summary": msg,
            }),
            msg,
        );
    }

    let n = Notification {
        session_id: SessionId::default(),
        title: "thurbox · test".into(),
        body: "Notifications are working. This is a test from `thurbox-cli notify --test`.".into(),
        sound: settings::global().notifications.sound,
    };

    match notifications::send_blocking(&n, backend) {
        Ok(()) => {
            let mut msg = format!(
                "Sent a test notification via {}. If you don't see it, check your OS \
                 notification settings.",
                backend.label()
            );
            append_macos_hint(&mut msg, backend);
            CommandOutput::new(
                json!({
                    "feature_enabled": true,
                    "tested": true,
                    "delivered": true,
                    "resolved_backend": backend.label(),
                    "summary": msg,
                }),
                msg,
            )
        }
        Err(e) => {
            let msg = format!("Test notification failed via {}: {e}", backend.label());
            CommandOutput::failed(
                json!({
                    "feature_enabled": true,
                    "tested": true,
                    "delivered": false,
                    "resolved_backend": backend.label(),
                    "error": e,
                    "summary": msg,
                }),
                msg.clone(),
                msg,
            )
        }
    }
}

/// One actionable summary line tailored to the resolved state.
fn diagnose_headline(feature_on: bool, backend: DeliveryBackend) -> String {
    if !feature_on {
        return "Notifications are OFF ([features] notifications = false).".into();
    }
    if !backend.is_deliverable() {
        return format!(
            "Notifications are enabled but have no working backend ({}). On WSL, ensure \
             powershell.exe is on PATH; otherwise install a notification daemon or set \
             [notifications] backend.",
            backend.label()
        );
    }
    let mut headline = format!(
        "Notifications enabled — delivering via {}. Run `thurbox-cli notify --test` to confirm.",
        backend.label()
    );
    append_macos_hint(&mut headline, backend);
    headline
}

/// The one-line, actionable nudge shown on the macOS backend when
/// `terminal-notifier` is absent: describes what's degraded (Script Editor
/// attribution, dead clicks) and the one-command fix. Text sourced from the
/// `notifications` module doc.
const MACOS_TERMINAL_NOTIFIER_HINT: &str =
    "On macOS these banners are attributed to \"Script Editor\" and clicking them does nothing — \
     run `brew install terminal-notifier` to enable click-to-focus and native branding.";

/// Append [`MACOS_TERMINAL_NOTIFIER_HINT`] to `msg` (space-separated) when the
/// macOS backend is degraded — i.e. active but without `terminal-notifier`.
/// Shared by the default diagnostic and the `--test` success message so the
/// nudge reads identically from either entry point.
fn append_macos_hint(msg: &mut String, backend: DeliveryBackend) {
    if macos_hint_applies(backend) {
        msg.push(' ');
        msg.push_str(MACOS_TERMINAL_NOTIFIER_HINT);
    }
}

/// True when the macOS backend is active but `terminal-notifier` is missing, so
/// the degraded-notification hint applies. Keys off
/// [`DeliveryBackend::supports_click_to_focus`], which already encodes exactly
/// "macOS + terminal-notifier present" (and is `false` for `Macos` on non-macOS
/// builds), so the branch stays cross-platform-compilable.
fn macos_hint_applies(backend: DeliveryBackend) -> bool {
    matches!(backend, DeliveryBackend::Macos) && !backend.supports_click_to_focus()
}

/// Whether `terminal-notifier` is available, but only for the macOS backend —
/// `None` (field omitted) on every other backend, where the notion is
/// irrelevant.
fn terminal_notifier_present(backend: DeliveryBackend) -> Option<bool> {
    matches!(backend, DeliveryBackend::Macos).then(|| backend.supports_click_to_focus())
}

fn on_off(b: bool) -> String {
    if b {
        "on".into()
    } else {
        "off".into()
    }
}

fn yes_no(b: bool) -> String {
    if b {
        "yes".into()
    } else {
        "no".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_default_reports_backend_fields() {
        // In the test process settings::global() is the all-default config
        // (feature on, backend auto); host probing runs against the real test
        // host, so we only assert the shape, not the concrete backend.
        let out = run(NotifyArgs { test: false });
        assert!(out["feature_enabled"].is_boolean());
        assert!(out["resolved_backend"].is_string());
        assert!(out["deliverable"].is_boolean());
        assert!(out["summary"].is_string());
        assert!(out.failure.is_none(), "the report itself never fails");
    }

    #[test]
    fn headline_calls_out_disabled_feature() {
        let h = diagnose_headline(false, DeliveryBackend::Dbus);
        assert!(h.contains("OFF"), "got: {h}");
    }

    #[test]
    fn headline_calls_out_no_backend() {
        let h = diagnose_headline(true, DeliveryBackend::None);
        assert!(h.contains("no working backend"), "got: {h}");
        assert!(h.contains("WSL"), "should hint at the WSL case: {h}");
    }

    #[test]
    fn headline_confirms_a_working_backend() {
        let h = diagnose_headline(true, DeliveryBackend::Dbus);
        assert!(h.contains("delivering via"), "got: {h}");
    }

    #[test]
    fn hint_names_the_fix() {
        assert!(MACOS_TERMINAL_NOTIFIER_HINT.contains("brew install"));
        assert!(MACOS_TERMINAL_NOTIFIER_HINT.contains("terminal-notifier"));
        assert!(MACOS_TERMINAL_NOTIFIER_HINT.contains("Script Editor"));
    }

    #[test]
    fn headline_hints_terminal_notifier_on_macos_without_it() {
        let h = diagnose_headline(true, DeliveryBackend::Macos);
        // Deterministic on non-macOS builds (Macos → no click-to-focus, so the
        // hint fires); on macOS it adapts to whether the tool is installed.
        if DeliveryBackend::Macos.supports_click_to_focus() {
            assert!(h.contains("delivering via"), "got: {h}");
            assert!(!h.contains("brew install"), "no hint when present: {h}");
        } else {
            assert!(h.contains("delivering via"), "still confirms delivery: {h}");
            assert!(h.contains("terminal-notifier"), "got: {h}");
            assert!(h.contains("brew install"), "got: {h}");
        }
    }

    #[test]
    fn non_macos_headlines_omit_the_macos_hint() {
        for b in [
            DeliveryBackend::Dbus,
            DeliveryBackend::WindowsToast,
            DeliveryBackend::None,
        ] {
            let h = diagnose_headline(true, b);
            assert!(!h.contains("terminal-notifier"), "{b:?} leaked hint: {h}");
        }
    }

    #[test]
    fn terminal_notifier_field_only_for_macos() {
        assert!(terminal_notifier_present(DeliveryBackend::Dbus).is_none());
        assert!(terminal_notifier_present(DeliveryBackend::None).is_none());
        // Present (Some) on the macOS backend, value tracking availability.
        let macos = terminal_notifier_present(DeliveryBackend::Macos);
        assert_eq!(
            macos,
            Some(DeliveryBackend::Macos.supports_click_to_focus())
        );
    }

    #[test]
    fn append_macos_hint_appends_only_when_degraded() {
        // Non-macOS backends never get the hint; the base string is untouched.
        let mut s = "base".to_string();
        append_macos_hint(&mut s, DeliveryBackend::Dbus);
        assert_eq!(s, "base");

        // On the macOS backend the hint fires only without terminal-notifier,
        // and when it does it preserves the base plus a single-space separator.
        // This is the only direct coverage of the append shared by `run_test`.
        let mut s = "base".to_string();
        append_macos_hint(&mut s, DeliveryBackend::Macos);
        if DeliveryBackend::Macos.supports_click_to_focus() {
            assert_eq!(s, "base", "no hint when terminal-notifier is present");
        } else {
            assert_eq!(s, format!("base {MACOS_TERMINAL_NOTIFIER_HINT}"));
        }
    }
}
