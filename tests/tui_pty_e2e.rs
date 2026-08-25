//! PTY end-to-end tests: the real `thurbox` binary on a real pseudo-terminal.
//!
//! Everything else in the suite paints to a `TestBackend`, which by design
//! never touches a tty — so nothing there can see the byte stream the binary
//! actually writes: the alternate-screen enter/leave, the mouse-reporting
//! modes, raw mode, or how the loop behaves when the window resizes under it.
//! The tmux smoke test drives real frames but reads them back through
//! `capture-pane`, which reconstructs the screen and discards those same
//! bytes. This file is where they are asserted.
//!
//! The failure it guards is the one users hit hardest: a TUI that exits (or
//! crashes) without restoring the terminal leaves the shell in raw mode
//! streaming mouse reports. `restore_terminal` exists for that; these tests
//! pin its output. Frames are reconstructed with the same `vt100` the render
//! path uses, so assertions survive any interleaving of diff repaints.
//!
//! Unix-only, and `ptyprocess` on purpose: these tests never need Windows
//! (the ConPTY path is exercised by the windows-vm e2e harness, and ConPTY
//! timing on shared CI runners is not something to gate merges on), and
//! ptyprocess's dependency tree is essentially `nix` alone — a
//! cross-platform PTY crate would buy portability nothing uses at the price
//! of a wider tree (portable-pty carries the unmaintained `serial`,
//! RUSTSEC-2017-0008, which would need a standing deny.toml exception).
#![cfg(unix)]

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ptyprocess::stream::Stream;
use ptyprocess::{PtyProcess, WaitStatus};

struct Tui {
    proc: PtyProcess,
    writer: Stream,
    /// The child's status once it stopped being `StillAlive` — `status()`
    /// reaps, so it is read once and remembered.
    exited: Option<WaitStatus>,
    /// Every byte the binary wrote, verbatim — the escape-sequence record.
    raw: Arc<Mutex<Vec<u8>>>,
    /// The same bytes fed through vt100, for asserting on the visible frame.
    parser: Arc<Mutex<vt100::Parser>>,
    _dirs: tempfile::TempDir,
}

impl Tui {
    /// Launch the binary fully isolated: private HOME/config/data dirs, a
    /// private `TMUX_TMPDIR`, and the network-facing and tmux-arming feature
    /// flags off — the same hermetic shape `scripts/dev/lib/sandbox-env.sh`
    /// gives the smoke test.
    fn spawn(rows: u16, cols: u16) -> Self {
        let dirs = tempfile::tempdir().expect("tempdir");
        let root = dirs.path();
        for sub in ["home", "config", "data", "tmux"] {
            std::fs::create_dir_all(root.join(sub)).expect("mkdir");
        }
        // No update check, no version check (both reach the network), and no
        // automation heartbeat (it would leave a tmux server behind).
        std::fs::write(
            root.join("config/settings.toml"),
            "[features]\nautomations = false\nversion_check = false\nauto_update = false\n",
        )
        .expect("seed settings");

        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_thurbox"));
        cmd.current_dir(root);
        cmd.env("HOME", root.join("home"));
        cmd.env("THURBOX_CONFIG_DIR", root.join("config"));
        cmd.env("THURBOX_DATA_DIR", root.join("data"));
        cmd.env("TMUX_TMPDIR", root.join("tmux"));
        cmd.env("TERM", "xterm-256color");
        // A test run inside tmux must not look like one to the binary.
        cmd.env_remove("TMUX");

        let mut proc = PtyProcess::spawn(cmd).expect("spawn thurbox");
        // The child starts on ptyprocess's default 80x24; the resize lands as
        // a SIGWINCH the loop handles like any user resize.
        proc.set_window_size(cols, rows).expect("set window size");

        let mut reader = proc.get_pty_stream().expect("reader");
        let writer = proc.get_pty_stream().expect("writer");
        let raw = Arc::new(Mutex::new(Vec::new()));
        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 0)));
        {
            let raw = Arc::clone(&raw);
            let parser = Arc::clone(&parser);
            // Reads until EOF/EIO, which is how a pty reports the child gone.
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                while let Ok(n) = reader.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    raw.lock().unwrap().extend_from_slice(&buf[..n]);
                    parser.lock().unwrap().process(&buf[..n]);
                }
            });
        }

        Self {
            proc,
            writer,
            exited: None,
            raw,
            parser,
            _dirs: dirs,
        }
    }

    fn screen_text(&self) -> String {
        self.parser.lock().unwrap().screen().contents()
    }

    fn raw_len(&self) -> usize {
        self.raw.lock().unwrap().len()
    }

    fn raw_string(&self) -> String {
        String::from_utf8_lossy(&self.raw.lock().unwrap()).into_owned()
    }

    /// Poll the reconstructed frame until it shows `needle`.
    fn wait_for(&self, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if self.screen_text().contains(needle) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!(
            "timed out waiting for {needle:?}; final frame:\n{}",
            self.screen_text()
        );
    }

    fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).expect("write to pty");
        self.writer.flush().expect("flush pty");
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        self.proc.set_window_size(cols, rows).expect("resize pty");
        self.parser
            .lock()
            .unwrap()
            .screen_mut()
            .set_size(rows, cols);
    }

    /// The child's exit status, if it has exited. `waitpid` reaps on first
    /// sight, so the answer is cached the moment it stops being alive.
    fn poll_exit(&mut self) -> Option<WaitStatus> {
        if self.exited.is_none() {
            match self.proc.status() {
                Ok(WaitStatus::StillAlive) => {}
                Ok(status) => self.exited = Some(status),
                // ECHILD after a reap elsewhere cannot happen (nothing else
                // waits on it); treat an error as "gone" with no status.
                Err(_) => {}
            }
        }
        self.exited
    }

    fn alive(&mut self) -> bool {
        self.poll_exit().is_none()
    }

    /// Wait for the process to exit on its own; panics if it doesn't.
    fn wait_exit(&mut self) -> WaitStatus {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if let Some(status) = self.poll_exit() {
                return status;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("thurbox did not exit; final frame:\n{}", self.screen_text());
    }
}

const CTRL_Q: &[u8] = b"\x11";

/// A clean `exit(0)`, whatever pid carried it.
fn exited_cleanly(status: WaitStatus) -> bool {
    matches!(status, WaitStatus::Exited(_, 0))
}

#[test]
fn boots_paints_and_exits_restoring_the_terminal() {
    let mut tui = Tui::spawn(40, 120);
    tui.wait_for("thurbox");
    tui.wait_for("No sessions yet");

    tui.send(CTRL_Q);
    let status = tui.wait_exit();
    assert!(
        exited_cleanly(status),
        "Ctrl+Q must exit cleanly: {status:?}"
    );

    let raw = tui.raw_string();
    // It took the alternate screen…
    assert!(
        raw.contains("\x1b[?1049h"),
        "the TUI never entered the alternate screen"
    );
    // …and gave everything back on the way out. A missing one of these is the
    // "my shell is streaming mouse reports" bug, which no in-process test and
    // no capture-pane assertion can see.
    for (seq, meaning) in [
        ("\x1b[?1049l", "leave the alternate screen"),
        ("\x1b[?1000l", "stop mouse reporting"),
        ("\x1b[?2004l", "disable bracketed paste"),
    ] {
        assert!(
            raw.contains(seq),
            "exit must {meaning} ({seq:?} missing from the byte stream)"
        );
    }
}

#[test]
fn survives_a_resize_storm_including_tiny_sizes() {
    // Resizing under the loop is where underflow lives (a one-cell pane is
    // exactly what `vt_floor` exists for). The binary must keep painting
    // through arbitrary sizes and still exit cleanly afterwards.
    let mut tui = Tui::spawn(40, 120);
    tui.wait_for("thurbox");

    for (rows, cols) in [(24, 80), (6, 20), (2, 2), (50, 140), (3, 4), (30, 100)] {
        tui.resize(rows, cols);
        std::thread::sleep(Duration::from_millis(120));
        assert!(
            tui.alive(),
            "thurbox died after resize to {rows}x{cols}; frame:\n{}",
            tui.screen_text()
        );
    }

    // Proof of life after the storm: the loop still paints (the forced-redraw
    // floor guarantees output within 250 ms of a live loop).
    let before = tui.raw_len();
    let deadline = Instant::now() + Duration::from_secs(10);
    while tui.raw_len() == before && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(tui.raw_len() > before, "no repaint after the resize storm");

    tui.send(CTRL_Q);
    let status = tui.wait_exit();
    assert!(
        exited_cleanly(status),
        "exit after resizing must be clean: {status:?}"
    );
}
