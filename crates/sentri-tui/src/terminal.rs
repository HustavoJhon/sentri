//! What SENTRI knows about the terminal it runs in, explained for people.
//!
//! The emulator is guessed from environment variables (every terminal sets
//! a few of its own); the graphics protocol comes from asking the terminal
//! itself (see `graphics`). Inside tmux the environment can belong to the
//! terminal tmux was first started from, so the guess is labelled as such.

use std::fmt::Write as _;

use sentri_app::ImageMode;

use crate::graphics::Backend;

/// A terminal emulator, as far as the environment tells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emulator {
    Kitty,
    Foot,
    Alacritty,
    Ghostty,
    WezTerm,
    Iterm2,
    Konsole,
    WindowsTerminal,
    /// GNOME Terminal, Tilix, Terminator… (all built on VTE).
    Vte,
    Xterm,
    LinuxConsole,
    Unknown,
}

impl Emulator {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Kitty => "kitty",
            Self::Foot => "foot",
            Self::Alacritty => "Alacritty",
            Self::Ghostty => "Ghostty",
            Self::WezTerm => "WezTerm",
            Self::Iterm2 => "iTerm2",
            Self::Konsole => "Konsole",
            Self::WindowsTerminal => "Windows Terminal",
            Self::Vte => "a VTE terminal (GNOME Terminal, Tilix…)",
            Self::Xterm => "xterm",
            Self::LinuxConsole => "the Linux console",
            Self::Unknown => "unknown",
        }
    }

    /// The graphics protocol SENTRI has verified with this emulator, used
    /// to suggest `images = "auto"` when the configuration turns it off.
    pub const fn verified_graphics(self) -> Option<&'static str> {
        match self {
            Self::Kitty => Some("the Kitty graphics protocol"),
            Self::Foot => Some("Sixel"),
            _ => None,
        }
    }

    /// What this emulator is documented to support, when it is known for
    /// sure; `None` when it depends on the version or build.
    pub const fn known_graphics(self) -> Option<&'static str> {
        match self {
            Self::Kitty => Some("Kitty graphics protocol (no Sixel)"),
            Self::Foot => Some("Sixel (no Kitty graphics protocol)"),
            Self::Alacritty => Some("neither the Kitty graphics protocol nor Sixel"),
            Self::LinuxConsole => Some("no graphics protocol"),
            _ => None,
        }
    }
}

/// Reads the emulator from `var` (an environment lookup).
pub fn emulator_from(var: impl Fn(&str) -> Option<String>) -> Emulator {
    let set = |name: &str| var(name).is_some_and(|v| !v.is_empty());
    let term = var("TERM").unwrap_or_default();
    let program = var("TERM_PROGRAM").unwrap_or_default().to_lowercase();
    if set("KITTY_WINDOW_ID") || term == "xterm-kitty" {
        Emulator::Kitty
    } else if program == "ghostty" || set("GHOSTTY_RESOURCES_DIR") || term == "xterm-ghostty" {
        Emulator::Ghostty
    } else if program == "wezterm" || set("WEZTERM_EXECUTABLE") {
        Emulator::WezTerm
    } else if program == "iterm.app" || set("ITERM_SESSION_ID") {
        Emulator::Iterm2
    } else if set("ALACRITTY_WINDOW_ID") || set("ALACRITTY_SOCKET") || term == "alacritty" {
        Emulator::Alacritty
    } else if term.starts_with("foot") {
        Emulator::Foot
    } else if set("KONSOLE_VERSION") {
        Emulator::Konsole
    } else if set("WT_SESSION") {
        Emulator::WindowsTerminal
    } else if set("VTE_VERSION") {
        Emulator::Vte
    } else if term == "linux" {
        Emulator::LinuxConsole
    } else if term.starts_with("xterm") && set("XTERM_VERSION") {
        Emulator::Xterm
    } else {
        Emulator::Unknown
    }
}

/// The emulator: the terminal process SENTRI runs under when it can be
/// found (Linux), else what the environment says. The process is more
/// reliable: `TERM` is often overridden (foot set to `xterm-256color`) and
/// variables leak into terminals started from another terminal.
pub fn emulator() -> Emulator {
    match emulator_from_processes() {
        Some(found) => found,
        None => emulator_from(|name| std::env::var(name).ok()),
    }
}

/// Emulator named by a process (its `comm`), if it is one.
pub fn emulator_named(comm: &str) -> Option<Emulator> {
    Some(match comm.trim() {
        "kitty" => Emulator::Kitty,
        "foot" | "footclient" => Emulator::Foot,
        "alacritty" => Emulator::Alacritty,
        "ghostty" => Emulator::Ghostty,
        "wezterm-gui" | "wezterm" => Emulator::WezTerm,
        "konsole" => Emulator::Konsole,
        "xterm" => Emulator::Xterm,
        name if name.starts_with("gnome-terminal") || name == "tilix" || name == "terminator" => {
            Emulator::Vte
        }
        _ => return None,
    })
}

/// Walks up the process tree to the terminal. Stops (returns `None`) at
/// tmux, screen or an SSH server: past them the terminal is elsewhere.
#[cfg(target_os = "linux")]
fn emulator_from_processes() -> Option<Emulator> {
    let mut current = std::process::id();
    for _ in 0..16 {
        let stat = std::fs::read_to_string(format!("/proc/{current}/stat")).ok()?;
        // `pid (comm) state parent …`; comm may contain spaces or parens.
        let (_, after) = stat.rsplit_once(')')?;
        let parent: u32 = after.split_whitespace().nth(1)?.parse().ok()?;
        if parent <= 1 {
            return None;
        }
        let comm = std::fs::read_to_string(format!("/proc/{parent}/comm")).ok()?;
        let comm = comm.trim();
        if comm.starts_with("tmux")
            || comm == "screen"
            || comm == "sshd"
            || comm.starts_with("sshd")
        {
            return None;
        }
        if let Some(found) = emulator_named(comm) {
            return Some(found);
        }
        current = parent;
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn emulator_from_processes() -> Option<Emulator> {
    None
}

/// tmux, when SENTRI runs inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tmux {
    /// `allow-passthrough` is on: graphics can reach the outer terminal.
    pub passthrough: bool,
}

/// `Some` inside tmux, with its passthrough setting (inherited values from
/// `set -g` included).
pub fn tmux() -> Option<Tmux> {
    std::env::var_os("TMUX")?;
    let mut command = std::process::Command::new("tmux");
    command.args(["display-message", "-p"]);
    if let Some(pane) = std::env::var_os("TMUX_PANE") {
        command.arg("-t").arg(pane);
    }
    let passthrough = command
        .arg("#{allow-passthrough}")
        .stdin(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| passthrough_enabled(&String::from_utf8_lossy(&o.stdout)));
    Some(Tmux { passthrough })
}

/// `on` and `all` forward sequences; empty or `off` do not.
pub fn passthrough_enabled(value: &str) -> bool {
    matches!(value.trim(), "on" | "all")
}

/// Everything that decided how images are drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnosis {
    pub emulator: Emulator,
    pub term: String,
    pub tmux: Option<Tmux>,
    /// `images` in the configuration.
    pub mode: ImageMode,
    /// The terminal was asked about graphics.
    pub probed: bool,
    /// What the terminal answered (`None`: not asked, or nothing).
    pub detected: Option<Backend>,
    /// Cell size in pixels, when the terminal reported it.
    pub cell: Option<(u16, u16)>,
    /// What draws images now.
    pub active: Backend,
}

impl Default for Diagnosis {
    fn default() -> Self {
        Self {
            emulator: Emulator::Unknown,
            term: String::new(),
            tmux: None,
            mode: ImageMode::Auto,
            probed: false,
            detected: None,
            cell: None,
            active: Backend::Text,
        }
    }
}

impl Diagnosis {
    /// Why no graphics protocol is in use, or `None` when one is.
    pub fn reason(&self) -> Option<String> {
        if self.active != Backend::Text {
            return None;
        }
        Some(match self.mode {
            ImageMode::Off => "images are turned off (images = \"off\")".into(),
            ImageMode::Halfblock | ImageMode::Ascii => {
                let mode = if self.mode == ImageMode::Ascii {
                    "ascii"
                } else {
                    "halfblock"
                };
                let mut text = format!(
                    "the configuration asks for character images (images = \"{mode}\"), so the \
                     terminal is not asked"
                );
                if let Some(native) = self.native_hint() {
                    let _ = write!(text, "; {native}");
                }
                text
            }
            _ if self.tmux.is_some_and(|t| !t.passthrough) => {
                "tmux does not forward graphics to the terminal; add `set -g allow-passthrough on` \
                 to ~/.tmux.conf (SENTRI never changes it for you)"
                    .into()
            }
            _ if !self.probed => "the terminal was not asked (output is not a terminal)".into(),
            _ if self.tmux.is_some()
                && matches!(self.detected, Some(Backend::Sixel | Backend::Iterm2)) =>
            {
                format!(
                    "the terminal supports {}, but inside tmux those images are drawn in the \
                     wrong place; run SENTRI outside tmux, or force it with images = \"{}\"",
                    self.detected.map_or("", Backend::label),
                    if self.detected == Some(Backend::Sixel) {
                        "sixel"
                    } else {
                        "iterm2"
                    }
                )
            }
            _ => {
                let mut text = String::from("the terminal did not announce a graphics protocol");
                if let Some(known) = self.emulator.known_graphics() {
                    let _ = write!(text, "; {} supports {known}", self.emulator.name());
                }
                text
            }
        })
    }

    /// When the configuration forces character images in a terminal whose
    /// real graphics SENTRI has verified: how to turn them on. Outside tmux
    /// only (inside it, the terminal cannot be identified for sure).
    pub fn native_hint(&self) -> Option<String> {
        if !matches!(self.mode, ImageMode::Halfblock | ImageMode::Ascii) || self.tmux.is_some() {
            return None;
        }
        let protocol = self.emulator.verified_graphics()?;
        Some(format!(
            "{} can show real images with {protocol}: set images = \"auto\" (Settings › Images) and restart",
            self.emulator.name()
        ))
    }

    /// What is drawn instead of real pixels.
    pub fn fallback(&self) -> &'static str {
        match (self.active, self.mode) {
            (Backend::Text, ImageMode::Off) => "no images",
            (Backend::Text, ImageMode::Ascii) => "ASCII shading (one character per cell)",
            (Backend::Text, _) => "half blocks (▀): two colored pixels per cell",
            _ => "none needed",
        }
    }

    /// Label and value pairs, for the CLI and Settings.
    pub fn rows(&self) -> Vec<(&'static str, String)> {
        let mut terminal = self.emulator.name().to_owned();
        if self.tmux.is_some() && self.emulator != Emulator::Unknown {
            terminal.push_str(" (from the environment; tmux may hide the real one)");
        }
        if !self.term.is_empty() {
            let _ = write!(terminal, " · TERM={}", self.term);
        }
        let tmux = match self.tmux {
            None => "no".to_owned(),
            Some(Tmux { passthrough: true }) => "yes, allow-passthrough on".to_owned(),
            Some(Tmux { passthrough: false }) => "yes, allow-passthrough off".to_owned(),
        };
        let detected = match (self.probed, self.detected) {
            (false, _) => "not asked".to_owned(),
            (true, None | Some(Backend::Text)) => "none".to_owned(),
            (true, Some(backend)) => backend.label().to_owned(),
        };
        let mode = format!("{:?}", self.mode).to_lowercase();
        let mut rows = vec![
            ("Terminal", terminal),
            ("tmux", tmux),
            ("Configured", format!("images = \"{mode}\"")),
            ("Detected", detected),
            ("Drawing with", self.active.label().to_owned()),
        ];
        if let Some((w, h)) = self.cell {
            rows.push(("Cell size", format!("{w}×{h} px")));
        }
        if let Some(reason) = self.reason() {
            rows.push(("Why", reason));
            rows.push(("Fallback", self.fallback().to_owned()));
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).to_owned())
        }
    }

    #[test]
    fn recognizes_emulators_from_their_variables() {
        let cases: &[(&[(&str, &str)], Emulator)] = &[
            (&[("TERM", "xterm-kitty")], Emulator::Kitty),
            (
                &[("KITTY_WINDOW_ID", "1"), ("TERM", "tmux-256color")],
                Emulator::Kitty,
            ),
            (&[("TERM", "foot")], Emulator::Foot),
            (&[("TERM", "foot-extra")], Emulator::Foot),
            (
                &[("TERM", "xterm-256color"), ("ALACRITTY_WINDOW_ID", "9")],
                Emulator::Alacritty,
            ),
            (&[("TERM_PROGRAM", "ghostty")], Emulator::Ghostty),
            (&[("TERM_PROGRAM", "WezTerm")], Emulator::WezTerm),
            (&[("VTE_VERSION", "7600")], Emulator::Vte),
            (&[("TERM", "linux")], Emulator::LinuxConsole),
            (&[("TERM", "xterm-256color")], Emulator::Unknown),
        ];
        for (vars, expected) in cases {
            assert_eq!(emulator_from(env(vars)), *expected, "{vars:?}");
        }
    }

    #[test]
    fn recognizes_emulator_processes() {
        assert_eq!(emulator_named("foot\n"), Some(Emulator::Foot));
        assert_eq!(emulator_named("kitty"), Some(Emulator::Kitty));
        assert_eq!(emulator_named("alacritty"), Some(Emulator::Alacritty));
        assert_eq!(emulator_named("gnome-terminal-"), Some(Emulator::Vte));
        assert_eq!(emulator_named("zsh"), None);
        assert_eq!(emulator_named("tmux: server"), None);
    }

    #[test]
    fn forced_half_blocks_in_a_graphics_terminal_suggest_auto() {
        let kitty = Diagnosis {
            emulator: Emulator::Kitty,
            mode: ImageMode::Halfblock,
            ..Diagnosis::default()
        };
        let why = kitty.reason().unwrap();
        assert!(why.contains("images = \"halfblock\""), "{why}");
        assert!(why.contains("set images = \"auto\""), "{why}");
        assert!(
            kitty
                .native_hint()
                .unwrap()
                .starts_with("kitty can show real images")
        );
        let alacritty = Diagnosis {
            emulator: Emulator::Alacritty,
            mode: ImageMode::Halfblock,
            ..Diagnosis::default()
        };
        assert_eq!(alacritty.native_hint(), None, "nothing better to offer");
        let in_tmux = Diagnosis {
            tmux: Some(Tmux { passthrough: true }),
            ..kitty.clone()
        };
        assert_eq!(in_tmux.native_hint(), None);
        let auto = Diagnosis {
            mode: ImageMode::Auto,
            ..kitty
        };
        assert_eq!(auto.native_hint(), None);
    }

    #[test]
    fn tmux_passthrough_values() {
        assert!(passthrough_enabled("on\n"));
        assert!(passthrough_enabled("all"));
        assert!(!passthrough_enabled("off"));
        assert!(!passthrough_enabled(""));
    }

    #[test]
    fn explains_why_graphics_are_missing() {
        let alacritty = Diagnosis {
            emulator: Emulator::Alacritty,
            probed: true,
            ..Diagnosis::default()
        };
        let why = alacritty.reason().unwrap();
        assert!(why.contains("did not announce"), "{why}");
        assert!(why.contains("neither the Kitty graphics protocol nor Sixel"));
        assert!(alacritty.fallback().contains("half blocks"));

        let tmux = Diagnosis {
            tmux: Some(Tmux { passthrough: false }),
            ..Diagnosis::default()
        };
        assert!(tmux.reason().unwrap().contains("allow-passthrough on"));

        let off = Diagnosis {
            mode: ImageMode::Off,
            ..Diagnosis::default()
        };
        assert_eq!(off.fallback(), "no images");

        let foot = Diagnosis {
            emulator: Emulator::Foot,
            probed: true,
            detected: Some(Backend::Sixel),
            active: Backend::Sixel,
            cell: Some((9, 19)),
            ..Diagnosis::default()
        };
        assert_eq!(foot.reason(), None);
        let rows = foot.rows();
        assert!(rows.contains(&("Drawing with", "sixel".to_owned())));
        assert!(rows.contains(&("Cell size", "9×19 px".to_owned())));
        assert!(!rows.iter().any(|(label, _)| *label == "Why"));
    }
}
