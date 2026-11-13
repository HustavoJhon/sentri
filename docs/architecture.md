# Arquitectura

SENTRI es un **monolito modular** en un workspace Cargo. No hay microservicios:
la TUI, la CLI, el daemon y la futura API son *adaptadores* que llaman a los
mismos casos de uso.

```
            ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐
 adapters   │   tui    │  │   cli    │  │  daemon  │  │ api (fut)│
            └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘
                 └─────────────┴──────┬──────┴─────────────┘
                               ┌──────▼──────┐
 use cases                     │ sentri-app  │  config, casos de uso
                               └──┬───────┬──┘
                    ┌─────────────┘       └────────────┐
             ┌──────▼────────┐                  ┌──────▼─────────┐
 infra       │sentri-storage │                  │ sentri-sources │
             │ SQLite + SQLx │                  │ conectores,HTTP│
             └──────┬────────┘                  └──────┬─────────┘
                    └───────────┐       ┌──────────────┘
                              ┌─▼───────▼─┐
 domain                       │sentri-core│  modelos, reglas, scoring
                              └───────────┘
```

## Crates

| Crate | Responsabilidad | Depende de |
|---|---|---|
| `sentri-core` | Modelos de dominio, normalización/deduplicación de URLs, intereses, motor de relevancia basado en reglas. **Sin I/O.** | — |
| `sentri-storage` | SQLite, migraciones versionadas, repositorios. | core |
| `sentri-sources` | Trait `Connector`, conectores (RSS/Atom, Reddit, GitHub…), cliente HTTP seguro (anti-SSRF, límites), extracción de metadatos. | core |
| `sentri-app` | Configuración (TOML) y casos de uso: recopilar, añadir enlace, interactuar, consultar feed. | core, storage, sources |
| `sentri-tui` | Interfaz Ratatui. Solo habla con `sentri-app`. | app, core |
| `sentri-cli` | Binario `sentri` (Clap). Despacha subcomandos y abre la TUI. | app, tui |

Módulos previstos (`calendar`, `applications`, `daemon`, `api`) se crearán como
crates **cuando se implementen**, no antes (ver ADR-0001). Mientras tanto la
recomendación vive en `sentri-core::scoring`, porque es lógica pura.

## Reglas

1. `sentri-core` no hace I/O ni depende de Tokio, SQLx o Reqwest.
2. Los adaptadores (TUI/CLI/daemon/API) no ejecutan SQL ni HTTP directamente.
3. Todo contenido externo se trata como no confiable: se valida, se trunca y
   nunca se ejecuta.
4. Cada conector es independiente; los errores se registran por fuente y no
   abortan una recopilación.
5. Errores: `thiserror` en librerías, `anyhow` solo en el binario.

## Flujo de datos

```
Connector::fetch ─► RawItem ─► normalize ─► Opportunity (fingerprint)
     ─► storage.upsert (dedup por URL canónica) ─► scoring ─► feed
```

## Despliegue por etapas

1. PC Linux local: TUI + daemon locales, SQLite en `~/.local/share/sentri`.
2. Móvil: SSH vía Tailscale a la misma máquina; la TUI funciona sin Nerd Fonts
   ni protocolos gráficos.
3. VPS: se mueven daemon + base de datos; las rutas son configurables.
4. API Axum + PWA reutilizando `sentri-app`.
