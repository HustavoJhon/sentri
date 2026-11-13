//! Keyword heuristics that fill *unknown* fields from explicit mentions in
//! the text (English and Spanish).
//!
//! The rules only fire on explicit phrases ("fully funded", "gratis",
//! "certificado"…) and never overwrite a value a source already provided.
//! They complement structured data; they do not replace reading the offer.

use crate::profile::contains_term;
use crate::{Category, Cost, Modality, Opportunity, Stance};

/// Ordered from most to least specific: the first match wins.
const CATEGORY_RULES: &[(Category, &[&str])] = &[
    (
        Category::Hackathon,
        &["hackathon", "hackatón", "hackaton", "game jam", "datathon"],
    ),
    (
        Category::Internship,
        &[
            "internship",
            "intern",
            "prácticas",
            "practicas",
            "pasantía",
            "pasantia",
            "practicante",
        ],
    ),
    (
        Category::Scholarship,
        &["scholarship", "beca", "becas", "grant for students"],
    ),
    (Category::Fellowship, &["fellowship", "fellows program"]),
    (
        Category::Exchange,
        &[
            "exchange program",
            "intercambio",
            "study abroad",
            "movilidad estudiantil",
        ],
    ),
    (
        Category::Volunteering,
        &[
            "volunteer",
            "volunteering",
            "voluntariado",
            "voluntario",
            "voluntarios",
        ],
    ),
    (
        Category::Competition,
        &[
            "competition",
            "contest",
            "challenge",
            "concurso",
            "competencia",
            "olimpiada",
        ],
    ),
    (
        Category::Conference,
        &[
            "conference",
            "summit",
            "congreso",
            "conferencia",
            "devfest",
            "symposium",
        ],
    ),
    (
        Category::Workshop,
        &["workshop", "taller", "hands-on lab", "codelab"],
    ),
    (
        Category::Meetup,
        &["meetup", "community meeting", "networking night"],
    ),
    (
        Category::Training,
        &[
            "bootcamp",
            "training program",
            "programa de formación",
            "academy",
        ],
    ),
    (
        Category::Course,
        &[
            "course",
            "curso",
            "mooc",
            "certification path",
            "learning path",
            "diplomado",
        ],
    ),
    (
        Category::Call,
        &[
            "call for",
            "convocatoria",
            "applications open",
            "cfp",
            "postula",
        ],
    ),
    (
        Category::Job,
        &[
            "job",
            "hiring",
            "we're hiring",
            "vacancy",
            "vacante",
            "empleo",
            "developer position",
            "engineer position",
        ],
    ),
    (
        Category::OpenSource,
        &[
            "good first issue",
            "open source project",
            "contributors wanted",
            "help wanted",
        ],
    ),
    (
        Category::Event,
        &["webinar", "event", "evento", "livestream", "meetup"],
    ),
];

const ONLINE: &[&str] = &[
    "online",
    "virtual",
    "remote",
    "remoto",
    "webinar",
    "en línea",
    "en linea",
];
const IN_PERSON: &[&str] = &["in-person", "in person", "on-site", "onsite", "presencial"];
const HYBRID: &[&str] = &["hybrid", "híbrido", "hibrido"];
const FREE: &[&str] = &[
    "free of charge",
    "free registration",
    "gratis",
    "gratuito",
    "gratuita",
    "sin costo",
    "no cost",
];
const CERTIFICATE: &[&str] = &[
    "certificate",
    "certificado",
    "certification",
    "certificación",
    "constancia",
];
const TRAVEL: &[&str] = &[
    "travel grant",
    "travel support",
    "flights covered",
    "airfare",
    "pasajes",
    "fully funded",
    "totalmente financiada",
    "travel expenses",
];
const ACCOMMODATION: &[&str] = &[
    "accommodation",
    "housing provided",
    "alojamiento",
    "hospedaje",
    "lodging",
];
const PRIZES: &[&str] = &["prize", "prizes", "premio", "premios", "award"];

/// Wording that signals unconfirmed information.
const SPECULATION: &[&str] = &[
    "rumor",
    "rumors",
    "rumour",
    "rumoured",
    "rumored",
    "reportedly",
    "leak",
    "leaked",
    "leaks",
    "allegedly",
    "sources say",
    "sources familiar",
    "people familiar",
    "could be",
    "may be",
    "is said to",
    "según fuentes",
    "rumores",
    "filtración",
    "supuestamente",
];
const OPINION: &[&str] = &[
    "opinion",
    "op-ed",
    "editorial",
    "commentary",
    "column",
    "analysis:",
    "why i",
    "i think",
    "hot take",
    "opinión",
    "columna",
];
const DISCUSSION: &[&str] = &["ask hn", "show hn", "tell hn", "discussion", "thread"];

/// News topics, first match wins.
const TOPICS: &[(&str, &[&str])] = &[
    (
        "security",
        &[
            "vulnerability",
            "cve",
            "exploit",
            "breach",
            "ransomware",
            "malware",
            "security",
            "zero-day",
        ],
    ),
    (
        "ai",
        &[
            "ai",
            "llm",
            "gpt",
            "openai",
            "anthropic",
            "gemini",
            "machine learning",
            "neural",
            "model",
            "chatbot",
            "inteligencia artificial",
        ],
    ),
    (
        "hardware",
        &[
            "gpu",
            "gpus",
            "cpus",
            "processor",
            "chipmaker",
            "cpu",
            "chip",
            "chips",
            "semiconductor",
            "nvidia",
            "amd",
            "intel",
            "tsmc",
            "arm",
        ],
    ),
    (
        "linux",
        &[
            "linux",
            "kernel",
            "distro",
            "ubuntu",
            "fedora",
            "arch linux",
            "debian",
            "gnome",
            "kde",
            "wayland",
        ],
    ),
    ("rust", &["rust", "cargo", "crates.io"]),
    ("go", &["golang", "go 1."]),
    (
        "open source",
        &["open source", "open-source", "foss", "license", "github"],
    ),
    (
        "business",
        &[
            "acquisition",
            "acquires",
            "layoffs",
            "funding",
            "raises",
            "ipo",
            "valuation",
            "revenue",
            "antitrust",
            "lawsuit",
            "ceo",
            "startup",
        ],
    ),
    (
        "dev tools",
        &[
            "compiler",
            "ide",
            "framework",
            "database",
            "developer",
            "programming",
            "api",
        ],
    ),
];

/// Companies and people worth tagging when mentioned.
const ENTITIES: &[(&str, &[&str])] = &[
    ("nvidia", &["nvidia", "jensen huang"]),
    ("amd", &["amd", "lisa su"]),
    ("intel", &["intel"]),
    ("openai", &["openai", "sam altman", "chatgpt"]),
    ("anthropic", &["anthropic", "claude", "dario amodei"]),
    (
        "google",
        &["google", "alphabet", "deepmind", "sundar pichai", "gemini"],
    ),
    (
        "microsoft",
        &["microsoft", "satya nadella", "azure", "copilot"],
    ),
    ("apple", &["apple", "tim cook", "iphone", "macos"]),
    ("meta", &["meta", "facebook", "mark zuckerberg", "llama"]),
    ("amazon", &["amazon", "aws", "andy jassy"]),
    ("tesla", &["tesla", "elon musk", "xai"]),
    ("github", &["github"]),
    ("linux foundation", &["linux foundation", "linus torvalds"]),
    ("mistral", &["mistral ai"]),
];

fn any(haystack: &str, terms: &[&str]) -> bool {
    terms.iter().any(|t| contains_term(haystack, t))
}

/// Infers a category from text, if any rule matches.
pub fn category(text: &str) -> Option<Category> {
    let text = text.to_lowercase();
    CATEGORY_RULES
        .iter()
        .find(|(_, terms)| any(&text, terms))
        .map(|(category, _)| *category)
}

/// How a news item presents itself. Falls back to `fallback` (e.g. the
/// source's configured stance) and finally to a plain report.
pub fn stance(text: &str, fallback: Option<Stance>) -> Stance {
    let text = text.to_lowercase();
    if any(&text, SPECULATION) || text.contains('?') && text.contains("could") {
        Stance::Speculation
    } else if any(&text, DISCUSSION) {
        Stance::Discussion
    } else if any(&text, OPINION) {
        Stance::Opinion
    } else {
        fallback.unwrap_or(Stance::Report)
    }
}

/// Main topic of a news item.
pub fn news_topic(text: &str) -> Option<&'static str> {
    let text = text.to_lowercase();
    TOPICS
        .iter()
        .find(|(_, terms)| any(&text, terms))
        .map(|(topic, _)| *topic)
}

/// Companies and people mentioned in the text.
pub fn entities(text: &str) -> Vec<&'static str> {
    let text = text.to_lowercase();
    ENTITIES
        .iter()
        .filter(|(_, terms)| any(&text, terms))
        .map(|(name, _)| *name)
        .collect()
}

/// Marks `o` as news and fills stance, topic and entity tags. The topic is
/// placed first among the tags.
pub fn enrich_news(o: &mut Opportunity, fallback: Option<Stance>) {
    let text = format!("{} {}", o.title, o.summary.as_deref().unwrap_or_default());
    o.content = crate::ContentKind::News;
    if o.stance.is_none() {
        o.stance = Some(stance(&text, fallback));
    }
    let mut tags: Vec<String> = news_topic(&text).map(str::to_owned).into_iter().collect();
    tags.extend(entities(&text).into_iter().map(str::to_owned));
    tags.append(&mut o.tags);
    o.tags = tags;
}

/// Infers the modality from text. Ambiguous mentions yield hybrid.
pub fn modality(text: &str) -> Option<Modality> {
    let text = text.to_lowercase();
    match (
        any(&text, HYBRID),
        any(&text, ONLINE),
        any(&text, IN_PERSON),
    ) {
        (true, _, _) | (false, true, true) => Some(Modality::Hybrid),
        (false, true, false) => Some(Modality::Online),
        (false, false, true) => Some(Modality::InPerson),
        (false, false, false) => None,
    }
}

/// Fills unknown fields of `o` from explicit mentions in its own text.
pub fn enrich(o: &mut Opportunity) {
    let text = o.haystack();
    if o.category == Category::Other
        && let Some(c) = category(&text)
    {
        o.category = c;
    }
    if o.modality == Modality::Unknown
        && let Some(m) = modality(&text)
    {
        o.modality = m;
    }
    if o.cost == Cost::Unknown && any(&text, FREE) {
        o.cost = Cost::Free;
    }
    if o.perks.certificate.is_none() && any(&text, CERTIFICATE) {
        o.perks.certificate = Some(true);
    }
    if o.perks.travel_covered.is_none() && any(&text, TRAVEL) {
        o.perks.travel_covered = Some(true);
    }
    if o.perks.accommodation_covered.is_none() && any(&text, ACCOMMODATION) {
        o.perks.accommodation_covered = Some(true);
    }
    if o.perks.prizes.is_none() && any(&text, PRIZES) {
        o.perks.prizes = Some("Mentioned in the description".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalUrl, SourceKind};
    use chrono::Utc;

    #[test]
    fn detects_categories_in_both_languages() {
        assert_eq!(
            category("NASA Space Apps Hackathon 2026"),
            Some(Category::Hackathon)
        );
        assert_eq!(
            category("Beca completa para maestría"),
            Some(Category::Scholarship)
        );
        assert_eq!(
            category("Prácticas pre-profesionales en TI"),
            Some(Category::Internship)
        );
        assert_eq!(category("Taller de Linux"), Some(Category::Workshop));
        assert_eq!(category("Curso gratuito de Rust"), Some(Category::Course));
        assert_eq!(
            category("We're hiring a backend engineer"),
            Some(Category::Job)
        );
        assert_eq!(category("Nothing to see"), None);
        // Whole words only.
        assert_eq!(category("Jobsite safety"), None);
    }

    #[test]
    fn classifies_news_without_claiming_verification() {
        assert_eq!(
            stance("NVIDIA reportedly plans new GPU", None),
            Stance::Speculation
        );
        assert_eq!(
            stance("Ask HN: Is Rust worth it?", None),
            Stance::Discussion
        );
        assert_eq!(stance("Opinion: the AI bubble", None), Stance::Opinion);
        assert_eq!(stance("Linux 7.0 released", None), Stance::Report);
        assert_eq!(
            stance("Rust 1.99 released", Some(Stance::Official)),
            Stance::Official
        );
        assert_eq!(news_topic("Critical CVE in OpenSSH"), Some("security"));
        assert_eq!(news_topic("Jensen Huang says GPUs…"), Some("hardware"));
        assert_eq!(
            entities("Jensen Huang and Sam Altman"),
            ["nvidia", "openai"]
        );
    }

    #[test]
    fn detects_modality() {
        assert_eq!(
            modality("Evento presencial en Cusco"),
            Some(Modality::InPerson)
        );
        assert_eq!(modality("Fully remote role"), Some(Modality::Online));
        assert_eq!(
            modality("Online and in-person tracks"),
            Some(Modality::Hybrid)
        );
        assert_eq!(modality("Lima"), None);
    }

    #[test]
    fn enrich_fills_only_unknowns() {
        let url = CanonicalUrl::parse("https://x.example/").unwrap();
        let mut o = Opportunity::new(
            SourceKind::Manual,
            "manual",
            url.as_str(),
            &url,
            "Fully funded fellowship with accommodation, gratis, certificado",
            Utc::now(),
        );
        o.modality = Modality::InPerson;
        enrich(&mut o);
        assert_eq!(o.category, Category::Fellowship);
        assert_eq!(o.modality, Modality::InPerson);
        assert_eq!(o.cost, Cost::Free);
        assert_eq!(o.perks.certificate, Some(true));
        assert_eq!(o.perks.travel_covered, Some(true));
        assert_eq!(o.perks.accommodation_covered, Some(true));
    }
}
