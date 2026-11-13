# SENTRI

**Personal Opportunity Intelligence Platform** — un radar personal de oportunidades
que se usa desde la terminal.

SENTRI reúne en un único feed eventos, cursos, hackathons, becas, empleos,
voluntariados, fellowships, convocatorias y proyectos open source procedentes de
múltiples fuentes (RSS/Atom, Reddit, GitHub, enlaces añadidos a mano…), los
puntúa según un perfil de intereses configurable y permite revisarlos, darles
like, guardarlos, descartarlos o convertirlos en actividades de calendario.

> Estado: **en desarrollo temprano** (rama `dev`). Consulta el
> [plan de fases](docs/roadmap.md).

## Características (objetivo)

- Feed unificado con filtros, búsqueda y puntuación de relevancia explicable.
- Interacciones independientes: like, guardado, descarte, postulado, inscrito, completado.
- `sentri add URL` para añadir cualquier enlace, con extracción segura de metadatos.
- Conectores independientes: un fallo en una fuente no detiene las demás.
- TUI en Rust + Ratatui con tema Gruvbox Dark, usable por SSH sin Nerd Fonts.
- Daemon de recopilación separado de la interfaz.
- Integración con Google Calendar vía OAuth2 (calendario propio `SENTRI Opportunities`).

## Requisitos

- Rust stable (edición 2024, MSRV 1.89).
- Nada más: SQLite va embebido (`sqlx` con `sqlite-bundled`).

## Uso rápido

```sh
cargo run -p sentri-cli --            # abre la TUI
cargo run -p sentri-cli -- add URL    # añade un enlace
cargo run -p sentri-cli -- search rust
cargo run -p sentri-cli -- --help
```

Instalar el binario `sentri`:

```sh
cargo install --path crates/sentri-cli
```

## Configuración y datos

| Qué | Ruta por defecto (Linux) |
|---|---|
| Configuración | `~/.config/sentri/config.toml` |
| Base de datos | `~/.local/share/sentri/sentri.db` |

Ver [docs/configuration.md](docs/configuration.md). Los secretos (tokens OAuth,
tokens de API) **nunca** se guardan en el repositorio.

## Desarrollo

```sh
./scripts/check.sh   # fmt + clippy (-D warnings) + tests
```

Documentación:

- [Arquitectura](docs/architecture.md)
- [Decisiones (ADR)](docs/adr/)
- [Investigación de fuentes](docs/sources.md)
- [Seguridad](docs/security.md)
- [TUI y atajos](docs/tui.md)
- [Configuración](docs/configuration.md)
- [Base de datos](docs/database.md)
- [Roadmap](docs/roadmap.md)
