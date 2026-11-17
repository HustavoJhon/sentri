# Roadmap

Las fases son incrementales; cada una termina con funcionalidad probada.

| Fase | Contenido | Estado |
|---|---|---|
| 0 | Auditoría, rama `dev`, workspace, calidad, docs | ✅ |
| 1 | Modelos, SQLite, migraciones, repositorios, configuración | ✅ |
| 2 | Normalización, deduplicación, búsqueda, filtros | ✅ |
| 3 | Likes, guardados, descartes, historial, `sentri add`, `sentri edit` | ✅ (formulario de edición en la TUI pendiente) |
| 4 | TUI: Gruvbox, dashboard, feed, detalles, atajos, command palette | ✅ |
| 5 | Conectores: RSS/Atom ✅, GitHub ✅, Peruanos.dev y hack0.dev ✅, GDG ✅, Hacker News, iCalendar (Reddit descartado, ver sources.md) | 🚧 |
| 6 | Intereses, scoring, explicaciones, personalización | 🚧 (base hecha en fase 1) |
| 7 | Eventos y Google Calendar (OAuth2) | 🚧 calendario local completo; Google sin conectar |
| 8 | Cursos, certificados, proyectos open source, aprendizaje | ⏳ |
| 9 | Empleos, perfil profesional, compatibilidad | ⏳ |
| 10 | Asistencia a inscripciones con confirmación explícita | ⏳ |
| 11 | Daemon, systemd, reintentos, supervisión | ✅ base (reintentos con backoff por fuente pendientes) |
| 12 | Acceso remoto (SSH + Tailscale), preparación VPS | ⏳ |
| 13 | API Axum, PWA | ⏳ |

Reordenación respecto al plan original: el motor de scoring (fase 6) se
introduce en su forma básica junto al núcleo (fase 1), porque el feed necesita
un orden desde el primer día; la fase 6 lo amplía con personalización.

## Próximos pasos

1. Formulario de edición y de alta completo en la TUI (hoy: alta por URL y `sentri edit`).
2. Conectores Hacker News (*Who is hiring?*), iCalendar (Luma/GDG/Meetup) y Microsoft Learn.
3. Reintentos con backoff por fuente en el daemon y límite de frecuencia por fuente.
4. Fase 7: Google Calendar con OAuth2 (requiere crear credenciales en Google Cloud;
   se dejará documentado sin inventar credenciales).
5. Afinidad: excluir la propia oportunidad al explicar "similar a lo que guardaste".

## Fase de evolución visual (2026-11-15)

Hecho: dashboard con gráficos, Tech News (HN, Google News RSS, medios y blogs
oficiales), cards con densidades, Details enriquecido con imágenes, editor
completo con capas detectado/manual, registro de redes sociales, calendario
mes/semana/día/agenda, heatmap de actividad, perfil y avatar de GitHub,
Settings editables.

Pendiente de esta fase:

- Personalizar atajos de teclado (hoy solo consultables).
- Preferencias de calendario (inicio de semana, colores) y Google Calendar.
- Iconos ASCII completos cuando `ascii_icons = true` (la opción ya existe y se
  guarda; falta sustituir todos los glifos).
- Reclasificar noticias antiguas tras mejorar la heurística (las ya guardadas
  conservan su naturaleza detectada).

## Fase del ecosistema tech peruano (2026-11-17)

Hecho:

- Conector Peruanos.dev (`kind = "peruanos"`) con cuatro objetivos independientes:
  eventos, comunidades, proyectos open source y Call for Speakers. La API real se
  comprobó el 2026-11-17 (endpoints, campos y filtros de ciudad/tipo).
- Conector hack0.dev (`kind = "hack0"`): listado público e JSON-LD de eventos,
  solo Perú, sin usar `/api/` (prohibido por `robots.txt`).
- Capítulos GDG de Lima y Cloud Lima añadidos por configuración, reutilizando el
  conector GDG existente.
- Categorías nuevas `Community` y `CallForSpeakers`, y vistas dedicadas
  *Communities* y *Call for Speakers* en la TUI. Los eventos y proyectos de Perú
  se muestran en las vistas *Events* y *Open Source* existentes.
- Filtros rápidos de modalidad (`M`), ciudad (`L`, empezando por la ciudad del
  perfil) y pasados (`H`), además de los existentes de precio, viaje,
  próximos y elegibles. Los plazos de los CFS se ordenan por fecha límite.
- Deduplicación entre fuentes con `opportunity_sightings`: un mismo evento de
  Peruanos.dev, GDG o hack0.dev es un solo elemento con varias referencias; los
  detalles muestran cada fuente. Las correcciones manuales nunca se sobrescriben.
- Pruebas unitarias con respuestas reales recortadas y pruebas de integración
  contra un servidor mock (deduplicación, recolección repetida, ediciones
  manuales, `429` y JSON inválido sin afectar a las demás fuentes).

Pendiente:

- No hay una API de comunidades de GDG ni de hack0.dev; esas fuentes aportan
  eventos. Las comunidades se descubren por Peruanos.dev o con `sentri add`.
- Los CFS de Peruanos.dev no siempre publican elegibilidad ni requisitos.
- Seguir la evolución del esquema de Peruanos.dev y de hack0.dev (sin versión
  pública); si cambian, el conector deja de importar y se usa `sentri add`.

## Fase de fuentes y fiabilidad (2026-11-16)

Hecho: conector iCal (Luma, Meetup, `.ics`), GDG (schema.org), Devpost
(opcional), Kaggle (con credenciales), parser de NVIDIA, deduplicación de
noticias entre feeds, heurística "Unconfirmed" más estricta y reclasificación
reversible, peticiones condicionales y backoff por fuente, gestor de fuentes
en la TUI, preferencias de calendario y `calendar-doctor`, atajos editables con
detección de conflictos, modo ASCII real.

Pendiente:

- OAuth y sincronización con Google Calendar (requiere credenciales del usuario).
- Probar Kaggle contra la API real cuando haya credenciales.
- Eventbrite (solo con token y por organizador) y HackerRank: sin conector.
- Conectores de empleo remoto (Remotive, RemoteOK, Arbeitnow) y Microsoft Learn.
