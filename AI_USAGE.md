# AI usage

This document answers a question the community asked: was AI used to build
SENTRI, and how much?

**Short answer:** SENTRI's development used AI coding assistants heavily. At
runtime, SENTRI itself uses no AI and sends nothing to AI services.

## Disclosure level

We use four levels:

1. **No AI used.**
2. **AI assistance only:** brainstorming, explanations, code review or
   debugging suggestions.
3. **Light AI generation:** limited code, documentation or tests generated
   with AI and reviewed by a human.
4. **Heavy AI generation:** substantial portions of code or documentation
   generated with AI assistance and then reviewed.

**SENTRI is at level 4: heavy AI generation.** The maintainer confirmed this
level; it is not an estimate made from the code.

## AI assistance during development

- The maintainer, Gustavo Jhon ([@HustavoJhon](https://github.com/HustavoJhon)),
  decides what SENTRI does. He writes the specifications for each phase:
  features, constraints, security rules and acceptance criteria.
- AI coding assistants (Claude Code by Anthropic, and OpenCode) implemented
  many of those specifications. That includes auditing the existing code,
  writing Rust code and tests, debugging, running the app in real terminals,
  and drafting documentation.
- AI was also used for research, such as checking platform APIs and terminal
  protocol support, and to review the code.

## AI-generated or AI-assisted source code

- Substantial parts of the Rust code, the tests and the documentation under
  `docs/` were generated with AI assistance. They were not written from
  scratch by hand.
- Files are not marked one by one, and the git history does not tell AI
  output apart from hand-written code. Every commit is authored by the
  maintainer, who reviewed and accepted it, and commits carry no AI
  co-author trailers. This document is the disclosure for the whole project.

## Human review and responsibility

- The maintainer reviews the changes, runs SENTRI, and decides what is
  committed and published.
- Every commit passes the same quality gate (`scripts/commit-if-green.sh` →
  `scripts/check.sh`): `cargo fmt`, `cargo clippy` with pedantic lints as
  errors, and the full test suite. CI runs the same checks.
- Features are checked by running SENTRI, not only by tests. When a claim
  could not be checked in practice (for example a terminal that was not
  available), the documentation says so.
- The maintainer is responsible for the project: its code, its behavior and
  its documentation, however each piece was produced.

## AI at runtime

**SENTRI does not use AI at runtime.**

- Recommendations come from explicit rules: interests, keywords, city,
  country, modality, deadlines and your likes and saves
  ([ADR 0003](docs/adr/0003-rule-based-relevance.md)). Every score shows the
  reasons behind it.
- News classification (Reported, Official, Unconfirmed, Opinion, Discussion)
  and topic detection use keyword rules in `sentri-core`, not a language
  model.
- SENTRI includes no machine-learning model and calls no AI API.

## Your data and AI services

**SENTRI does not send your data to AI services.**

It only contacts:

- the sources you enable in `config.toml` (feeds, public APIs and pages);
- the links you add yourself with `sentri add` or `n` in the interface;
- image hosts, to download pictures for items you view;
- GitHub's public API, only if you set `[github] username` (during
  onboarding or later), to read your public profile, avatar and the
  languages of your public repositories (no token).

Your profile, location, interests, likes, notes and history stay on your
computer, in `config.toml` and the local SQLite database: your country, city,
time zone and interests are never sent anywhere (see
[docs/privacy.md](docs/privacy.md) and [docs/security.md](docs/security.md)).

## Questions

If something here is unclear, or you think it is inaccurate, please
[open an issue](https://github.com/HustavoJhon/sentri/issues).
