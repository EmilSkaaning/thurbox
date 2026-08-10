//! The v2 plugin host: Luau VMs, discovery, lifecycle, and capability
//! enforcement.
//!
//! Thurbox v2 moves user-visible surfaces out of the binary and into plugins.
//! This module is the kernel half of that split — it finds plugins, runs their
//! code in bounded sandboxes, and bounds what each one can reach. It draws
//! nothing; rendering is the caller's business.
//!
//! The layering mirrors the constraints the host has to hold:
//!
//! - [`crate::session::plugin_manifest`] owns the manifest as **pure data**, so
//!   a plugin's surfaces and requested powers are readable with no VM involved.
//! - [`discovery`] turns directories into candidate plugins, resolving
//!   precedence and reporting everything it rejected.
//! - [`capabilities`] builds the per-plugin environment, granting a binding by
//!   *inserting* it and denying one by leaving it out.
//! - [`runtime`] owns a VM and its thread. A VM is never touched by another
//!   thread — `mlua::Lua` is `!Send` here by choice, so the compiler enforces
//!   that plugin execution stays off the render loop.
//! - [`lifecycle`] sequences the two: discovered → loaded → running → stopped,
//!   with every failure recorded against the plugin that caused it.
//! - [`commands`] turns manifests into the command registry every surface
//!   resolves through — likewise with no VM, so a plugin that never started
//!   still lists what it offers — and bounds a command result on its way out.
//! - [`spawn`] turns manifests into the spawn-contribution registry the kernel
//!   reads when it launches an agent — the one plugin surface that involves no
//!   VM at all, because the spawn path must not enter one.
//!
//! Nothing in this module is compiled unless the `plugins` feature is on.

pub mod capabilities;
pub mod commands;
pub mod discovery;
pub mod lifecycle;
pub mod pane;
pub mod runtime;
pub mod service;
pub mod spawn;
pub mod view;

pub use capabilities::GrantedCapabilities;
pub use discovery::{DiscoveredPlugin, DiscoveryOutcome, PluginSource};
pub use lifecycle::{PluginHost, PluginReport, PluginState, PluginStatus, Transition};
pub use pane::{PanePresentation, PluginPane};
pub use runtime::{ExecutionBounds, RuntimeError};
pub use service::{ServiceError, ServiceHost};
pub use view::ViewError;
