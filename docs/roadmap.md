# Roadmap

Las fases son incrementales; cada una termina con funcionalidad probada.

| Fase | Contenido | Estado |
|---|---|---|
| 0 | Auditoría, rama `dev`, workspace, calidad, docs | ✅ |
| 1 | Modelos, SQLite, migraciones, repositorios, configuración | 🚧 |
| 2 | Normalización, deduplicación, búsqueda, filtros | ⏳ |
| 3 | Likes, guardados, descartes, historial, `sentri add` | ⏳ |
| 4 | TUI: Gruvbox, dashboard, feed, detalles, atajos, command palette | ⏳ |
| 5 | Conectores: RSS/Atom, GitHub, Hacker News, iCalendar (Reddit descartado, ver sources.md) | ⏳ |
| 6 | Intereses, scoring, explicaciones, personalización | ⏳ |
| 7 | Eventos y Google Calendar (OAuth2) | ⏳ |
| 8 | Cursos, certificados, proyectos open source, aprendizaje | ⏳ |
| 9 | Empleos, perfil profesional, compatibilidad | ⏳ |
| 10 | Asistencia a inscripciones con confirmación explícita | ⏳ |
| 11 | Daemon, systemd, reintentos, supervisión | ⏳ |
| 12 | Acceso remoto (SSH + Tailscale), preparación VPS | ⏳ |
| 13 | API Axum, PWA | ⏳ |

Reordenación respecto al plan original: el motor de scoring (fase 6) se
introduce en su forma básica junto al núcleo (fase 1), porque el feed necesita
un orden desde el primer día; la fase 6 lo amplía con personalización.
