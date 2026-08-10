//! The info panel — the first of thurbox's own panes to render through the
//! **view tree** rather than through hand-built terminal primitives.
//!
//! The pane is still ordinary in-process Rust called from `App::render_info_panel`
//! on the UI thread; no plugin, no VM. What changed is the
//! intermediate representation: [`info_tree`] returns a
//! [`crate::session::view_tree::ViewNode`] and
//! [`crate::ui::plugin_pane::render_tree`] paints it. That makes this pane the
//! evidence for the Phase 0 exit criterion — the catalogue can carry a real v1
//! pane — and it is why three catalogue gaps
//! (`docs/PHASE4-PANE-READINESS.md` §3 and §4, plus wrapping) were closed here.
//!
//! One property is worth stating because it is the point of the gauge node: the
//! tree [`info_tree`] returns carries **no geometry at all**. Every width-
//! dependent decision the old renderer made by hand — the flush-right gauge
//! suffix, the bar length, the separator's width, where a long value wraps — is
//! now the renderer's, resolved from the area it is given. That is what a plugin
//! would need, since a plugin never learns its pane's width.

use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{Block, Borders},
    Frame,
};
// Only the retained byte-identity oracle (`legacy_lines` and the section
// builders it calls) still assembles ratatui primitives by hand.
#[cfg(test)]
use ratatui::{
    style::Modifier,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use super::theme::Theme;
use crate::session::view_tree::{StyleToken, TextStyle, ViewNode};
use crate::session::{AgentMetrics, SessionInfo, SessionStatus};

/// View-only entry for upcoming automations shown in the info panel.
pub struct AutomationEntry {
    pub label: String,
    pub countdown: String,
}

/// System-wide and active-session resource metrics.
pub struct SystemMetrics {
    /// Overall CPU usage 0-100.
    pub cpu_percent: f32,
    /// Total RAM used in bytes.
    pub memory_used: u64,
    /// Total RAM in bytes.
    pub memory_total: u64,
    /// Active session CPU usage 0-100+.
    pub session_cpu_percent: f32,
    /// Active session memory in bytes.
    pub session_memory_bytes: u64,
}

#[allow(clippy::too_many_arguments)]
pub fn render_info_panel(
    frame: &mut Frame,
    area: Rect,
    info: &SessionInfo,
    metrics: Option<&SystemMetrics>,
    automations: &[AutomationEntry],
    usage: Option<&crate::session::AgentUsage>,
    parent_name: Option<&str>,
    thurbox_dir_bytes: Option<u64>,
) {
    let block = Block::default()
        .title(" Info ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Theme::border_unfocused()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let tree = info_tree(
        info,
        metrics,
        automations,
        usage,
        parent_name,
        thurbox_dir_bytes,
        epoch_now_secs(),
    );
    // A fresh frame table every paint: nothing in this pane animates, and a
    // motion the kernel is not driving draws frame 0 anyway.
    super::plugin_pane::render_tree(
        &tree,
        inner,
        &super::theme::current(),
        &crate::session::motion::FrameTable::default(),
        frame.buffer_mut(),
    );
}

/// Build the pane's view tree.
///
/// Separated from the paint so the tree is inspectable in a test without a
/// terminal, and because it takes no area: nothing here depends on how wide the
/// pane resolved to.
///
/// `now` is Unix epoch seconds, taken as an argument rather than read here so
/// the tree is a pure function of its inputs — the same reason it takes no area.
/// A plugin reproducing this pane has neither a width nor a clock, and a caller
/// comparing two builders needs them to agree on the instant.
#[allow(clippy::too_many_arguments)]
pub fn info_tree(
    info: &SessionInfo,
    metrics: Option<&SystemMetrics>,
    automations: &[AutomationEntry],
    usage: Option<&crate::session::AgentUsage>,
    parent_name: Option<&str>,
    thurbox_dir_bytes: Option<u64>,
    now: u64,
) -> ViewNode {
    let mut rows = Vec::new();

    push_session_rows(&mut rows, info, parent_name);
    push_repo_rows(&mut rows, info);

    if let Some(git) = &info.git_stats {
        push_git_rows(&mut rows, git);
    }
    if let Some(m) = metrics {
        push_session_resource_rows(&mut rows, m);
    }
    if let Some(m) = &info.agent_metrics {
        push_agent_rows(&mut rows, m);
    }
    if let Some(u) = usage {
        if !u.is_empty() {
            // Usage is per (agent, host); label the host so two sessions on
            // different accounts read unambiguously.
            push_usage_rows(&mut rows, u, info.remote_host.as_deref(), now);
        }
    }
    if let Some(m) = metrics {
        push_system_rows(&mut rows, m, thurbox_dir_bytes);
    }
    push_automation_rows(&mut rows, automations);

    ViewNode::list(rows)
}

// ── row constructors ─────────────────────────────────────────────────────────
//
// Every textual row is a `paragraph` rather than a `line`: this pane's values
// are agent-supplied (an OSC title, a notification body, a session name) and
// clipping them at the pane edge would hide exactly the text a user reads when
// a session wants attention.

/// A muted `label` followed by `value` in `token`.
fn field(label: &str, value: impl Into<String>, token: StyleToken) -> ViewNode {
    ViewNode::Paragraph(vec![
        ViewNode::token(label, StyleToken::Muted),
        ViewNode::token(value, token),
    ])
}

/// A muted `label` followed by `value` in the theme's primary foreground.
fn field_plain(label: &str, value: impl Into<String>) -> ViewNode {
    ViewNode::Paragraph(vec![
        ViewNode::token(label, StyleToken::Muted),
        ViewNode::text(value),
    ])
}

/// A bold accent section heading.
fn section(title: impl Into<String>) -> ViewNode {
    ViewNode::Paragraph(vec![ViewNode::styled(
        title,
        TextStyle {
            token: Some(StyleToken::Accent),
            bold: true,
            ..TextStyle::default()
        },
    )])
}

/// The session header rows: name, status, agent, and the optional parent / host
/// / hooks / activity / signal rows.
fn push_session_rows(rows: &mut Vec<ViewNode>, info: &SessionInfo, parent_name: Option<&str>) {
    rows.push(field_plain("Name: ", &info.name));
    rows.push(ViewNode::Paragraph(vec![
        ViewNode::token("Status: ", StyleToken::Muted),
        ViewNode::styled(
            format!("{} {}", info.status.icon(), info.status),
            TextStyle {
                token: Some(StyleToken::for_status(info.status)),
                bold: true,
                ..TextStyle::default()
            },
        ),
    ]));
    rows.push(ViewNode::Paragraph(vec![
        ViewNode::token("Agent: ", StyleToken::Muted),
        ViewNode::styled(
            &info.agent,
            TextStyle {
                token: Some(StyleToken::Role),
                bold: true,
                ..TextStyle::default()
            },
        ),
    ]));
    // Parent session (lead/worker linkage); omitted for top-level sessions.
    if let Some(parent) = parent_name {
        rows.push(field("Parent: ", parent, StyleToken::Secondary));
    }
    // Remote host (ssh:/wsl:); omitted entirely for local sessions.
    if let Some(host) = info.remote_host.as_deref() {
        rows.push(field(
            "Host:  ",
            format!("\u{21c5} {host}"),
            StyleToken::Accent,
        ));
    }
    // Hooks-driven status is degraded on this (remote) session — without this
    // hint the dot just silently stays Idle and looks like an agent at rest.
    if let Some(reason) = info.hook_wiring.as_deref() {
        rows.push(field(
            "Hooks: ",
            format!("degraded — {reason}"),
            StyleToken::Muted,
        ));
    }
    // Live activity from the agent-emitted OSC terminal title.
    if let Some(activity) = info.agent_activity.as_deref() {
        rows.push(field("Activity: ", activity, StyleToken::Secondary));
    }
    // Latest signal the agent pushed (OSC 9/777 notification). Highlighted while
    // the session is blocked, muted otherwise.
    if let Some(signal) = info.notification.as_deref() {
        let token = if info.status == SessionStatus::Blocked {
            StyleToken::for_status(info.status)
        } else {
            StyleToken::Muted
        };
        rows.push(field("Signal:   ", signal, token));
    }
}

/// The repo rows: the primary `repo/branch`, then one continuation row per
/// additional directory, indented under it.
fn push_repo_rows(rows: &mut Vec<ViewNode>, info: &SessionInfo) {
    let first_repo = info
        .worktrees
        .first()
        .map(|wt| &wt.repo_path)
        .or(info.cwd.as_ref())
        .and_then(|p| p.file_name())
        .and_then(|f| f.to_str());
    let branch = info.worktrees.first().map(|wt| wt.branch.as_str());

    let primary = match (first_repo, branch) {
        (Some(repo), Some(br)) => format!("{repo}/{br}"),
        (Some(repo), None) => repo.to_string(),
        (None, Some(br)) => br.to_string(),
        // Nothing to show if there is no repo info at all.
        (None, None) => return,
    };
    rows.push(field("Repos: ", primary, StyleToken::Branch));

    for name in info
        .additional_dirs
        .iter()
        .filter_map(|d| d.file_name().and_then(|f| f.to_str()))
    {
        rows.push(field("       ", name, StyleToken::Branch));
    }
}

/// The git change-summary rows (uncommitted diff + sync state).
fn push_git_rows(rows: &mut Vec<ViewNode>, git: &crate::session::GitStats) {
    // Changes row: only when there is something uncommitted.
    if git.files_changed > 0 || git.dirty {
        let files = if git.files_changed == 1 {
            "1 file".to_string()
        } else {
            format!("{} files", git.files_changed)
        };
        let mut runs = vec![
            ViewNode::token("Changes: ", StyleToken::Muted),
            ViewNode::text(files),
            ViewNode::text(" "),
            ViewNode::token(format!("+{}", git.insertions), StyleToken::Added),
            ViewNode::token(" / ", StyleToken::Muted),
            ViewNode::token(format!("-{}", git.deletions), StyleToken::StatusError),
        ];
        if git.dirty && git.files_changed == 0 {
            runs.push(ViewNode::token(" dirty", StyleToken::Muted));
        }
        rows.push(ViewNode::Paragraph(runs));
    }

    // Sync row: only when ahead of / behind the base branch.
    if git.ahead > 0 || git.behind > 0 {
        rows.push(field(
            "Sync:    ",
            format!("↑{} ↓{}", git.ahead, git.behind),
            StyleToken::Muted,
        ));
    }
}

/// The active session's CPU gauge + RAM row (only when either is non-zero).
fn push_session_resource_rows(rows: &mut Vec<ViewNode>, m: &SystemMetrics) {
    if m.session_cpu_percent > 0.0 || m.session_memory_bytes > 0 {
        rows.push(ViewNode::gauge("CPU", m.session_cpu_percent, None));
        rows.push(ViewNode::Paragraph(vec![
            ViewNode::token("RAM", StyleToken::Muted),
            ViewNode::text(format!("  {}", format_bytes(m.session_memory_bytes))),
        ]));
    }
}

/// The Agent section: model/version heading then the agent's reported metrics.
fn push_agent_rows(rows: &mut Vec<ViewNode>, metrics: &AgentMetrics) {
    rows.push(ViewNode::Divider);
    rows.push(section(agent_section_header(metrics)));

    // Cost (headline number, only when the agent reports a non-zero spend).
    if let Some(cost) = metrics.total_cost_usd.filter(|c| *c > 0.0) {
        rows.push(field("Cost:    ", format_cost(cost), StyleToken::Accent));
    }

    if let Some(ms) = metrics.total_duration_ms {
        let mut runs = vec![
            ViewNode::token("Time:    ", StyleToken::Muted),
            ViewNode::text(format_duration(ms)),
        ];
        if let Some(api_ms) = metrics.total_api_duration_ms.filter(|&v| v > 0) {
            runs.push(ViewNode::token(
                format!("  (api {})", format_duration(api_ms)),
                StyleToken::Muted,
            ));
        }
        rows.push(ViewNode::Paragraph(runs));
    }

    if metrics.total_input_tokens.is_some() || metrics.total_output_tokens.is_some() {
        let render = |v: Option<u64>| v.map(format_tokens).unwrap_or_else(|| "-".to_string());
        rows.push(field_plain(
            "Tokens:  ",
            format!(
                "{} in / {} out",
                render(metrics.total_input_tokens),
                render(metrics.total_output_tokens)
            ),
        ));
    }

    // Context-window gauge: as used/total tokens when the window size is known,
    // else a bare percentage.
    if let Some(pct) = metrics.used_percentage {
        let suffix = metrics.context_window_size.map(|size| {
            let used = (size as f64 * pct as f64 / 100.0).round() as u64;
            format!("{}/{}", format_tokens(used), format_tokens(size))
        });
        rows.push(ViewNode::gauge("Context", pct as f32, suffix));
    }

    if metrics.total_lines_added.is_some() || metrics.total_lines_removed.is_some() {
        rows.push(ViewNode::Paragraph(vec![
            ViewNode::token("Lines:   ", StyleToken::Muted),
            ViewNode::token(
                format!("+{}", metrics.total_lines_added.unwrap_or(0)),
                StyleToken::Added,
            ),
            ViewNode::token(" / ", StyleToken::Muted),
            ViewNode::token(
                format!("-{}", metrics.total_lines_removed.unwrap_or(0)),
                StyleToken::StatusError,
            ),
        ]));
    }

    let cache_read = metrics.cache_read_input_tokens.unwrap_or(0);
    let cache_create = metrics.cache_creation_input_tokens.unwrap_or(0);
    if cache_read > 0 || cache_create > 0 {
        rows.push(field_plain(
            "Cache:   ",
            format!(
                "{} read / {} created",
                format_tokens(cache_read),
                format_tokens(cache_create)
            ),
        ));
    }
}

/// The account-level usage / rate-limit section (the `/usage` equivalent): one
/// gauge per window with used-% and a reset countdown. `host` names the
/// credential source for a remote session (`None` = local), so usage from
/// different accounts is never mistaken for the local one.
fn push_usage_rows(
    rows: &mut Vec<ViewNode>,
    usage: &crate::session::AgentUsage,
    host: Option<&str>,
    now: u64,
) {
    rows.push(ViewNode::Divider);
    rows.push(section(usage_section_header(usage, host)));

    if usage.windows.is_empty() {
        if let Some(note) = &usage.note {
            // A paragraph, not a bare text node: the note is vendor-supplied
            // ("not logged in", an API error) and a `text` node would clip it.
            rows.push(ViewNode::Paragraph(vec![ViewNode::token(
                note.clone(),
                StyleToken::Muted,
            )]));
        }
        return;
    }

    for w in &usage.windows {
        let suffix = match w.resets_at {
            Some(reset) => format!(
                "{:.0}%  {}",
                w.used_percent,
                format_countdown_secs(reset.saturating_sub(now))
            ),
            None => format!("{:.0}%", w.used_percent),
        };
        rows.push(ViewNode::gauge(&w.label, w.used_percent, Some(suffix)));
    }
}

/// The System section (global CPU/RAM gauges), plus thurbox's own on-disk
/// footprint (`~/.local/share/thurbox`) when it has been measured — a heads-up
/// when accumulated worktrees/workspaces have grown the data dir.
fn push_system_rows(rows: &mut Vec<ViewNode>, m: &SystemMetrics, thurbox_dir_bytes: Option<u64>) {
    rows.push(ViewNode::Divider);
    rows.push(section("System"));
    rows.push(ViewNode::gauge("CPU", m.cpu_percent, None));

    let ram_percent = if m.memory_total > 0 {
        (m.memory_used as f64 / m.memory_total as f64 * 100.0) as f32
    } else {
        0.0
    };
    rows.push(ViewNode::gauge(
        "RAM",
        ram_percent,
        Some(format_bytes_pair(m.memory_used, m.memory_total)),
    ));

    if let Some(bytes) = thurbox_dir_bytes {
        rows.push(ViewNode::Paragraph(vec![
            ViewNode::token("Disk", StyleToken::Muted),
            ViewNode::text(format!("  {} (thurbox dir)", format_bytes(bytes))),
        ]));
    }
}

/// The upcoming-automations section (skipped when there are none).
fn push_automation_rows(rows: &mut Vec<ViewNode>, automations: &[AutomationEntry]) {
    if automations.is_empty() {
        return;
    }
    rows.push(ViewNode::Divider);
    rows.push(section(format!("Automations ({})", automations.len())));
    for entry in automations {
        rows.push(ViewNode::Paragraph(vec![
            ViewNode::token(&entry.countdown, StyleToken::Accent),
            ViewNode::text("  "),
            ViewNode::token(&entry.label, StyleToken::Secondary),
        ]));
    }
}

/// The Usage heading, which names the plan and — for a remote session — the host
/// whose credentials the numbers came from.
fn usage_section_header(usage: &crate::session::AgentUsage, host: Option<&str>) -> String {
    match (&usage.plan, host) {
        (Some(plan), Some(host)) => format!("Usage ({plan} · {host})"),
        (Some(plan), None) => format!("Usage ({plan})"),
        (None, Some(host)) => format!("Usage ({host})"),
        (None, None) => "Usage".to_string(),
    }
}

/// Append the session header rows: name, status, agent, and the optional
/// parent / host / activity / signal rows.
///
/// Retained as the **byte-identity oracle** for the view-tree port: the pane it
/// used to draw is what `view_tree_render_matches_the_legacy_paragraph` compares
/// the tree's rendering against, cell for cell. Not compiled into the binary.
#[cfg(test)]
fn append_session_section<'a>(
    lines: &mut Vec<Line<'a>>,
    info: &'a SessionInfo,
    parent_name: Option<&str>,
) {
    lines.push(Line::from(vec![
        Span::styled("Name: ", Theme::label()),
        Span::styled(&info.name, Style::default().fg(Theme::text_primary())),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Status: ", Theme::label()),
        Span::styled(
            format!("{} {}", info.status.icon(), info.status),
            Style::default()
                .fg(super::status_color(info.status))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Agent: ", Theme::label()),
        Span::styled(
            &info.agent,
            Style::default()
                .fg(Theme::role_name())
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    // Parent session (lead/worker linkage); omitted for top-level sessions.
    if let Some(parent) = parent_name {
        lines.push(Line::from(vec![
            Span::styled("Parent: ", Theme::label()),
            Span::styled(
                parent.to_string(),
                Style::default().fg(Theme::text_secondary()),
            ),
        ]));
    }
    // Remote host (ssh:/wsl:); omitted entirely for local sessions.
    if let Some(host) = info.remote_host.as_deref() {
        lines.push(Line::from(vec![
            Span::styled("Host:  ", Theme::label()),
            Span::styled(
                format!("\u{21c5} {host}"),
                Style::default().fg(Theme::accent()),
            ),
        ]));
    }
    // Hooks-driven status is degraded on this (remote) session — without this
    // hint the dot just silently stays Idle and looks like an agent at rest.
    if let Some(reason) = info.hook_wiring.as_deref() {
        lines.push(Line::from(vec![
            Span::styled("Hooks: ", Theme::label()),
            Span::styled(
                format!("degraded — {reason}"),
                Style::default().fg(Theme::text_muted()),
            ),
        ]));
    }
    // Live activity from the agent-emitted OSC terminal title.
    if let Some(activity) = info.agent_activity.as_deref() {
        lines.push(Line::from(vec![
            Span::styled("Activity: ", Theme::label()),
            Span::styled(activity, Style::default().fg(Theme::text_secondary())),
        ]));
    }
    // Latest signal the agent pushed (OSC 9/777 notification). Highlighted while
    // the session is blocked, muted otherwise.
    if let Some(signal) = info.notification.as_deref() {
        let value_style = if info.status == crate::session::SessionStatus::Blocked {
            Style::default().fg(super::status_color(info.status))
        } else {
            Style::default().fg(Theme::text_muted())
        };
        lines.push(Line::from(vec![
            Span::styled("Signal:   ", Theme::label()),
            Span::styled(signal, value_style),
        ]));
    }
}

/// Append the active session's CPU gauge + RAM line (only when non-zero).
#[cfg(test)]
fn append_session_resources(lines: &mut Vec<Line<'_>>, m: &SystemMetrics, inner_width: usize) {
    if m.session_cpu_percent > 0.0 || m.session_memory_bytes > 0 {
        let cpu_gauge = render_gauge_lines("CPU", m.session_cpu_percent, None, inner_width);
        lines.extend(cpu_gauge);

        lines.push(Line::from(vec![
            Span::styled("RAM", Style::default().fg(Theme::text_muted())),
            Span::styled(
                format!("  {}", format_bytes(m.session_memory_bytes)),
                Style::default().fg(Theme::text_primary()),
            ),
        ]));
    }
}

/// Append the System Resources section (global CPU/RAM gauges), plus thurbox's
/// own on-disk footprint (`~/.local/share/thurbox`) when it has been measured —
/// a heads-up when accumulated worktrees/workspaces have grown the data dir.
#[cfg(test)]
fn append_system_section(
    lines: &mut Vec<Line<'_>>,
    m: &SystemMetrics,
    thurbox_dir_bytes: Option<u64>,
    inner_width: usize,
) {
    lines.push(separator(inner_width));
    lines.push(Line::from(Span::styled("System", Theme::section_header())));

    let gauge_lines = render_gauge_lines("CPU", m.cpu_percent, None, inner_width);
    lines.extend(gauge_lines);

    let ram_lines = render_gauge_lines(
        "RAM",
        if m.memory_total > 0 {
            (m.memory_used as f64 / m.memory_total as f64 * 100.0) as f32
        } else {
            0.0
        },
        Some(format_bytes_pair(m.memory_used, m.memory_total)),
        inner_width,
    );
    lines.extend(ram_lines);

    if let Some(bytes) = thurbox_dir_bytes {
        lines.push(Line::from(vec![
            Span::styled("Disk", Style::default().fg(Theme::text_muted())),
            Span::styled(
                format!("  {} (thurbox dir)", format_bytes(bytes)),
                Style::default().fg(Theme::text_primary()),
            ),
        ]));
    }
}

/// Append the upcoming-automations section (skipped when there are none).
#[cfg(test)]
fn append_automations_section<'a>(
    lines: &mut Vec<Line<'a>>,
    automations: &'a [AutomationEntry],
    inner_width: usize,
) {
    if automations.is_empty() {
        return;
    }
    lines.push(separator(inner_width));
    lines.push(Line::from(Span::styled(
        format!("Automations ({})", automations.len()),
        Theme::section_header(),
    )));
    for entry in automations {
        lines.push(Line::from(vec![
            Span::styled(&entry.countdown, Style::default().fg(Theme::accent())),
            Span::styled("  ", Style::default()),
            Span::styled(&entry.label, Style::default().fg(Theme::text_secondary())),
        ]));
    }
}

/// Build the Agent section header from the model name + CLI version.
fn agent_section_header(metrics: &AgentMetrics) -> String {
    match (&metrics.model_display_name, &metrics.cli_version) {
        (Some(model), Some(ver)) => format!("Agent ({model} v{ver})"),
        (Some(model), None) => format!("Agent ({model})"),
        (None, Some(ver)) => format!("Agent (v{ver})"),
        (None, None) => "Agent".to_string(),
    }
}

/// Append the elapsed-time row, with API time as a muted aside when present.
#[cfg(test)]
fn append_agent_time(lines: &mut Vec<Line<'_>>, metrics: &AgentMetrics) {
    let Some(ms) = metrics.total_duration_ms else {
        return;
    };
    let mut spans = vec![
        Span::styled("Time:    ", Theme::label()),
        Span::styled(
            format_duration(ms),
            Style::default().fg(Theme::text_primary()),
        ),
    ];
    if let Some(api_ms) = metrics.total_api_duration_ms.filter(|&v| v > 0) {
        spans.push(Span::styled(
            format!("  (api {})", format_duration(api_ms)),
            Style::default().fg(Theme::text_muted()),
        ));
    }
    lines.push(Line::from(spans));
}

/// Append the tokens row (input / output), skipped when neither is reported.
#[cfg(test)]
fn append_agent_tokens(lines: &mut Vec<Line<'_>>, metrics: &AgentMetrics) {
    if metrics.total_input_tokens.is_none() && metrics.total_output_tokens.is_none() {
        return;
    }
    let input = metrics
        .total_input_tokens
        .map(format_tokens)
        .unwrap_or_else(|| "-".to_string());
    let output = metrics
        .total_output_tokens
        .map(format_tokens)
        .unwrap_or_else(|| "-".to_string());
    lines.push(Line::from(vec![
        Span::styled("Tokens:  ", Theme::label()),
        Span::styled(
            format!("{input} in / {output} out"),
            Style::default().fg(Theme::text_primary()),
        ),
    ]));
}

/// Append the context-window gauge. When the window size is known, show the
/// gauge as used/total tokens rather than a bare percentage.
#[cfg(test)]
fn append_agent_context(lines: &mut Vec<Line<'_>>, metrics: &AgentMetrics, inner_width: usize) {
    if let Some(pct) = metrics.used_percentage {
        let suffix = metrics.context_window_size.map(|size| {
            let used = (size as f64 * pct as f64 / 100.0).round() as u64;
            format!("{}/{}", format_tokens(used), format_tokens(size))
        });
        let gauge = render_gauge_lines("Context", pct as f32, suffix, inner_width);
        lines.extend(gauge);
    }
}

/// Append the lines-changed row, skipped when neither is reported.
#[cfg(test)]
fn append_agent_lines_changed(lines: &mut Vec<Line<'_>>, metrics: &AgentMetrics) {
    if metrics.total_lines_added.is_none() && metrics.total_lines_removed.is_none() {
        return;
    }
    let added = metrics.total_lines_added.unwrap_or(0);
    let removed = metrics.total_lines_removed.unwrap_or(0);
    lines.push(Line::from(vec![
        Span::styled("Lines:   ", Theme::label()),
        Span::styled(
            format!("+{added}"),
            Style::default().fg(Theme::tool_allowed()),
        ),
        Span::styled(" / ", Style::default().fg(Theme::text_muted())),
        Span::styled(
            format!("-{removed}"),
            Style::default().fg(Theme::status_error()),
        ),
    ]));
}

/// Append the cache-stats row (only when non-zero).
#[cfg(test)]
fn append_agent_cache(lines: &mut Vec<Line<'_>>, metrics: &AgentMetrics) {
    let cache_read = metrics.cache_read_input_tokens.unwrap_or(0);
    let cache_create = metrics.cache_creation_input_tokens.unwrap_or(0);
    if cache_read > 0 || cache_create > 0 {
        lines.push(Line::from(vec![
            Span::styled("Cache:   ", Theme::label()),
            Span::styled(
                format!(
                    "{} read / {} created",
                    format_tokens(cache_read),
                    format_tokens(cache_create)
                ),
                Style::default().fg(Theme::text_primary()),
            ),
        ]));
    }
}

/// Append the Agent section showing Claude CLI metrics.
#[cfg(test)]
fn append_agent_section(lines: &mut Vec<Line<'_>>, metrics: &AgentMetrics, inner_width: usize) {
    lines.push(separator(inner_width));
    lines.push(Line::from(Span::styled(
        agent_section_header(metrics),
        Theme::section_header(),
    )));

    // Cost (headline number, only when the agent reports a non-zero spend).
    if let Some(cost) = metrics.total_cost_usd {
        if cost > 0.0 {
            lines.push(Line::from(vec![
                Span::styled("Cost:    ", Theme::label()),
                Span::styled(format_cost(cost), Style::default().fg(Theme::accent())),
            ]));
        }
    }

    append_agent_time(lines, metrics);
    append_agent_tokens(lines, metrics);
    append_agent_context(lines, metrics, inner_width);
    append_agent_lines_changed(lines, metrics);
    append_agent_cache(lines, metrics);
}

/// Append the repos section showing all repo paths for a session.
#[cfg(test)]
fn append_repos_section<'a>(lines: &mut Vec<Line<'a>>, info: &'a SessionInfo) {
    // Collect repo names: first worktree repo, then additional_dirs.
    let first_repo = info
        .worktrees
        .first()
        .map(|wt| &wt.repo_path)
        .or(info.cwd.as_ref())
        .and_then(|p| p.file_name())
        .and_then(|f| f.to_str());

    let branch = info.worktrees.first().map(|wt| wt.branch.as_str());

    // Nothing to show if there's no repo info at all.
    if first_repo.is_none() && branch.is_none() {
        return;
    }

    let primary = match (first_repo, branch) {
        (Some(repo), Some(br)) => format!("{repo}/{br}"),
        (Some(repo), None) => repo.to_string(),
        (None, Some(br)) => br.to_string(),
        (None, None) => return,
    };

    let extra_names: Vec<&str> = info
        .additional_dirs
        .iter()
        .filter_map(|d| d.file_name().and_then(|f| f.to_str()))
        .collect();

    if extra_names.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Repos: ", Theme::label()),
            Span::styled(primary, Style::default().fg(Theme::branch_name())),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Repos: ", Theme::label()),
            Span::styled(primary, Style::default().fg(Theme::branch_name())),
        ]));
        for name in &extra_names {
            lines.push(Line::from(vec![
                Span::styled("       ", Theme::label()),
                Span::styled(*name, Style::default().fg(Theme::branch_name())),
            ]));
        }
    }
}

/// Append the git change-summary lines (uncommitted diff + sync state).
#[cfg(test)]
fn append_git_section(lines: &mut Vec<Line<'_>>, git: &crate::session::GitStats) {
    // Changes line: only when there is something uncommitted.
    if git.files_changed > 0 || git.dirty {
        let files = if git.files_changed == 1 {
            "1 file".to_string()
        } else {
            format!("{} files", git.files_changed)
        };
        let mut spans = vec![
            Span::styled("Changes: ", Theme::label()),
            Span::styled(files, Style::default().fg(Theme::text_primary())),
            Span::raw(" "),
            Span::styled(
                format!("+{}", git.insertions),
                Style::default().fg(Theme::tool_allowed()),
            ),
            Span::styled(" / ", Style::default().fg(Theme::text_muted())),
            Span::styled(
                format!("-{}", git.deletions),
                Style::default().fg(Theme::status_error()),
            ),
        ];
        if git.dirty && git.files_changed == 0 {
            spans.push(Span::styled(
                " dirty",
                Style::default().fg(Theme::text_muted()),
            ));
        }
        lines.push(Line::from(spans));
    }

    // Sync line: only when ahead of / behind the base branch.
    if git.ahead > 0 || git.behind > 0 {
        lines.push(Line::from(vec![
            Span::styled("Sync:    ", Theme::label()),
            Span::styled(
                format!("↑{} ↓{}", git.ahead, git.behind),
                Style::default().fg(Theme::text_muted()),
            ),
        ]));
    }
}

/// Current Unix time in seconds (for usage reset countdowns).
fn epoch_now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Format a seconds countdown compactly: `"now"`, `"5m"`, `"1h 12m"`, `"4d 3h"`.
fn format_countdown_secs(secs: u64) -> String {
    if secs == 0 {
        return "now".to_string();
    }
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let mins = (secs % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else if mins > 0 {
        format!("{mins}m")
    } else {
        "<1m".to_string()
    }
}

/// Append the account-level usage / rate-limit section (the `/usage`
/// equivalent): one gauge per window with used-% and a reset countdown.
/// `host` names the credential source for a remote session (`None` = local),
/// so usage from different accounts is never mistaken for the local one.
#[cfg(test)]
fn append_usage_section(
    lines: &mut Vec<Line<'_>>,
    usage: &crate::session::AgentUsage,
    host: Option<&str>,
    width: usize,
) {
    lines.push(separator(width));
    let header = match (&usage.plan, host) {
        (Some(plan), Some(host)) => format!("Usage ({plan} · {host})"),
        (Some(plan), None) => format!("Usage ({plan})"),
        (None, Some(host)) => format!("Usage ({host})"),
        (None, None) => "Usage".to_string(),
    };
    lines.push(Line::from(Span::styled(header, Theme::section_header())));

    if usage.windows.is_empty() {
        if let Some(note) = &usage.note {
            lines.push(Line::from(Span::styled(
                note.clone(),
                Style::default().fg(Theme::text_muted()),
            )));
        }
        return;
    }

    let now = epoch_now_secs();
    for w in &usage.windows {
        let suffix = match w.resets_at {
            Some(reset) => format!(
                "{:.0}%  {}",
                w.used_percent,
                format_countdown_secs(reset.saturating_sub(now))
            ),
            None => format!("{:.0}%", w.used_percent),
        };
        let gauge = render_gauge_lines(&w.label, w.used_percent, Some(suffix), width);
        lines.extend(gauge);
    }
}

#[cfg(test)]
fn separator(width: usize) -> Line<'static> {
    Line::from(Span::styled(
        "─".repeat(width),
        Style::default().fg(Theme::border_unfocused()),
    ))
}

/// Render a gauge as two lines:
/// Line 1: label left-aligned, suffix/percent right-aligned
/// Line 2: full-width bar `[███░░░░]` scaled to `width - 2`
#[cfg(test)]
fn render_gauge_lines(
    label: &str,
    percent: f32,
    suffix: Option<String>,
    width: usize,
) -> Vec<Line<'static>> {
    let clamped = percent.clamp(0.0, 100.0);
    let right_text = suffix.unwrap_or_else(|| format!("{clamped:.0}%"));

    // Line 1: label + right-aligned suffix
    let label_len = label.chars().count();
    let right_len = right_text.chars().count();
    let padding = width.saturating_sub(label_len + right_len);
    let header_line = Line::from(vec![
        Span::styled(label.to_string(), Style::default().fg(Theme::text_muted())),
        Span::raw(" ".repeat(padding)),
        Span::styled(right_text, Style::default().fg(Theme::text_primary())),
    ]);

    // Line 2: bar scaled to width - 2 (for [ and ])
    let bar_width = width.saturating_sub(2);
    let filled = ((clamped / 100.0) * bar_width as f32).round() as usize;
    let empty = bar_width.saturating_sub(filled);
    let bar_line = Line::from(vec![
        Span::styled("[", Style::default().fg(Theme::text_muted())),
        Span::styled("█".repeat(filled), Style::default().fg(Theme::accent())),
        Span::styled("░".repeat(empty), Style::default().fg(Theme::text_muted())),
        Span::styled("]", Style::default().fg(Theme::text_muted())),
    ]);

    vec![header_line, bar_line]
}

/// Format bytes as a human-readable string like "1.2 GB".
fn format_bytes(bytes: u64) -> String {
    let (val, _, unit) = human_bytes(bytes);
    format!("{val:.1} {unit}")
}

/// Format a used/total byte pair like "8.2/16.0 GB".
fn format_bytes_pair(used: u64, total: u64) -> String {
    let (total_val, divisor, unit) = human_bytes(total);
    let used_val = used as f64 / divisor;
    format!("{used_val:.1}/{total_val:.1} {unit}")
}

/// Convert bytes to a human-readable (value, divisor, unit) tuple.
fn human_bytes(bytes: u64) -> (f64, f64, &'static str) {
    const GB: f64 = 1_073_741_824.0;
    const MB: f64 = 1_048_576.0;
    const KB: f64 = 1_024.0;

    let b = bytes as f64;
    if b >= GB {
        (b / GB, GB, "GB")
    } else if b >= MB {
        (b / MB, MB, "MB")
    } else {
        (b / KB, KB, "KB")
    }
}

/// Format a USD cost like `$0.0423` (4 dp below $1, 2 dp at/above $1).
fn format_cost(usd: f64) -> String {
    if usd >= 1.0 {
        format!("${usd:.2}")
    } else {
        format!("${usd:.4}")
    }
}

/// Format a millisecond duration as a compact human string:
/// `"820ms"`, `"45s"`, `"1m 23s"`, `"2h 05m"`.
fn format_duration(ms: u64) -> String {
    if ms < 1_000 {
        return format!("{ms}ms");
    }
    let total_secs = ms / 1_000;
    let secs = total_secs % 60;
    let mins = (total_secs / 60) % 60;
    let hours = total_secs / 3_600;
    if hours > 0 {
        format!("{hours}h {mins:02}m")
    } else if mins > 0 {
        format!("{mins}m {secs:02}s")
    } else {
        format!("{secs}s")
    }
}

/// Format a token count with human-readable suffixes (e.g., "15.2k", "1.2M").
fn format_tokens(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}k", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

/// The pane exactly as the pre-view-tree renderer assembled it, kept as the
/// oracle for byte-identity.
///
/// This is the body of the old `render_info_panel` minus the `Block`: one
/// `Vec<Line>` handed to a single wrapping `Paragraph`. Nothing calls it in a
/// release build — it exists so the port can be *proved* equal rather than
/// eyeballed, and so the section-level tests below keep pinning the behaviour the
/// port has to reproduce.
#[cfg(test)]
fn legacy_lines<'a>(
    info: &'a SessionInfo,
    metrics: Option<&SystemMetrics>,
    automations: &'a [AutomationEntry],
    usage: Option<&crate::session::AgentUsage>,
    parent_name: Option<&str>,
    thurbox_dir_bytes: Option<u64>,
    inner_width: usize,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    append_session_section(&mut lines, info, parent_name);
    append_repos_section(&mut lines, info);

    if let Some(ref git) = info.git_stats {
        append_git_section(&mut lines, git);
    }
    if let Some(m) = metrics {
        append_session_resources(&mut lines, m, inner_width);
    }
    if let Some(ref metrics) = info.agent_metrics {
        append_agent_section(&mut lines, metrics, inner_width);
    }
    if let Some(u) = usage {
        if !u.is_empty() {
            append_usage_section(&mut lines, u, info.remote_host.as_deref(), inner_width);
        }
    }
    if let Some(m) = metrics {
        append_system_section(&mut lines, m, thurbox_dir_bytes, inner_width);
    }
    append_automations_section(&mut lines, automations, inner_width);

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    // ── whole-pane frame ──
    //
    // The pane's *rendered* contract, pinned rather than asserted section by
    // section: every optional row is populated at once, so one frame covers
    // each row shape the panel can draw — labelled rows, a wrapping value, the
    // repo continuation, both git rows, five gauges, the disk row, and the
    // automations list. This is the snapshot the Phase 0 exit criterion means
    // when it says the port must be byte-identical.

    /// A session with every optional row populated.
    fn fixture_info() -> SessionInfo {
        let mut info = SessionInfo::new("fix-osc52-tmux".into());
        info.status = crate::session::SessionStatus::Blocked;
        info.agent = "claude".into();
        info.remote_host = Some("devbox".into());
        info.hook_wiring = Some("stripped for psmux host".into());
        // Long enough to wrap at this width, which is the only way the pinned
        // frame can prove wrapping survived the port.
        info.agent_activity =
            Some("Editing src/agent/control_mode.rs to fix the subscription".into());
        info.notification = Some("needs approval".into());
        info.worktrees = vec![crate::session::WorktreeInfo {
            repo_path: std::path::PathBuf::from("/src/thurbox"),
            worktree_path: std::path::PathBuf::from("/wt/thurbox"),
            branch: "fix/osc52".into(),
        }];
        info.additional_dirs = vec![
            std::path::PathBuf::from("/src/docs-site"),
            std::path::PathBuf::from("/src/reference"),
        ];
        info.git_stats = Some(crate::session::GitStats {
            files_changed: 3,
            insertions: 120,
            deletions: 8,
            untracked: 1,
            dirty: true,
            ahead: 2,
            behind: 1,
        });
        info.agent_metrics = Some(AgentMetrics {
            model_display_name: Some("Opus".into()),
            cli_version: Some("1.2.3".into()),
            total_cost_usd: Some(0.0423),
            total_duration_ms: Some(83_000),
            total_api_duration_ms: Some(45_000),
            total_lines_added: Some(120),
            total_lines_removed: Some(8),
            total_input_tokens: Some(15_200),
            total_output_tokens: Some(2_400),
            context_window_size: Some(200_000),
            used_percentage: Some(42),
            cache_creation_input_tokens: Some(1_024),
            cache_read_input_tokens: Some(9_216),
            ..Default::default()
        });
        info
    }

    fn fixture_metrics() -> SystemMetrics {
        SystemMetrics {
            cpu_percent: 37.0,
            memory_used: 8_589_934_592,
            memory_total: 17_179_869_184,
            session_cpu_percent: 12.5,
            session_memory_bytes: 268_435_456,
        }
    }

    /// Windows carry no `resets_at`: a countdown reads the wall clock, and a
    /// pinned frame cannot contain one.
    fn fixture_usage() -> crate::session::AgentUsage {
        crate::session::AgentUsage {
            windows: vec![
                crate::session::UsageWindow {
                    label: "5h".into(),
                    used_percent: 42.0,
                    resets_at: None,
                },
                crate::session::UsageWindow {
                    label: "Week".into(),
                    used_percent: 18.0,
                    resets_at: None,
                },
            ],
            plan: Some("max".into()),
            note: None,
        }
    }

    fn fixture_automations() -> Vec<AutomationEntry> {
        vec![
            AutomationEntry {
                label: "flow-tick".into(),
                countdown: "5m".into(),
            },
            AutomationEntry {
                label: "renovate-sweep".into(),
                countdown: "4d 3h".into(),
            },
        ]
    }

    /// Render the pane at `w`×`h` and return the frame as text, one line per
    /// row with trailing blanks trimmed.
    fn draw_panel(w: u16, h: u16) -> String {
        let info = fixture_info();
        let metrics = fixture_metrics();
        let usage = fixture_usage();
        let automations = fixture_automations();
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|frame| {
                render_info_panel(
                    frame,
                    Rect {
                        x: 0,
                        y: 0,
                        width: w,
                        height: h,
                    },
                    &info,
                    Some(&metrics),
                    &automations,
                    Some(&usage),
                    Some("lead-session"),
                    Some(524_288_000),
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn info_panel_full_frame() {
        insta::assert_snapshot!(draw_panel(40, 46));
    }

    // ── byte identity against the pre-port renderer ──
    //
    // The pinned frame above proves one size and one fixture did not move. This
    // proves the *rule*: the view tree and the retained v1 line builders agree
    // cell for cell across every width the pane can be given and every content
    // variant it can hold. It is the assertion that makes "byte-identical" a
    // property rather than an anecdote.

    /// One case for the differential sweep: everything `render_info_panel` takes.
    struct Case {
        name: &'static str,
        info: SessionInfo,
        metrics: Option<SystemMetrics>,
        automations: Vec<AutomationEntry>,
        usage: Option<crate::session::AgentUsage>,
        parent: Option<&'static str>,
        disk: Option<u64>,
    }

    fn case(name: &'static str, info: SessionInfo) -> Case {
        Case {
            name,
            info,
            metrics: None,
            automations: Vec::new(),
            usage: None,
            parent: None,
            disk: None,
        }
    }

    /// Content variants chosen to reach every branch in the row builders, plus
    /// the long-value cases that make the wrap decide something.
    fn differential_cases() -> Vec<Case> {
        let mut cases = Vec::new();

        // A session with nothing optional set: every `if let` skipped.
        cases.push(case("bare", SessionInfo::new("demo".into())));

        // A clean local session with a cwd but no worktree, and empty git stats
        // (so the Changes/Sync rows are both skipped).
        let mut local = SessionInfo::new("local".into());
        local.status = crate::session::SessionStatus::Idle;
        local.cwd = Some(std::path::PathBuf::from("/src/thurbox"));
        local.git_stats = Some(crate::session::GitStats::default());
        cases.push(case("clean-local", local));

        // Dirty with no counted files, which is the one branch that appends
        // " dirty" to the Changes row.
        let mut dirty = SessionInfo::new("dirty".into());
        dirty.git_stats = Some(crate::session::GitStats {
            dirty: true,
            ..Default::default()
        });
        cases.push(case("dirty-only", dirty));

        // Agent metrics reporting nothing but a context percentage, so the gauge
        // renders with no suffix and every other agent row is skipped.
        let mut sparse = SessionInfo::new("sparse".into());
        sparse.status = crate::session::SessionStatus::Done;
        sparse.agent_metrics = Some(AgentMetrics {
            used_percentage: Some(7),
            ..Default::default()
        });
        cases.push(case("sparse-agent", sparse));

        // The pinned fixture, with every section populated.
        let mut full = case("full", fixture_info());
        full.metrics = Some(fixture_metrics());
        full.usage = Some(fixture_usage());
        full.automations = fixture_automations();
        full.parent = Some("lead-session");
        full.disk = Some(524_288_000);
        cases.push(full);

        // Values long enough that the wrap has to decide where to break, in
        // every row that carries agent- or user-supplied text.
        let mut long = fixture_info();
        long.name = "a-very-long-session-name-that-will-not-fit-in-a-narrow-panel".into();
        long.agent_activity =
            Some("Reading src/session/view_tree.rs and then writing a much longer sentence".into());
        long.notification =
            Some("the agent would like permission to run a command with a long name".into());
        long.hook_wiring = Some(
            "codex hooks were not provisioned on the psmux host named win, so status is stale"
                .into(),
        );
        long.worktrees = vec![crate::session::WorktreeInfo {
            repo_path: std::path::PathBuf::from("/src/a-repository-with-a-long-directory-name"),
            worktree_path: std::path::PathBuf::from("/wt/x"),
            branch: "feature/an-unusually-long-branch-name-for-testing".into(),
        }];
        let mut long_case = case("long-values", long);
        long_case.metrics = Some(fixture_metrics());
        long_case.parent = Some("a-lead-session-with-a-long-name");
        long_case.disk = Some(1_073_741_824);
        cases.push(long_case);

        // A usage window whose label plus suffix overflows the width: the one
        // case where a *gauge header* itself has to wrap.
        let mut wide_gauge = case("overflowing-gauge", SessionInfo::new("g".into()));
        wide_gauge.usage = Some(crate::session::AgentUsage {
            windows: vec![crate::session::UsageWindow {
                label: "claude-opus-4-a-long-model-identifier".into(),
                used_percent: 63.0,
                resets_at: None,
            }],
            plan: Some("a-long-plan-name".into()),
            note: None,
        });
        cases.push(wide_gauge);

        // No windows but a note, which is the other usage branch.
        let mut noted = case("usage-note", SessionInfo::new("n".into()));
        noted.usage = Some(crate::session::AgentUsage {
            windows: vec![],
            plan: None,
            note: Some("not logged in — run the agent's login command to see usage".into()),
        });
        cases.push(noted);

        cases
    }

    /// Render `case` twice at `w`×`h`: once through the view tree, once through
    /// the retained v1 line builders.
    ///
    /// Neither side draws the `Block`, so the comparison is of the pane's
    /// *content* — the border is byte-identical by construction, since
    /// `render_info_panel` still renders the same `Block` widget.
    fn render_both(c: &Case, w: u16, h: u16) -> (Buffer, Buffer) {
        let rect = Rect {
            x: 0,
            y: 0,
            width: w,
            height: h,
        };

        let mut ported = Buffer::empty(rect);
        let tree = info_tree(
            &c.info,
            c.metrics.as_ref(),
            &c.automations,
            c.usage.as_ref(),
            c.parent,
            c.disk,
            epoch_now_secs(),
        );
        super::super::plugin_pane::render_tree(
            &tree,
            rect,
            &super::super::theme::current(),
            &crate::session::motion::FrameTable::default(),
            &mut ported,
        );

        let mut legacy = Buffer::empty(rect);
        let lines = legacy_lines(
            &c.info,
            c.metrics.as_ref(),
            &c.automations,
            c.usage.as_ref(),
            c.parent,
            c.disk,
            w as usize,
        );
        ratatui::widgets::Widget::render(
            Paragraph::new(lines).wrap(Wrap { trim: false }),
            rect,
            &mut legacy,
        );

        (ported, legacy)
    }

    #[test]
    fn view_tree_render_matches_the_legacy_paragraph_cell_for_cell() {
        // Widths from "narrower than any label" up to comfortably wide; heights
        // covering full render and every degree of bottom truncation.
        for c in differential_cases() {
            for w in 6..=64u16 {
                for h in [1u16, 2, 3, 5, 12, 96] {
                    let (ported, legacy) = render_both(&c, w, h);
                    for y in 0..h {
                        for x in 0..w {
                            let (a, b) = (&ported[(x, y)], &legacy[(x, y)]);
                            assert_eq!(
                                a.symbol(),
                                b.symbol(),
                                "{} at {w}x{h} cell ({x},{y})\nported: {:?}\nlegacy: {:?}",
                                c.name,
                                row_text(&ported, y, w),
                                row_text(&legacy, y, w),
                            );
                            if a.fg == b.fg && a.modifier == b.modifier && a.bg == b.bg {
                                continue;
                            }
                            // The one tolerated divergence: v1 drew a few
                            // separator spaces with `Span::raw`, leaving those
                            // cells' foreground unset, where the tree gives them
                            // a theme colour. A space has no glyph and neither
                            // side sets a background, so the two are the same
                            // pixels. Anything else is a real drift.
                            assert!(
                                a.symbol().trim().is_empty()
                                    && b.fg == ratatui::style::Color::Reset
                                    && a.modifier == b.modifier
                                    && a.bg == b.bg,
                                "{} at {w}x{h} cell ({x},{y}) style drift: \
                                 ported {:?}/{:?} vs legacy {:?}/{:?} on {:?}",
                                c.name,
                                a.fg,
                                a.modifier,
                                b.fg,
                                b.modifier,
                                a.symbol(),
                            );
                        }
                    }
                }
            }
        }
    }

    /// One buffer row as text, for a failure message that shows the whole line.
    fn row_text(buf: &Buffer, y: u16, w: u16) -> String {
        (0..w).map(|x| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn the_only_style_divergence_is_an_invisible_space() {
        // Pins divergence 1 in the change's `design.md` rather than leaving it
        // as prose: the Changes row's separator space is the cell in question,
        // and it must be a *space* whose colour differs — never a glyph.
        let mut info = SessionInfo::new("d".into());
        info.git_stats = Some(crate::session::GitStats {
            files_changed: 2,
            insertions: 3,
            deletions: 4,
            ..Default::default()
        });
        let (ported, legacy) = render_both(&case("changes", info), 40, 8);
        let mut differing = Vec::new();
        for y in 0..8u16 {
            for x in 0..40u16 {
                let (a, b) = (&ported[(x, y)], &legacy[(x, y)]);
                if a.fg != b.fg {
                    differing.push(a.symbol().to_string());
                }
            }
        }
        assert!(
            !differing.is_empty(),
            "the divergence must actually be reachable, or this test proves nothing"
        );
        assert!(
            differing.iter().all(|s| s == " "),
            "only space cells may differ, got {differing:?}"
        );
    }

    #[test]
    fn agent_supplied_control_characters_no_longer_reach_the_terminal() {
        // Divergence 2: v1 put an OSC-derived string straight into a `Span`, and
        // ratatui writes a cell's symbol to the terminal verbatim — so an escape
        // sequence in a window title reached the terminal as a control code. The
        // view tree sanitizes it. This is the port changing behaviour on purpose.
        let mut info = SessionInfo::new("x".into());
        info.agent_activity = Some("safe\x1b[2Jtext".into());
        let tree = info_tree(&info, None, &[], None, None, None, 0);
        let rect = Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 8,
        };
        let mut buf = Buffer::empty(rect);
        super::super::plugin_pane::render_tree(
            &tree,
            rect,
            &super::super::theme::current(),
            &crate::session::motion::FrameTable::default(),
            &mut buf,
        );
        let painted: String = (0..8)
            .flat_map(|y| (0..40).map(move |x| (x, y)))
            .map(|(x, y)| buf[(x, y)].symbol())
            .collect();
        assert!(
            !painted.contains('\x1b'),
            "no escape introducer may reach a cell"
        );
        assert!(painted.contains("safe[2Jtext"), "{painted:?}");
    }

    #[test]
    fn the_tree_carries_no_geometry() {
        // The property that makes the port meaningful: `info_tree` takes no area
        // and no width, so everything a plugin could not know is now resolved by
        // the renderer. A regression here would mean a later row had gone back to
        // pre-computing its own layout.
        let a = info_tree(&fixture_info(), None, &[], None, None, None, 0);
        let b = info_tree(&fixture_info(), None, &[], None, None, None, 0);
        assert_eq!(a, b, "the tree is a pure function of its inputs");

        fn assert_no_padding_runs(node: &ViewNode) {
            if let ViewNode::Text { content, .. } = node {
                // The gauge header's flush-right padding was the pane's only
                // width-derived run; a long space run reappearing would mean a
                // row had started measuring again. The repo continuation's
                // seven-space label is fixed, not measured, so the bound is
                // generous.
                assert!(
                    content.chars().filter(|c| *c == ' ').count() <= 9,
                    "a measured padding run reappeared: {content:?}"
                );
            }
            for child in node.children() {
                assert_no_padding_runs(child);
            }
        }
        let full = info_tree(
            &fixture_info(),
            Some(&fixture_metrics()),
            &fixture_automations(),
            Some(&fixture_usage()),
            Some("lead"),
            Some(1),
            epoch_now_secs(),
        );
        assert_no_padding_runs(&full);
    }

    // ── append_session_section tests ──

    #[test]
    fn session_section_renders_hook_wiring_degradation() {
        let mut info = SessionInfo::new("demo".into());
        let mut lines = Vec::new();
        append_session_section(&mut lines, &info, None);
        let text = |lines: &[Line]| {
            lines
                .iter()
                .map(|l| {
                    l.spans
                        .iter()
                        .map(|s| s.content.as_ref())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        // Healthy/local session: no Hooks row at all.
        assert!(!text(&lines).contains("Hooks:"));

        info.hook_wiring = Some("codex hooks not provisioned on psmux host 'win'".into());
        let mut lines = Vec::new();
        append_session_section(&mut lines, &info, None);
        let rendered = text(&lines);
        assert!(rendered.contains("Hooks: degraded — codex hooks not provisioned"));
    }

    // ── append_system_section tests ──

    fn render_system(m: &SystemMetrics, thurbox_dir_bytes: Option<u64>) -> String {
        let mut lines: Vec<Line> = Vec::new();
        append_system_section(&mut lines, m, thurbox_dir_bytes, 24);
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn sample_metrics() -> SystemMetrics {
        SystemMetrics {
            cpu_percent: 10.0,
            memory_used: 8_589_934_592,
            memory_total: 17_179_869_184,
            session_cpu_percent: 0.0,
            session_memory_bytes: 0,
        }
    }

    #[test]
    fn system_section_shows_thurbox_dir_size_when_measured() {
        let out = render_system(&sample_metrics(), Some(524_288_000));
        assert!(out.contains("Disk"));
        assert!(out.contains("500.0 MB"));
        assert!(out.contains("thurbox dir"));
    }

    #[test]
    fn system_section_omits_disk_line_before_first_scan() {
        let out = render_system(&sample_metrics(), None);
        assert!(!out.contains("thurbox dir"));
    }

    // ── human_bytes tests ──

    #[test]
    fn human_bytes_gigabytes() {
        let (val, _, unit) = human_bytes(2_147_483_648); // 2 GB
        assert!((val - 2.0).abs() < 0.01);
        assert_eq!(unit, "GB");
    }

    #[test]
    fn human_bytes_megabytes() {
        let (val, _, unit) = human_bytes(104_857_600); // 100 MB
        assert!((val - 100.0).abs() < 0.01);
        assert_eq!(unit, "MB");
    }

    #[test]
    fn human_bytes_kilobytes() {
        let (val, _, unit) = human_bytes(512_000); // ~500 KB
        assert_eq!(unit, "KB");
        assert!(val > 400.0 && val < 600.0);
    }

    #[test]
    fn human_bytes_zero() {
        let (val, _, unit) = human_bytes(0);
        assert!((val - 0.0).abs() < 0.01);
        assert_eq!(unit, "KB");
    }

    #[test]
    fn human_bytes_divisor_matches_unit() {
        let bytes = 3_221_225_472u64; // 3 GB
        let (val, divisor, unit) = human_bytes(bytes);
        assert_eq!(unit, "GB");
        assert!((bytes as f64 / divisor - val).abs() < 0.001);
    }

    // ── format_bytes tests ──

    #[test]
    fn format_bytes_megabytes() {
        assert_eq!(format_bytes(524_288_000), "500.0 MB");
    }

    #[test]
    fn format_bytes_zero() {
        assert_eq!(format_bytes(0), "0.0 KB");
    }

    // ── format_bytes_pair tests ──

    #[test]
    fn format_bytes_pair_same_unit() {
        let s = format_bytes_pair(8_589_934_592, 17_179_869_184); // 8/16 GB
        assert_eq!(s, "8.0/16.0 GB");
    }

    #[test]
    fn format_bytes_pair_zero_used() {
        let s = format_bytes_pair(0, 17_179_869_184);
        assert_eq!(s, "0.0/16.0 GB");
    }

    // ── render_gauge_lines tests ──

    #[test]
    fn render_gauge_lines_zero_percent() {
        let lines = render_gauge_lines("CPU", 0.0, None, 20);
        assert_eq!(lines.len(), 2);
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(header.contains("CPU"));
        assert!(header.contains("0%"));
        let bar: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        // bar_width = 20 - 2 = 18
        assert!(bar.contains(&"░".repeat(18)));
        assert!(!bar.contains('█'));
    }

    #[test]
    fn render_gauge_lines_hundred_percent() {
        let lines = render_gauge_lines("CPU", 100.0, None, 20);
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(header.contains("100%"));
        let bar: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        // bar_width = 18
        assert!(bar.contains(&"█".repeat(18)));
        assert!(!bar.contains('░'));
    }

    #[test]
    fn render_gauge_lines_fifty_percent() {
        let lines = render_gauge_lines("RAM", 50.0, None, 20);
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(header.contains("50%"));
    }

    #[test]
    fn render_gauge_lines_with_suffix() {
        let lines = render_gauge_lines("RAM", 50.0, Some("8.0/16.0 GB".to_string()), 20);
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(header.contains("8.0/16.0 GB"));
        assert!(!header.contains("50%"));
    }

    #[test]
    fn render_gauge_lines_clamps_above_100() {
        let lines = render_gauge_lines("CPU", 150.0, None, 20);
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(header.contains("100%"));
    }

    #[test]
    fn render_gauge_lines_clamps_below_zero() {
        let lines = render_gauge_lines("CPU", -10.0, None, 20);
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(header.contains("0%"));
    }

    #[test]
    fn render_gauge_lines_bar_width_matches_inner_width() {
        let inner_width = 16;
        let lines = render_gauge_lines("CPU", 42.0, None, inner_width);
        let bar: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        let bar_content_len = bar.chars().count();
        // [ + bar_width + ] == inner_width
        assert_eq!(bar_content_len, inner_width);
    }

    // ── separator tests ──

    #[test]
    fn separator_width_matches() {
        let line = separator(16);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.chars().count(), 16);
        assert!(text.chars().all(|c| c == '─'));
    }

    // ── format_tokens tests ──

    #[test]
    fn format_tokens_small() {
        assert_eq!(format_tokens(42), "42");
        assert_eq!(format_tokens(999), "999");
    }

    #[test]
    fn format_tokens_thousands() {
        assert_eq!(format_tokens(1_000), "1.0k");
        assert_eq!(format_tokens(15_200), "15.2k");
        assert_eq!(format_tokens(999_999), "1000.0k");
    }

    #[test]
    fn format_tokens_millions() {
        assert_eq!(format_tokens(1_000_000), "1.0M");
        assert_eq!(format_tokens(2_500_000), "2.5M");
    }

    // ── format_cost tests ──

    #[test]
    fn format_cost_sub_dollar_uses_four_dp() {
        assert_eq!(format_cost(0.0423), "$0.0423");
        assert_eq!(format_cost(0.0), "$0.0000");
    }

    #[test]
    fn format_cost_dollar_and_above_uses_two_dp() {
        assert_eq!(format_cost(1.0), "$1.00");
        assert_eq!(format_cost(12.345), "$12.35");
    }

    // ── format_duration tests ──

    #[test]
    fn format_duration_millis() {
        assert_eq!(format_duration(0), "0ms");
        assert_eq!(format_duration(820), "820ms");
    }

    #[test]
    fn format_duration_seconds() {
        assert_eq!(format_duration(1_000), "1s");
        assert_eq!(format_duration(45_000), "45s");
    }

    #[test]
    fn format_duration_minutes() {
        assert_eq!(format_duration(83_000), "1m 23s");
        assert_eq!(format_duration(600_000), "10m 00s");
    }

    #[test]
    fn format_duration_hours() {
        assert_eq!(format_duration(3_600_000), "1h 00m");
        assert_eq!(format_duration(7_500_000), "2h 05m");
    }

    // ── append_git_section tests ──

    fn render_git(git: &crate::session::GitStats) -> String {
        let mut lines: Vec<Line> = Vec::new();
        append_git_section(&mut lines, git);
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn git_section_shows_changes_and_sync() {
        let git = crate::session::GitStats {
            files_changed: 3,
            insertions: 120,
            deletions: 8,
            untracked: 0,
            dirty: true,
            ahead: 2,
            behind: 0,
        };
        let out = render_git(&git);
        assert!(out.contains("Changes:"));
        assert!(out.contains("3 files"));
        assert!(out.contains("+120"));
        assert!(out.contains("-8"));
        assert!(out.contains("Sync:"));
        assert!(out.contains("↑2 ↓0"));
    }

    #[test]
    fn git_section_singular_file_and_no_sync() {
        let git = crate::session::GitStats {
            files_changed: 1,
            insertions: 5,
            deletions: 0,
            untracked: 0,
            dirty: true,
            ahead: 0,
            behind: 0,
        };
        let out = render_git(&git);
        assert!(out.contains("1 file"));
        assert!(!out.contains("files"));
        assert!(!out.contains("Sync:"));
    }

    #[test]
    fn git_section_clean_repo_is_empty() {
        let git = crate::session::GitStats::default();
        assert_eq!(render_git(&git), "");
    }

    // ── usage section tests ──

    #[test]
    fn format_countdown_variants() {
        assert_eq!(format_countdown_secs(0), "now");
        assert_eq!(format_countdown_secs(30), "<1m");
        assert_eq!(format_countdown_secs(5 * 60), "5m");
        assert_eq!(format_countdown_secs(3600 + 12 * 60), "1h 12m");
        assert_eq!(format_countdown_secs(4 * 86400 + 3 * 3600), "4d 3h");
    }

    fn render_usage(u: &crate::session::AgentUsage) -> String {
        render_usage_on(u, None)
    }

    fn render_usage_on(u: &crate::session::AgentUsage, host: Option<&str>) -> String {
        let mut lines: Vec<Line> = Vec::new();
        append_usage_section(&mut lines, u, host, 24);
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn usage_section_renders_plan_and_windows() {
        let u = crate::session::AgentUsage {
            windows: vec![
                crate::session::UsageWindow {
                    label: "5h".into(),
                    used_percent: 42.0,
                    resets_at: None,
                },
                crate::session::UsageWindow {
                    label: "Week".into(),
                    used_percent: 18.0,
                    resets_at: None,
                },
            ],
            plan: Some("max".into()),
            note: None,
        };
        let out = render_usage(&u);
        assert!(out.contains("Usage (max)"));
        assert!(out.contains("5h"));
        assert!(out.contains("42%"));
        assert!(out.contains("Week"));
        assert!(out.contains("18%"));
    }

    #[test]
    fn usage_section_renders_note_when_no_windows() {
        let u = crate::session::AgentUsage {
            windows: vec![],
            plan: None,
            note: Some("not logged in".into()),
        };
        let out = render_usage(&u);
        assert!(out.contains("Usage"));
        assert!(out.contains("not logged in"));
    }

    #[test]
    fn usage_section_labels_remote_host() {
        let u = crate::session::AgentUsage {
            windows: vec![crate::session::UsageWindow {
                label: "5h".into(),
                used_percent: 42.0,
                resets_at: None,
            }],
            plan: Some("max".into()),
            note: None,
        };
        assert!(render_usage_on(&u, Some("devbox")).contains("Usage (max · devbox)"));
        let no_plan = crate::session::AgentUsage {
            plan: None,
            ..u.clone()
        };
        assert!(render_usage_on(&no_plan, Some("devbox")).contains("Usage (devbox)"));
    }
}
