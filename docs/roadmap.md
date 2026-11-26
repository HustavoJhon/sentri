# Roadmap

Las fases son incrementales; cada una termina con funcionalidad probada.

| Fase | Contenido | Estado |
|---|---|---|
| 0 | Auditoría, rama `dev`, workspace, calidad, docs | ✅ |
| 1 | Modelos, SQLite, migraciones, repositorios, configuración | ✅ |
| 2 | Normalización, deduplicación, búsqueda, filtros | ✅ |
| 3 | Likes, guardados, descartes, historial, `sentri add`, `sentri edit` | ✅ (formulario de edición en la TUI pendiente) |
| 4 | TUI: Gruvbox, dashboard, feed, detalles, atajos, command palette | ✅ |
| 5 | Conectores: RSS/Atom ✅, GitHub ✅, Peruanos.dev, hack0.dev y dev.events ✅, Get on Board ✅, GDG ✅, Hacker News, iCalendar (Reddit descartado, ver sources.md) | 🚧 |
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

## Fase 13: Observer 3D e identidad visual (2026-11-25)

Hecho:

- Modelo 3D original del ward (cabeza con ojo-lente en ambas caras, alas con
  plumas barridas hacia atrás, cresta, bastón con núcleo de energía y base)
  muestreado en puntos con normales y renderizado con rotación, perspectiva,
  búfer de profundidad y sombreado por densidad de caracteres; determinista.
- Despertar: aceleración suave hasta una vuelta cada ~5 s, luz que se aviva
  según el progreso, ojo que se abre por etapas sin adelantarse a los pasos
  reales, anillos de energía, radar y partículas que se suman con el ojo.
- Pantalla de arranque con marca SENTRI, lema, panel INITIALIZATION (pasos,
  ubicación, ámbitos, progreso) y tres distribuciones según el tamaño; el ward
  plano de la fase 12 queda para terminales pequeñas. ASCII completo.
- Corregido: la pantalla podía quedarse congelada si caducaba entre un
  dibujado y el siguiente temporizador.
- `cargo run -p sentri-tui --example observer_frames` para revisar fotogramas.

Validado en Alacritty dentro de tmux: 120×46, 160×34 (lado a lado), 80×24
(ward plano), `LANG=C` (ASCII), `SENTRI_NO_ANIMATION=1`, primera ejecución con
el asistente después del Observer, y `sentri` sin terminal.

Pendiente: en tmux la detección de gráficos retrasa ~1 s el primer fotograma.

## Fase 12: despertar del Observer y onboarding personalizado (2026-11-25)

Hecho:

- Pantalla de arranque con un ward Observer original en Unicode y ASCII: el
  ojo se abre según pasos reales de inicialización, nunca bloquea más de 4 s,
  se salta con cualquier tecla, es estática con `SENTRI_NO_ANIMATION`/`CI` y se
  desactiva con `[onboarding] startup_animation = false`.
- Asistente de primera ejecución (GitHub, ubicación ISO, zona horaria, ámbitos,
  modalidades e intereses) que guarda cada página y retoma tras una
  interrupción. Los usuarios existentes no pasan por él; se repite con `O` en
  Settings o `sentri onboarding`.
- Scoring: intereses elegidos (`scoring.topic`), ámbito local/nacional/
  internacional (`scoring.international`) sin ocultar nada, nombres del país
  desde su código ISO y lenguajes de los repositorios públicos de GitHub para
  open source.
- `config.toml` se escribe de forma atómica; `docs/privacy.md` y
  `docs/onboarding.md`.
- `sentri` sin terminal interactiva ya no intenta abrir la interfaz.

Validado en Alacritty 0.17 dentro de tmux 3.8: primera ejecución, interrupción
y reanudación, final del asistente, arranque directo al dashboard, `O` en
Settings y la pantalla ASCII estática.

Pendiente: usar `profile.timezone` para mostrar fechas (hoy se guarda y las
fechas siguen la zona del sistema).

## Fase 11: comentarios de la comunidad, transparencia sobre IA y documentación (2026-11-21)

Hecho:

- Imágenes en terminales reales: Sixel en foot 1.28, respaldo en Alacritty
  0.17 y tmux 3.8 con y sin `allow-passthrough`. Dentro de tmux, `auto` ya no
  usa Sixel (las imágenes quedaban fuera de lugar) y se respeta el
  `allow-passthrough` global.
- Corregido: con la detección gráfica de la fase 10, la TUI no se dibujaba en
  terminales que responden a la consulta (foot) hasta pulsar una tecla.
- `sentri terminal` y la cabecera de Settings explican la terminal, el
  protocolo, el respaldo y el motivo; la terminal se identifica por su proceso.
- `AI_USAGE.md`, README en inglés (`README.md`) y español (`README.es.md`).
- La card de perfil oculta direcciones IP de los campos públicos de GitHub.

Validación posterior: protocolo de Kitty probado en kitty 0.49 directamente y
dentro de tmux (imágenes, visor, navegación, cambio de tamaño, salida limpia).
Las imágenes que se veían pixeladas en kitty venían de `images = "halfblock"`
en una configuración antigua; ahora SENTRI lo avisa.

Pendiente: cambiar el tamaño con imágenes Sixel en pantalla.

## Fase 10 de UX: dashboard, Details e identidad visual (2026-11-20)

Hecho:

- Dashboard configurable (`[dashboard]`): widgets visibles y su orden,
  métricas, número de recomendaciones, diseño `detailed`/`compact`, y un
  widget nuevo de empleos.
- Cards con un orden de datos propio para cada tipo (empleos, eventos, cursos,
  concursos, comunidades, open source, CFP, noticias) y preferencias en
  `[cards]` (datos visibles, orden, fechas, selección, tipo, plataforma,
  medidor), aplicadas al guardar.
- Descripciones con estructura: el HTML de las fuentes conserva párrafos y
  listas; las descripciones aplanadas se actualizan al recolectar de nuevo sin
  tocar las ediciones manuales.
- Visor de imágenes (`z`), barra estrecha en terminales de 60–79 columnas,
  perfil de GitHub con seguidores y ubicación/empresa/bio, fechas relativas por
  día de calendario y la primera tecla ya no se pierde tras la consulta
  gráfica.

Pendiente: probar Kitty e iTerm2 en terminales reales; las descripciones
guardadas antes de esta fase recuperan su estructura solo cuando su fuente
vuelve a publicarlas igual.

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

## Fase de primer arranque y diseño (2026-11-18)

- Nuevas fuentes probadas en vivo: dev.events (meetups y conferencias de Perú,
  una petición por ejecución), Get on Board (API pública de empleos abiertos a
  Perú: programación y datos/IA) y noticias en español (Xataka, Genbeta,
  Hipertextual, El Comercio Tecnología, Google News Perú).
- Primer arranque sin pantallas vacías: la TUI recolecta en paralelo al abrir
  (`ui.collect_on_start`) y llena cada sección a medida que responden las
  fuentes, con progreso en cabecera, barra de estado y pantalla de bienvenida.
- Escrituras concurrentes seguras (`BEGIN IMMEDIATE`) y registros en
  `sentri.log` en lugar de la terminal mientras la TUI está abierta.
- Diseño: contadores por sección, atajos contextuales que respetan los atajos
  personalizados, cards con empresa, salario y día del evento, recomendaciones
  que mezclan plataformas, columna de fechas corregida, y bordes que ya no se
  rompen por iconos o emoji de ancho ambiguo.
- Eventos publicados por varios directorios se fusionan por título normalizado
  y fecha de inicio.

Pendiente: RPP, La República y PRONABEC no tienen feeds públicos.

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
