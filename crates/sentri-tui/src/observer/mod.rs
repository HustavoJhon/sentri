//! The Observer: SENTRI's startup ward, drawn with terminal characters.
//!
//! An original design of a watchful ward — an eye between two wings on a
//! staff. Its eye opens as real initialization steps finish, so the picture
//! never claims more progress than there is. Pure functions only: the
//! widget code colors each [`Part`].

/// What a piece of the drawing is, so the UI can color it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Blank,
    Spark,
    Ray,
    Shell,
    Wing,
    Eye,
    Staff,
    Base,
    /// The pupil at the centre of the eye.
    Pupil,
    /// The energy core floating on the staff.
    Core,
    /// The orbiting energy rings.
    Ring,
    /// The radar circle and its sweep on the ground.
    Radar,
    /// Ambient particles drifting around the ward.
    Particle,
}

pub mod model;

/// Cosine of the eye's angular radius on the head: model normals whose
/// `|z|` exceeds it belong to the lens.
pub const EYE_EDGE: f32 = 0.78;

/// How awake the Observer is, from initialization progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Eye {
    Closed,
    Stirring,
    Half,
    Open,
}

impl Eye {
    /// `done` of `total` steps resolved.
    pub const fn from_progress(done: usize, total: usize) -> Self {
        if total == 0 || done >= total {
            Self::Open
        } else if done == 0 {
            Self::Closed
        } else if done * 2 < total {
            Self::Stirring
        } else {
            Self::Half
        }
    }
}

/// How the startup screen is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplashMode {
    /// No startup screen: the dashboard opens directly.
    Off,
    /// One frame redrawn only when progress changes (no timer).
    Static,
    /// Frames advance on a short timer.
    Animated,
}

/// Environment facts that decide the splash mode and glyph set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SplashEnv {
    pub interactive: bool,
    pub term: Option<String>,
    /// `SENTRI_NO_ANIMATION` or `CI` is set.
    pub no_animation: bool,
    /// `LC_ALL`, `LC_CTYPE` or `LANG`, the first one set.
    pub locale: Option<String>,
}

impl SplashEnv {
    pub fn detect() -> Self {
        use std::io::IsTerminal;
        let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
        Self {
            interactive: std::io::stdout().is_terminal() && std::io::stdin().is_terminal(),
            term: std::env::var("TERM").ok(),
            no_animation: set("SENTRI_NO_ANIMATION") || set("CI"),
            locale: ["LC_ALL", "LC_CTYPE", "LANG"]
                .iter()
                .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty())),
        }
    }

    fn dumb(&self) -> bool {
        self.term.as_deref().is_none_or(|t| t == "dumb")
    }

    /// Choose the mode for a configuration that has the animation `enabled`.
    pub fn mode(&self, enabled: bool) -> SplashMode {
        if !enabled || !self.interactive {
            SplashMode::Off
        } else if self.no_animation || self.dumb() {
            SplashMode::Static
        } else {
            SplashMode::Animated
        }
    }

    /// Whether box-drawing and block characters can be trusted.
    pub fn unicode(&self, ascii_icons: bool) -> bool {
        let utf8 = self.locale.as_deref().is_none_or(|l| {
            let l = l.to_ascii_lowercase();
            l.contains("utf-8") || l.contains("utf8")
        });
        !ascii_icons && utf8 && !self.dumb() && self.term.as_deref() != Some("linux")
    }
}

/// One drawn line: segments of text with their part.
pub type ArtLine = Vec<(Part, String)>;

/// Width of every line of the drawing, in columns.
pub const WIDTH: usize = 25;
/// Height of the drawing, in rows.
pub const HEIGHT: usize = 11;

/// The three-column eye for this state and animation tick.
fn eye(state: Eye, tick: u64, unicode: bool) -> &'static str {
    // While awake the pupil watches center, left, center, right.
    let look = match (tick / 6) % 4 {
        1 => 0,
        3 => 2,
        _ => 1,
    };
    match (state, unicode) {
        (Eye::Closed, true) => "───",
        (Eye::Stirring, true) => "─◡─",
        (Eye::Half, true) => " ◒ ",
        (Eye::Open, true) => ["◉  ", " ◉ ", "  ◉"][look],
        (Eye::Closed, false) => "---",
        (Eye::Stirring, false) => "-.-",
        (Eye::Half, false) => " o ",
        (Eye::Open, false) => ["@  ", " @ ", "  @"][look],
    }
}

/// The full drawing for an eye state and animation tick.
pub fn frame(state: Eye, tick: u64, unicode: bool) -> Vec<ArtLine> {
    use Part::{Base, Blank, Eye as E, Ray, Shell, Spark, Staff, Wing};
    let seg = |part: Part, text: &str| (part, text.to_owned());
    let awake = state >= Eye::Half;
    let spark = match (state, unicode, tick % 8 < 4) {
        (Eye::Open, true, true) => "✦",
        (Eye::Open, true, false) => "✧",
        (Eye::Open, false, _) => "*",
        _ => " ",
    };
    let g = |u: &'static str, a: &'static str| if unicode { u } else { a };
    let rays = if awake {
        g("╲  │  ╱", "\\  |  /")
    } else {
        "       "
    };
    let lines: Vec<ArtLine> = vec![
        vec![seg(Blank, "            "), seg(Spark, spark)],
        vec![seg(Blank, "         "), seg(Ray, rays)],
        vec![
            seg(Blank, "        "),
            seg(Shell, g("╭───────╮", ".-------.")),
        ],
        vec![
            seg(Blank, "   "),
            seg(Wing, g("◢◤", "/\\")),
            seg(Blank, "  "),
            seg(Shell, g("╱  ╭───╮  ╲", "/  .---.  \\")),
            seg(Blank, "  "),
            seg(Wing, g("◥◣", "/\\")),
        ],
        vec![
            seg(Blank, "  "),
            seg(Wing, g("◢██──", "<##--")),
            seg(Shell, g("┤  │", "|  |")),
            seg(E, eye(state, tick, unicode)),
            seg(Shell, g("│  ├", "|  |")),
            seg(Wing, g("──██◣", "--##>")),
        ],
        vec![
            seg(Blank, "   "),
            seg(Wing, g("◥◣", "\\/")),
            seg(Blank, "  "),
            seg(Shell, g("╲  ╰───╯  ╱", "\\  '---'  /")),
            seg(Blank, "  "),
            seg(Wing, g("◢◤", "\\/")),
        ],
        vec![
            seg(Blank, "        "),
            seg(Shell, g("╰───┬───╯", "'---+---'")),
        ],
        vec![seg(Blank, "            "), seg(Staff, g("┃", "|"))],
        vec![seg(Blank, "            "), seg(Staff, g("┃", "|"))],
        vec![seg(Blank, "          "), seg(Staff, g("╱ ┃ ╲", "/ | \\"))],
        vec![
            seg(Blank, "      "),
            seg(Base, g("▀▀▀▀▀▀▀▀▀▀▀▀▀", "=============")),
        ],
    ];
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    fn width(line: &ArtLine) -> usize {
        line.iter().map(|(_, t)| t.width()).sum()
    }

    #[test]
    fn every_frame_is_symmetric_and_fits() {
        for unicode in [true, false] {
            for state in [Eye::Closed, Eye::Stirring, Eye::Half, Eye::Open] {
                for tick in 0..24 {
                    let lines = frame(state, tick, unicode);
                    assert_eq!(lines.len(), HEIGHT);
                    for line in &lines {
                        assert!(width(line) <= WIDTH, "{line:?}");
                    }
                    // The eye row is centered on column 12.
                    assert_eq!(width(&lines[4]), 23);
                }
            }
        }
    }

    #[test]
    fn ascii_frames_use_only_ascii() {
        for state in [Eye::Closed, Eye::Open] {
            for line in frame(state, 3, false) {
                for (_, text) in line {
                    assert!(text.is_ascii(), "{text}");
                }
            }
        }
    }

    #[test]
    fn eye_opens_with_progress_only() {
        assert_eq!(Eye::from_progress(0, 5), Eye::Closed);
        assert_eq!(Eye::from_progress(2, 5), Eye::Stirring);
        assert_eq!(Eye::from_progress(3, 5), Eye::Half);
        assert_eq!(Eye::from_progress(4, 5), Eye::Half);
        assert_eq!(Eye::from_progress(5, 5), Eye::Open);
        let closed = frame(Eye::Closed, 0, true);
        assert!(
            closed[1].iter().all(|(_, t)| t.trim().is_empty()),
            "no rays yet"
        );
    }

    #[test]
    fn mode_respects_config_and_environment() {
        let tty = SplashEnv {
            interactive: true,
            term: Some("xterm-kitty".into()),
            locale: Some("en_US.UTF-8".into()),
            no_animation: false,
        };
        assert_eq!(tty.mode(true), SplashMode::Animated);
        assert_eq!(tty.mode(false), SplashMode::Off, "disabled in config");
        assert!(tty.unicode(false));
        assert!(!tty.unicode(true), "ascii_icons forces ASCII");

        let piped = SplashEnv {
            interactive: false,
            ..tty.clone()
        };
        assert_eq!(piped.mode(true), SplashMode::Off);

        let ci = SplashEnv {
            no_animation: true,
            ..tty.clone()
        };
        assert_eq!(ci.mode(true), SplashMode::Static);

        let console = SplashEnv {
            term: Some("linux".into()),
            ..tty.clone()
        };
        assert_eq!(console.mode(true), SplashMode::Animated);
        assert!(
            !console.unicode(false),
            "the Linux console lacks the glyphs"
        );

        let dumb = SplashEnv {
            term: Some("dumb".into()),
            ..tty.clone()
        };
        assert_eq!(dumb.mode(true), SplashMode::Static);
        let latin1 = SplashEnv {
            locale: Some("C".into()),
            ..tty
        };
        assert!(!latin1.unicode(false));
    }
}
