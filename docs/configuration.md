# Configuración

Fichero: `~/.config/sentri/config.toml` (o la ruta en `SENTRI_CONFIG`).
Si no existe, SENTRI usa la plantilla por defecto
([`crates/sentri-app/config/default.toml`](../crates/sentri-app/config/default.toml)).

```sh
sentri config init    # escribe la plantilla comentada (no sobrescribe)
sentri config path    # muestra la ruta en uso
sentri config check   # valida el fichero
```

Las claves desconocidas son un error (para detectar erratas).

## `[profile]`

| Clave | Descripción |
|---|---|
| `home_city` | Ciudad donde asistir en persona es fácil (Cusco). |
| `country` | País de participación (elegibilidad). |
| `location_aliases` | Otros nombres que cuentan como "tu país/región". |
| `preferred_modalities` | `online`, `in_person`, `hybrid`. |
| `avoid_keywords` | Términos que restan relevancia (nunca ocultan). |

### `[[profile.interests]]`

| Clave | Descripción |
|---|---|
| `name` | Nombre visible del interés. |
| `keywords` | Palabras o frases completas buscadas en título, resumen, etiquetas y ubicación. |
| `categories` | Categorías relevantes: `event`, `conference`, `workshop`, `meetup`, `course`, `training`, `hackathon`, `competition`, `scholarship`, `exchange`, `fellowship`, `volunteering`, `call`, `job`, `internship`, `open_source`, `other`. |
| `priority` | `low` (×1), `medium` (×2), `high` (×3). |

Añadir un interés nuevo no requiere tocar código: basta un bloque más.

## `[scoring]`

Pesos de cada regla (valores por defecto entre paréntesis):
`interest_keyword` (6, por coincidencia, máx. 3, × prioridad),
`interest_category` (4 × prioridad), `home_city` (15), `home_country` (8),
`preferred_modality` (5), `eligible` (8), `not_eligible` (-30), `free` (4),
`certificate` (4), `prizes` (5), `travel` (12), `deadline_soon` (10),
`deadline_passed` (-40), `avoid_keyword` (-20), `affinity_unit` (3),
`affinity_cap` (24), `deadline_soon_days` (14).

La afinidad aprendida suma señales de tus interacciones por categoría,
etiqueta y "ofrece viaje": like +2, guardado +3, interesante +1,
postulado/inscrito +3, descartado −3.

## `[http]`

`timeout_secs`, `max_body_bytes`, `max_redirects`: límites de toda petición saliente.

## `[[sources]]`

| Clave | Descripción |
|---|---|
| `name` | Identificador único (`a-z0-9-_.`). |
| `kind` | `rss`, `github`, … (ver `docs/sources.md`). |
| `enabled` | `true` por defecto. |
| `url` | URL del feed/endpoint (http/https). |
| `query` | Consulta, para fuentes con búsqueda. |
| `target` | Específico del conector. GitHub: `repositories` (por defecto) o `issues`. |
| `category` | Categoría por defecto de los ítems. |
| `tags` | Etiquetas añadidas a cada ítem. |
| `max_items` | 1–500 (50 por defecto). |
| `token_env` | **Nombre** de la variable de entorno con el token. Nunca el token. |

## `[ui]`

| Clave | Valores |
|---|---|
| `density` | `compact`, `comfortable`, `detailed` |
| `images` | `auto` (por defecto), `kitty`, `sixel`, `iterm2`, `halfblock`, `ascii`, `off` — ver [Imágenes](#imágenes) |
| `colors` | `auto`, `truecolor`, `indexed` |
| `ascii_icons` | `true`/`false` |
| `show_sidebar`, `show_details` | `true`/`false` |
| `feed_width` | 30–80 (% del área principal) |
| `collect_on_start` | `auto` (por defecto): recolecta al abrir la TUI si la base está vacía o ninguna fuente tuvo éxito dentro de `daemon.interval_minutes`; `always`; `never` |
| `cache_dir` | Caché de imágenes y avatar (`~/.cache/sentri/images`) |

## `[dashboard]`

| Clave | Valores |
|---|---|
| `layout` | `detailed` (por defecto) o `compact`: métricas en una línea de texto y gráficos más bajos. Las terminales de menos de 24 filas usan siempre `compact` |
| `widgets` | Widgets en el orden en que se dibujan: `metrics`, `activity`, `interactions`, `distribution`, `recommendations`, `upcoming`, `news`, `jobs`. Quitar uno lo oculta |
| `metrics` | Contadores, en orden: `found`, `new`, `news`, `events`, `courses`, `jobs`, `saved`, `liked`, `applied`, `calendar` |
| `recommendations` | Filas de recomendaciones (1–30, por defecto 10) |

Los gráficos consecutivos comparten fila, y también las listas consecutivas;
cada fila tiene tantos widgets como quepan (3 desde 110 columnas, 2 desde
70/60, 1 en terminales estrechas). Si las métricas no caben como recuadros en
una fila, se muestran como texto en una o dos líneas.

```toml
[dashboard]
layout = "compact"
widgets = ["recommendations", "upcoming", "jobs", "metrics"]
metrics = ["new", "jobs", "events", "saved"]
recommendations = 6
```

## `[cards]`

| Clave | Valores |
|---|---|
| `show_category` | Etiqueta de tipo (`JOB`, `MEETUP`, `OFFICIAL`…) |
| `show_source` | Badge y nombre de la plataforma |
| `show_meter` | Medidor `●●●○○` (en compact, la columna de puntuación) |
| `facts` | Datos bajo el título: `when`, `deadline`, `pay`, `organizer`, `place`, `modality`, `cost`, `perks`, `topic`, `skills`, `links`. Quitar uno lo oculta |
| `fact_order` | `kind` (por defecto: cada tipo de card usa su propio orden) o `custom` (el orden de `facts`) |
| `dates` | `both` (`Sat 28 Nov 08:30 · in 8d`), `relative` (`in 8d`) o `absolute` (`Sat 28 Nov 08:30`) |
| `selection` | `highlight` (barra y card sombreada) o `rail` (solo la barra) |

Todo `[dashboard]` y `[cards]` se edita en **Settings › Dashboard / Cards**
y se aplica al guardar, sin reiniciar. Las listas se escriben separadas por
comas; Settings muestra las opciones válidas y rechaza nombres desconocidos o
repetidos.

## `[github]`

`username`: tu usuario público. Solo se lee `GET /users/<usuario>` (sin token,
60 peticiones/hora); se guarda en caché 24 h. Se muestran avatar, nombre,
usuario, repositorios, seguidores y, si caben, ubicación, empresa o bio
públicas. Si falla, se muestran tus iniciales.

## `[daemon]`

`interval_minutes` (≥ 15).

## Fuentes de noticias

En `[[sources]]`, `content = "news"` envía los ítems a **Tech News** y
`stance` fija la naturaleza por defecto (`official` para blogs de empresa,
`discussion` para redes). Tipos de feed: `rss`, `googlenews`, `mastodon`,
`bluesky`, `youtube`; `hackernews` usa la API de Algolia (`query` opcional,
`target = "jobs"`).

Todo lo anterior también se edita desde **Settings** en la TUI
(`Ctrl-s` guarda con backup; `D` restaura los valores por defecto).

## `[calendar]`

| Clave | Valores |
|---|---|
| `week_start` | `monday` (por defecto) o `sunday` |
| `default_view` | `month`, `week`, `day`, `agenda` |
| `google_credentials` | Fichero OAuth (por defecto `~/.config/sentri/google-oauth.json`). Ver `sentri calendar-doctor`. |

## `[keys]`

Atajos remapeables: `acción = "tecla"`. Teclas: un carácter (`l`, `S`, `?`),
`ctrl-x`, `alt-x`, `enter`, `space`, `f1`…`f12`. Acciones: `like`, `save`,
`dismiss`, `interesting`, `applied`, `enrolled`, `completed`, `schedule`,
`edit`, `open`, `view_image`, `add_link`, `search`, `palette`, `density`, `sort`, `collect`,
`reload`, `help`, `quit`, `filter_free`, `filter_travel`, `filter_upcoming`,
`filter_eligible`, `filter_certificate`.

```toml
[keys]
like = "f"
collect = "f5"
```

Se rechazan teclas duplicadas y las reservadas para navegar (`j` `k` `g` `G`
`[` `]` dígitos, Enter, Esc, Tab, espacio, Ctrl-c, Ctrl-s). Dentro de
Calendar, History, Sources, Settings y el editor, sus teclas propias tienen
prioridad. Editable en **Settings › Shortcuts** (vacío = valor por defecto).

## Imágenes

Con `images = "auto"` SENTRI pregunta a la terminal al abrir la TUI qué
protocolo gráfico entiende y el tamaño de cada celda en píxeles:

| Protocolo | Terminales | Calidad |
|---|---|---|
| Kitty | kitty, Ghostty, WezTerm | píxeles reales |
| Sixel | foot, Windows Terminal, xterm (`-ti vt340`), Konsole, tmux ≥ 3.4 con sixel | píxeles reales |
| iTerm2 | iTerm2, WezTerm | píxeles reales |
| Medios bloques | cualquier terminal truecolor o de 256 colores, SSH, consola | 2 píxeles por celda |

Si la terminal no responde o no anuncia ninguno, se usan medios bloques (`▀`).
Dentro de tmux sin `allow-passthrough` la consulta no llega a la terminal
exterior, así que se usan medios bloques sin preguntar.
`kitty`, `sixel` o `iterm2` fuerzan un protocolo aunque no se anuncie (útil
dentro de tmux con `set -g allow-passthrough on`). `halfblock` y `ascii`
dibujan siempre con caracteres. Cambiar a un protocolo desde Settings requiere
reiniciar la TUI si se abrió con `halfblock`, `ascii` u `off`.

De cada imagen se descarga la mejor variante conocida (el original de
WordPress en vez del recorte `-500x500`, `highres_` de Meetup, el avatar de
GitHub a 460 px) y, si no existe, la URL publicada. Los bytes originales se
guardan en `cache_dir` una semana; cada tamaño que se dibuja se remuestrea una
sola vez desde ese original con un filtro Lanczos. Las transparencias se
mezclan con el fondo Gruvbox. Formatos: PNG, JPEG, WebP y GIF (SVG no).

`z` en un elemento con imagen abre el **visor**: la imagen ocupa toda la
pantalla y `Esc` vuelve exactamente a donde estabas. El pie indica la
resolución real de la imagen y cómo se dibuja; sin protocolo gráfico lo dice
(“half-block preview”), en vez de presentar una ampliación que no lo es.

## Modo ASCII

`[ui] ascii_icons = true` sustituye bordes, iconos, barras y marcas por ASCII
en todas las pantallas; el texto de las oportunidades no se modifica. Para una
terminal totalmente ASCII, combínalo con `images = "off"` o `"ascii"`.
