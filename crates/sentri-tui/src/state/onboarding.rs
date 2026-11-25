//! First-run onboarding: GitHub, location, discovery scope and interests.
//!
//! Every Continue saves that page's answers (validated, through the normal
//! settings writer) together with the step to resume from, so quitting
//! halfway loses nothing. Esc goes back a page and never writes.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use sentri_app::{Config, Lookup};
use sentri_core::{Modality, Topic, country};

use crate::input::LineInput;

/// Wizard pages, in order. The index is what `onboarding.step` stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Welcome,
    GitHub,
    Location,
    Discovery,
    Interests,
    Review,
}

impl Page {
    pub const ALL: [Self; 6] = [
        Self::Welcome,
        Self::GitHub,
        Self::Location,
        Self::Discovery,
        Self::Interests,
        Self::Review,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Self::Welcome => "Welcome",
            Self::GitHub => "GitHub profile",
            Self::Location => "Location",
            Self::Discovery => "Where to look",
            Self::Interests => "Interests",
            Self::Review => "Review",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }
}

/// Location fields, in the order they are edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Country,
    City,
    Region,
    Timezone,
}

impl Field {
    pub const ALL: [Self; 4] = [Self::Country, Self::City, Self::Region, Self::Timezone];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Country => "Country code",
            Self::City => "City",
            Self::Region => "Region / state",
            Self::Timezone => "Time zone",
        }
    }
}

/// Discovery toggles, in the order they are shown.
pub const SCOPES: [&str; 6] = [
    "Local opportunities",
    "National opportunities",
    "International opportunities",
    "Remote opportunities",
    "Hybrid opportunities",
    "In-person opportunities",
];

/// What the wizard knows about the typed GitHub username.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GitHubCheck {
    #[default]
    Unchecked,
    Checking(String),
    Checked(String, Lookup),
}

/// What the wizard asks the runtime to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Write these settings; `finish` closes the wizard once saved.
    Save {
        changes: Vec<(String, String)>,
        backup: bool,
        finish: bool,
    },
    CheckGitHub(String),
    /// Close for this session without writing (it opens again next start).
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Onboarding {
    pub page: Page,
    pub github: LineInput,
    pub check: GitHubCheck,
    pub fields: [LineInput; 4],
    pub field: usize,
    /// local, national, international, remote, hybrid, in person.
    pub scopes: [bool; 6],
    pub scope_cursor: usize,
    pub topics: [bool; 10],
    pub topic_cursor: usize,
    pub error: Option<String>,
    /// A backup is taken before the first write only.
    pub backed_up: bool,
    /// Waiting for the final save.
    pub finishing: bool,
}

fn opt(value: Option<&String>) -> String {
    value.map(|v| v.trim().to_owned()).unwrap_or_default()
}

impl Onboarding {
    /// Starts (or resumes) from what the configuration already holds.
    /// `detected_zone` is the system time zone, offered as the default.
    pub fn from_config(config: &Config, detected_zone: Option<String>) -> Self {
        let p = &config.profile;
        let code = p
            .country_code
            .as_deref()
            .and_then(country::normalize_code)
            .or_else(|| p.country.as_deref().and_then(country::code_for_name))
            .unwrap_or_default();
        let zone = p
            .timezone
            .clone()
            .filter(|z| !z.is_empty())
            .or(detected_zone)
            .unwrap_or_default();
        let d = &config.discovery;
        let modal = |m: Modality| p.preferred_modalities.contains(&m);
        let mut topics = [false; 10];
        for (i, topic) in Topic::ALL.iter().enumerate() {
            topics[i] = d.topics.contains(topic);
        }
        let step = usize::from(config.onboarding.step.unwrap_or(0)).min(Page::ALL.len() - 1);
        Self {
            page: Page::ALL[step],
            github: LineInput::with_value(&opt(config.github.username.as_ref())),
            check: GitHubCheck::Unchecked,
            fields: [
                LineInput::with_value(code),
                LineInput::with_value(&opt(p.home_city.as_ref())),
                LineInput::with_value(&opt(p.region.as_ref())),
                LineInput::with_value(&zone),
            ],
            field: 0,
            scopes: [
                d.local,
                d.national,
                d.international,
                modal(Modality::Online),
                modal(Modality::Hybrid),
                modal(Modality::InPerson),
            ],
            scope_cursor: 0,
            topics,
            topic_cursor: 0,
            error: None,
            backed_up: false,
            finishing: false,
        }
    }

    pub fn step(&self) -> usize {
        self.page.index()
    }

    pub fn value(&self, field: Field) -> String {
        let i = Field::ALL.iter().position(|f| *f == field).unwrap_or(0);
        self.fields[i].value().trim().to_owned()
    }

    /// The country name for the typed code, or why it is not valid.
    pub fn country_name(&self) -> Result<Option<&'static str>, String> {
        let code = self.value(Field::Country);
        if code.is_empty() {
            return Ok(None);
        }
        country::name(&code)
            .map(Some)
            .ok_or_else(|| format!("{code:?} is not an ISO country code (PE, MX, ES, US…)"))
    }

    pub fn chosen_topics(&self) -> Vec<Topic> {
        Topic::ALL
            .iter()
            .zip(self.topics)
            .filter_map(|(t, on)| on.then_some(*t))
            .collect()
    }

    pub fn modalities(&self) -> Vec<&'static str> {
        [("online", 3), ("hybrid", 4), ("in_person", 5)]
            .iter()
            .filter(|(_, i)| self.scopes[*i])
            .map(|(name, _)| *name)
            .collect()
    }

    /// Settings written when leaving `page` forward.
    fn changes(&self, page: Page) -> Result<Vec<(String, String)>, String> {
        let pair = |k: &str, v: String| (k.to_owned(), v);
        Ok(match page {
            Page::Welcome | Page::Review => Vec::new(),
            Page::GitHub => {
                let name = self.github.value().trim().to_owned();
                if !name.is_empty() && !sentri_app::profile::valid_username(&name) {
                    return Err(format!(
                        "{name:?} is not a GitHub username (letters, digits and single hyphens)"
                    ));
                }
                vec![pair("github.username", name)]
            }
            Page::Location => {
                let country = self.country_name()?;
                let zone = self.value(Field::Timezone);
                if !zone.is_empty() && !sentri_app::timezone::valid(&zone) {
                    return Err(format!(
                        "{zone:?} is not a time zone name like America/Lima"
                    ));
                }
                for field in [Field::City, Field::Region] {
                    if self.value(field).chars().count() > 80 {
                        return Err(format!("{} is longer than 80 characters", field.label()));
                    }
                }
                vec![
                    pair(
                        "profile.country_code",
                        self.value(Field::Country).to_ascii_uppercase(),
                    ),
                    pair("profile.country", country.unwrap_or_default().to_owned()),
                    pair("profile.home_city", self.value(Field::City)),
                    pair("profile.region", self.value(Field::Region)),
                    pair("profile.timezone", zone),
                ]
            }
            Page::Discovery => vec![
                pair("discovery.local", self.scopes[0].to_string()),
                pair("discovery.national", self.scopes[1].to_string()),
                pair("discovery.international", self.scopes[2].to_string()),
                pair("profile.preferred_modalities", self.modalities().join(", ")),
            ],
            Page::Interests => vec![pair(
                "discovery.topics",
                self.chosen_topics()
                    .iter()
                    .map(|t| t.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            )],
        })
    }

    fn save(&mut self, mut changes: Vec<(String, String)>, finish: bool) -> Request {
        if finish {
            changes.push(("onboarding.completed".into(), "true".into()));
            changes.push(("onboarding.step".into(), String::new()));
        } else {
            changes.push(("onboarding.step".into(), self.step().to_string()));
        }
        let backup = !self.backed_up;
        self.backed_up = true;
        Request::Save {
            changes,
            backup,
            finish,
        }
    }

    /// Validates and saves the current page, then moves forward.
    fn advance(&mut self) -> Vec<Request> {
        let changes = match self.changes(self.page) {
            Ok(changes) => changes,
            Err(e) => {
                self.error = Some(e);
                return Vec::new();
            }
        };
        self.error = None;
        if self.page == Page::Review {
            self.finishing = true;
            return vec![self.save(Vec::new(), true)];
        }
        let mut requests = Vec::new();
        if self.page == Page::GitHub {
            let name = self.github.value().trim().to_owned();
            let known = match &self.check {
                GitHubCheck::Checking(n) | GitHubCheck::Checked(n, _) => n == &name,
                GitHubCheck::Unchecked => false,
            };
            if !name.is_empty() && !known {
                // Checked in the background; the answer shows on Review.
                self.check = GitHubCheck::Checking(name.clone());
                requests.push(Request::CheckGitHub(name));
            } else if name.is_empty() {
                self.check = GitHubCheck::Unchecked;
            }
        }
        self.page = Page::ALL[(self.step() + 1).min(Page::ALL.len() - 1)];
        requests.insert(0, self.save(changes, false));
        requests
    }

    fn back(&mut self) -> Vec<Request> {
        self.error = None;
        if self.page == Page::Welcome {
            return vec![Request::Close];
        }
        self.page = Page::ALL[self.step() - 1];
        Vec::new()
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Vec<Request> {
        if self.finishing {
            return Vec::new();
        }
        match key.code {
            KeyCode::Enter => return self.advance(),
            KeyCode::Esc => return self.back(),
            _ => {}
        }
        match self.page {
            Page::Welcome | Page::Review => {}
            Page::GitHub => {
                if self.github.handle(key) {
                    self.error = None;
                }
            }
            Page::Location => self.location_key(key),
            Page::Discovery => toggle_key(key, &mut self.scopes, &mut self.scope_cursor),
            Page::Interests => toggle_key(key, &mut self.topics, &mut self.topic_cursor),
        }
        Vec::new()
    }

    fn location_key(&mut self, key: KeyEvent) {
        let last = Field::ALL.len() - 1;
        let back_tab = key.code == KeyCode::BackTab
            || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT));
        match key.code {
            _ if back_tab => self.field = self.field.saturating_sub(1),
            KeyCode::Up => self.field = self.field.saturating_sub(1),
            KeyCode::Tab | KeyCode::Down => self.field = (self.field + 1).min(last),
            KeyCode::Left | KeyCode::Right if Field::ALL[self.field] == Field::Timezone => {
                self.cycle_zone(key.code == KeyCode::Right);
            }
            _ => {
                if self.fields[self.field].handle(key) {
                    self.error = None;
                }
            }
        }
    }

    /// Steps through the suggested zones, starting from the typed one.
    fn cycle_zone(&mut self, forward: bool) {
        let list = sentri_app::timezone::SUGGESTIONS;
        let current = self.value(Field::Timezone);
        let next = match list.iter().position(|z| *z == current) {
            Some(i) if forward => (i + 1) % list.len(),
            Some(i) => (i + list.len() - 1) % list.len(),
            None => 0,
        };
        self.fields[3] = LineInput::with_value(list[next]);
    }

    /// The final save failed: let the user try again.
    pub const fn save_failed(&mut self) {
        self.finishing = false;
    }
}

fn toggle_key<const N: usize>(key: KeyEvent, values: &mut [bool; N], cursor: &mut usize) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => *cursor = cursor.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => *cursor = (*cursor + 1).min(N - 1),
        KeyCode::Char(' ' | 'x') => values[*cursor] = !values[*cursor],
        KeyCode::Char('a') => {
            let all = values.iter().all(|v| *v);
            for v in values.iter_mut() {
                *v = !all;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn typed(w: &mut Onboarding, text: &str) {
        for c in text.chars() {
            w.on_key(key(KeyCode::Char(c)));
        }
    }

    fn clear(w: &mut Onboarding) {
        w.on_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
        w.on_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    }

    fn fresh() -> Onboarding {
        let config = Config::parse("", Path::new("x")).unwrap();
        Onboarding::from_config(&config, Some("America/Lima".into()))
    }

    fn saved(requests: &[Request]) -> Vec<(String, String)> {
        match requests.first() {
            Some(Request::Save { changes, .. }) => changes.clone(),
            other => panic!("expected a save, got {other:?}"),
        }
    }

    fn get<'a>(changes: &'a [(String, String)], key: &str) -> &'a str {
        changes
            .iter()
            .find(|(k, _)| k == key)
            .map_or("<missing>", |(_, v)| v.as_str())
    }

    #[test]
    fn walks_every_page_saving_each_step() {
        let mut w = fresh();
        assert_eq!(w.page, Page::Welcome);
        let r = w.on_key(key(KeyCode::Enter));
        assert_eq!(get(&saved(&r), "onboarding.step"), "1");
        assert!(
            matches!(r[0], Request::Save { backup: true, .. }),
            "first write backs up"
        );

        typed(&mut w, "octocat");
        let r = w.on_key(key(KeyCode::Enter));
        assert_eq!(get(&saved(&r), "github.username"), "octocat");
        assert!(matches!(r[0], Request::Save { backup: false, .. }));
        assert_eq!(r[1], Request::CheckGitHub("octocat".into()));
        assert_eq!(w.page, Page::Location);

        typed(&mut w, "mx");
        w.on_key(key(KeyCode::Down));
        typed(&mut w, "Guadalajara");
        w.on_key(key(KeyCode::Tab));
        typed(&mut w, "Jalisco");
        let r = w.on_key(key(KeyCode::Enter));
        let c = saved(&r);
        assert_eq!(get(&c, "profile.country_code"), "MX");
        assert_eq!(get(&c, "profile.country"), "Mexico");
        assert_eq!(get(&c, "profile.home_city"), "Guadalajara");
        assert_eq!(get(&c, "profile.region"), "Jalisco");
        assert_eq!(
            get(&c, "profile.timezone"),
            "America/Lima",
            "detected default"
        );
        assert_eq!(get(&c, "onboarding.step"), "3");

        // International off, in person on.
        w.on_key(key(KeyCode::Down));
        w.on_key(key(KeyCode::Down));
        w.on_key(key(KeyCode::Char(' ')));
        for _ in 0..3 {
            w.on_key(key(KeyCode::Down));
        }
        w.on_key(key(KeyCode::Char(' ')));
        let c = saved(&w.on_key(key(KeyCode::Enter)));
        assert_eq!(get(&c, "discovery.international"), "false");
        assert_eq!(get(&c, "discovery.local"), "true");
        assert_eq!(
            get(&c, "profile.preferred_modalities"),
            "in_person",
            "none were set before"
        );

        w.on_key(key(KeyCode::Char(' '))); // events
        w.on_key(key(KeyCode::Down));
        w.on_key(key(KeyCode::Down));
        w.on_key(key(KeyCode::Char('x'))); // hackathons
        let c = saved(&w.on_key(key(KeyCode::Enter)));
        assert_eq!(get(&c, "discovery.topics"), "events, hackathons");
        assert_eq!(w.page, Page::Review);

        let r = w.on_key(key(KeyCode::Enter));
        assert!(matches!(r[0], Request::Save { finish: true, .. }));
        let c = saved(&r);
        assert_eq!(get(&c, "onboarding.completed"), "true");
        assert_eq!(get(&c, "onboarding.step"), "", "removed when done");
        assert!(w.on_key(key(KeyCode::Enter)).is_empty(), "no double finish");
        w.save_failed();
        assert!(
            !w.on_key(key(KeyCode::Enter)).is_empty(),
            "retry after a failure"
        );
    }

    #[test]
    fn optional_fields_can_stay_empty() {
        let config = Config::parse("[profile]\nhome_city = \"Lima\"\n", Path::new("x")).unwrap();
        let mut w = Onboarding::from_config(&config, None);
        w.page = Page::GitHub;
        let r = w.on_key(key(KeyCode::Enter));
        assert_eq!(get(&saved(&r), "github.username"), "");
        assert_eq!(r.len(), 1, "nothing to check");
        let c = saved(&w.on_key(key(KeyCode::Enter)));
        assert_eq!(get(&c, "profile.country_code"), "");
        assert_eq!(get(&c, "profile.region"), "");
        assert_eq!(get(&c, "profile.timezone"), "");
        assert_eq!(get(&c, "profile.home_city"), "Lima");
    }

    #[test]
    fn invalid_answers_stay_on_the_page_without_writing() {
        let mut w = fresh();
        w.page = Page::GitHub;
        typed(&mut w, "not valid!");
        assert_eq!(w.on_key(key(KeyCode::Enter)), Vec::<Request>::new());
        assert_eq!(w.page, Page::GitHub);
        assert!(
            w.error
                .as_deref()
                .unwrap()
                .contains("not a GitHub username")
        );
        typed(&mut w, "x");
        assert_eq!(w.error, None, "editing clears the error");

        w.page = Page::Location;
        w.field = 0;
        clear(&mut w);
        typed(&mut w, "PER");
        assert_eq!(w.on_key(key(KeyCode::Enter)), Vec::<Request>::new());
        assert!(w.error.as_deref().unwrap().contains("ISO country code"));
        clear(&mut w);
        typed(&mut w, "pe");
        w.field = 3;
        clear(&mut w);
        typed(&mut w, "Lima");
        assert_eq!(w.on_key(key(KeyCode::Enter)), Vec::<Request>::new());
        assert!(w.error.as_deref().unwrap().contains("time zone"));
        w.on_key(key(KeyCode::Right));
        assert_eq!(w.value(Field::Timezone), "America/Los_Angeles");
        w.on_key(key(KeyCode::Left));
        assert_eq!(w.value(Field::Timezone), "Australia/Sydney");
        assert_ne!(w.on_key(key(KeyCode::Enter)), Vec::<Request>::new());
    }

    #[test]
    fn esc_goes_back_and_never_writes() {
        let mut w = fresh();
        w.page = Page::Interests;
        assert_eq!(w.on_key(key(KeyCode::Esc)), Vec::<Request>::new());
        assert_eq!(w.page, Page::Discovery);
        w.page = Page::Welcome;
        assert_eq!(w.on_key(key(KeyCode::Esc)), vec![Request::Close]);
    }

    #[test]
    fn resumes_an_interrupted_onboarding_with_saved_answers() {
        let text = "[onboarding]\ncompleted = false\nstep = 3\n\
            [github]\nusername = \"octocat\"\n\
            [profile]\ncountry = \"Perú\"\nhome_city = \"Cusco\"\npreferred_modalities = [\"hybrid\"]\n\
            [discovery]\nnational = false\ntopics = [\"jobs\"]\n";
        let config = Config::parse(text, Path::new("x")).unwrap();
        let w = Onboarding::from_config(&config, Some("Europe/Madrid".into()));
        assert_eq!(w.page, Page::Discovery);
        assert_eq!(w.github.value(), "octocat");
        assert_eq!(
            w.value(Field::Country),
            "PE",
            "derived from the country name"
        );
        assert_eq!(w.value(Field::Timezone), "Europe/Madrid");
        assert_eq!(w.scopes, [true, false, true, false, true, false]);
        assert_eq!(w.chosen_topics(), [Topic::Jobs]);
    }

    #[test]
    fn select_all_toggles_every_interest() {
        let mut w = fresh();
        w.page = Page::Interests;
        w.on_key(key(KeyCode::Char('a')));
        assert_eq!(w.chosen_topics().len(), Topic::ALL.len());
        w.on_key(key(KeyCode::Char('a')));
        assert_eq!(w.chosen_topics(), Vec::<Topic>::new());
    }

    mod flow {
        use super::*;
        use crate::state::{Effect, Msg, State};

        fn state_with_wizard() -> State {
            State {
                onboarding: Some(fresh()),
                ..State::default()
            }
        }

        fn saved_config(text: &str) -> Box<sentri_app::settings::Saved> {
            Box::new(sentri_app::settings::Saved {
                config: Config::parse(text, Path::new("x")).unwrap(),
                backup: None,
            })
        }

        #[test]
        fn wizard_takes_the_keys_and_finishing_opens_the_dashboard() {
            let mut state = state_with_wizard();
            let effects = state.on_key(key(KeyCode::Enter));
            assert!(matches!(
                effects.as_slice(),
                [Effect::SaveOnboarding {
                    finish: false,
                    backup: true,
                    ..
                }]
            ));
            // `q` is text here, not quit.
            state.on_key(key(KeyCode::Char('q')));
            assert!(!state.quit);
            assert_eq!(state.onboarding.as_ref().unwrap().github.value(), "q");

            let partial = state.on_msg(Msg::OnboardingSaved(saved_config(""), false));
            assert_eq!(partial, Vec::new());
            assert!(state.onboarding.is_some());

            let done = state.on_msg(Msg::OnboardingSaved(
                saved_config("[discovery]\ntopics = [\"jobs\"]\n"),
                true,
            ));
            assert_eq!(done, vec![Effect::Load, Effect::LoadProfile]);
            assert!(state.onboarding.is_none());
            assert_eq!(
                state.overview.config.discovery.topics,
                [Topic::Jobs],
                "settings reflect the new profile"
            );
        }

        #[test]
        fn esc_on_welcome_skips_without_writing_and_failures_show_inline() {
            let mut state = state_with_wizard();
            state.onboarding.as_mut().unwrap().page = Page::Review;
            state.on_key(key(KeyCode::Enter));
            state.on_msg(Msg::OnboardingFailed("Setup not saved: disk full".into()));
            let w = state.onboarding.as_ref().unwrap();
            assert_eq!(w.error.as_deref(), Some("Setup not saved: disk full"));
            assert!(!w.finishing, "can retry");

            let mut state = state_with_wizard();
            assert_eq!(state.on_key(key(KeyCode::Esc)), Vec::new());
            assert!(state.onboarding.is_none());
        }

        #[test]
        fn settings_can_rerun_onboarding_with_saved_answers() {
            let mut state = State::default();
            state.section = crate::state::Section::ALL
                .iter()
                .position(|s| *s == crate::state::Section::Settings)
                .unwrap();
            assert_eq!(
                state.on_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::SHIFT)),
                vec![Effect::RestartOnboarding]
            );
            state.on_msg(Msg::OnboardingRestarted(
                saved_config(
                    "[onboarding]\ncompleted = false\nstep = 0\n[github]\nusername = \"octocat\"\n",
                ),
                None,
            ));
            let w = state.onboarding.as_ref().unwrap();
            assert_eq!(w.page, Page::Welcome);
            assert_eq!(w.github.value(), "octocat");
        }
    }
}
