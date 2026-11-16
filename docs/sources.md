# Investigación de fuentes

Fecha de la revisión: **2026-11-13**. Las políticas de las plataformas cambian
con frecuencia; cada conector debe revalidarse al implementarse. Las entradas
marcadas con *(verificar)* se basan en conocimiento previo y no se han
contrastado en esta revisión.

Leyenda de viabilidad: ✅ integrar · 🟡 integrar con condiciones · ❌ no integrar
automáticamente (usar `sentri add`).

## Fuentes principales

### Reddit — ❌ (cambio reciente)

| Aspecto | Situación |
|---|---|
| API oficial | Data API con OAuth2. Desde nov-2025 requiere aprobación manual (*Responsible Builder Policy*). Reddit dejó de aceptar nuevas solicitudes de acceso tras el 31-oct-2026; el acceso público restante termina en marzo de 2027. |
| RSS | **Retirado el 13-nov-2026.** Reddit indica que no hay reemplazo para uso fuera de comunidades que moderas. |
| JSON sin autenticar | Bloqueado/limitado en 2026; además no es una vía permitida. |
| Uso comercial | Requiere contrato. Uso académico: *Reddit for Researchers*. |

**Decisión:** no se implementa conector de Reddit. Las publicaciones de Reddit
se incorporan con `sentri add URL` (la extracción de metadatos funciona sobre
la página pública). Si en el futuro se obtiene acceso aprobado, el conector
encaja en el trait `Connector` sin cambios en el resto del sistema.

Fuentes: [TechRepublic](https://www.techrepublic.com/article/news-reddit-rss-public-api-shutdown/),
[The Next Web](https://thenextweb.com/news/reddit-rss-feeds-shut-down-old-reddit-ai-scraping),
[ppc.land](https://ppc.land/reddit-to-halt-public-data-api-access-in-march-2027-as-14-000-apps-register/).

### LinkedIn — ❌

| Aspecto | Situación |
|---|---|
| API oficial | Las APIs de empleos, búsqueda y feed están restringidas a partners aprobados *(verificar)*. La API abierta (*Sign In with LinkedIn*, *Share*) no permite leer ofertas ni eventos. |
| RSS | No disponible. |
| Scraping | Prohibido por el User Agreement; riesgo real de bloqueo de cuenta. |

**Decisión:** sin conector. Alternativas: `sentri add` con el enlace de la oferta
(solo metadatos públicos `og:*`), y fuentes alternativas de empleo (ver abajo).
Nunca se usan cookies de sesión ni credenciales de la cuenta personal.

### X / Twitter — 🟡 (de pago, desactivado por defecto)

| Aspecto | Situación |
|---|---|
| API oficial | API v2. Desde feb-2026 el modelo por defecto es *pay-per-use* y no hay nivel gratuito para nuevos desarrolladores. Se cobra por recurso devuelto (~0,005 USD por post leído según fuentes secundarias). |
| RSS | No disponible. |
| Scraping | Prohibido por los términos. |

**Decisión:** conector opcional futuro que requiera un *bearer token* del usuario
y un tope de gasto configurable (`max_reads_per_day`). Desactivado por defecto.
Mientras tanto: `sentri add`.

Fuentes: [postproxy.dev](https://www.postproxy.dev/blog/x-api-pricing-2026/),
[opentweet.io](https://opentweet.io/answers/is-the-x-api-free-in-2026).

### Google Search / Web — 🟡

| Aspecto | Situación |
|---|---|
| Custom Search JSON API | Cerrada a nuevos clientes; se apaga el **1-ene-2027**. No se usará. |
| Sucesor | Vertex AI Search (de pago, limitado a dominios configurados). |
| Alternativas | Brave Search API (clave, de pago *(verificar)*), SearXNG autoalojado (JSON propio). |

**Decisión:** conector de búsqueda web genérico contra **SearXNG** (instancia
propia en el PC o VPS) como opción recomendada, y Brave Search como opción con
clave. Ninguno es obligatorio.

Fuentes: [Firecrawl](https://www.firecrawl.dev/blog/google-search-json-api-alternative),
[Brave](https://brave.com/learn/google-api-shutdown/).

## Otras fuentes

| Fuente | API / feed | Auth | Viabilidad | Notas |
|---|---|---|---|---|
| **RSS/Atom genérico** | RSS 2.0, Atom, JSON Feed | No | ✅ | Base para blogs, portales de becas (muchos son WordPress: `/feed`), universidades, comunidades. |
| **GitHub** | REST v3 / GraphQL v4 | Opcional (token) | ✅ | Búsqueda de repos e issues `good first issue`. Sin token: 10 req/min en *search*; con token: 30 req/min. Guardar solo metadatos. |
| **Hacker News** | API oficial Firebase + Algolia HN Search | No | ✅ | Hilos *Who is hiring?*, *Show HN*. Sin límites publicados; ser conservador. |
| **Devpost** | Sin API pública documentada *(verificar)* | — | 🟡 | El listado de hackathons usa un endpoint JSON interno no documentado. Revisar robots/ToS antes de usarlo; si no está permitido, `sentri add`. |
| **Luma** | iCal por calendario; API oficial solo para organizadores con plan de pago *(verificar)* | No (iCal) | ✅ | Suscribirse a los `.ics` de calendarios públicos de comunidades. Requiere conector **iCalendar**. |
| **GDG / Google Developer Groups** | Plataforma Bevy (gdg.community.dev); feeds/iCal por capítulo *(verificar)* | No | 🟡 | GDG Cusco: si el capítulo expone iCal, usar conector iCalendar; si no, `sentri add`. |
| **Eventbrite** | API oficial con token; la búsqueda pública de eventos fue retirada (2020) | Sí | 🟡 | Solo eventos por organizador/venue conocido. |
| **Meetup** | API GraphQL; requiere suscripción Meetup Pro *(verificar)* | Sí | 🟡 | Alternativa: iCal de cada grupo, si está disponible. |
| **HackerEarth** | API pública de challenges retirada *(verificar)* | — | ❌ | `sentri add`. |
| **Kaggle** | API oficial (token de usuario) | Sí | ✅ | Listado de competiciones. |
| **Coursera** | API de catálogo restringida a partners/afiliados *(verificar)* | Sí | 🟡 | `sentri add` o feeds de blog. |
| **edX** | Discovery API para partners *(verificar)* | Sí | 🟡 | Igual que Coursera. |
| **freeCodeCamp** | RSS de noticias (`/news/rss`) | No | ✅ | Vía conector RSS. |
| **Microsoft Learn** | Catalog API pública (`learn.microsoft.com/api/catalog`) | No | ✅ | Cursos, rutas y certificaciones con metadatos ricos. |
| **AWS Training / Skill Builder** | Sin API pública *(verificar)* | — | ❌ | Blogs RSS de AWS para anuncios. |
| **Google Cloud Skills Boost** | Sin API pública *(verificar)* | — | ❌ | Blog RSS de Google Cloud. |
| **Portales de becas** | RSS (WordPress) en muchos casos | No | ✅ | Configurables como fuentes RSS. PRONABEC/CONCYTEC: revisar si publican feeds; si no, `sentri add`. |
| **Voluntariados** | Variable (UN Volunteers, Idealist…) *(verificar)* | — | 🟡 | Evaluar caso por caso. |
| **Empleo remoto** | Remotive API, RemoteOK API, We Work Remotely RSS, Arbeitnow API *(verificar ToS: exigen atribución/enlace)* | No | ✅ | Buen sustituto de LinkedIn para empleos remotos desde Perú. |
| **Empleo por empresa** | Greenhouse / Lever / Ashby job board APIs públicas | No | ✅ | Seguir empresas concretas. |
| **Empleo Perú** (Computrabajo, Bumeran) | Sin API pública *(verificar)* | — | ❌ | `sentri add`. |

## Orden de implementación propuesto

1. **RSS/Atom genérico** — cubre el mayor número de fuentes con un solo conector.
2. **GitHub** — repos y *good first issues*.
3. **Hacker News** — empleos (*Who is hiring?*) y proyectos.
4. **iCalendar** — Luma, GDG, Meetup cuando expongan `.ics`.
5. **Microsoft Learn Catalog** — cursos.
6. **Empleo remoto** (Remotive / RemoteOK / Arbeitnow).
7. **Búsqueda web** (SearXNG/Brave), **Kaggle**, **X** (opcional, de pago).

## Reglas para todos los conectores

- Identificarse con un `User-Agent` genérico (`sentri/<versión>`), sin datos personales.
- Respetar límites de uso y `Retry-After`; límite por fuente configurable.
- Guardar solo metadatos necesarios y siempre la URL original.
- Un conector que falla registra el error en `collection_runs` y no detiene al resto.

## Redes sociales (revisión 2026-11-13)

| Plataforma | API oficial y acceso | Coste / límites | Automatización en SENTRI |
|---|---|---|---|
| **Instagram** | Graph API solo para cuentas profesionales propias; búsqueda de hashtags requiere app revisada y cuenta business. Basic Display API retirada (dic-2024) *(verificar)*. oEmbed requiere token de app. | Gratis con revisión de Meta | ❌ Solo `sentri add` (se guarda la URL; título, autor e imagen se completan a mano si Instagram bloquea la página). |
| **Threads** | Threads API para publicaciones propias; búsqueda por palabra clave requiere permiso revisado *(verificar)*. | Gratis con revisión | ❌ `sentri add`. |
| **Facebook** | Contenido público de páginas requiere revisión (PPCA) *(verificar)*. | — | ❌ `sentri add`. |
| **TikTok** | Research API solo para investigación académica; Display API solo contenido propio *(verificar)*. | — | ❌ `sentri add`. |
| **X** | API v2 de pago por uso (ver arriba). | ~0,005 USD/post leído | 🟡 Futuro conector opcional con tope de gasto; hoy `sentri add`. |
| **LinkedIn / Reddit** | Ver secciones anteriores. | — | ❌ `sentri add`. |
| **YouTube** | Data API v3 con clave (10 000 unidades/día gratis) y **feeds RSS por canal sin clave**. | Gratis | ✅ `kind = "youtube"` con `url = …/feeds/videos.xml?channel_id=…` (probado). |
| **Bluesky** | AT Protocol abierto; AppView pública sin autenticación; **RSS por perfil**. | Gratis | ✅ `kind = "bluesky"` con `url = https://bsky.app/profile/<handle>/rss` (probado; redirige a la URL con DID). |
| **Mastodon** | API pública por instancia y **RSS por cuenta y hashtag** (`/@user.rss`, `/tags/<tag>.rss`). | Gratis; respetar normas de cada instancia | ✅ `kind = "mastodon"` (probado con `mastodon.social/tags/rust.rss`). |

Captura manual universal: `sentri add URL` detecta la plataforma por dominio
(incluidas instancias Mastodon por la forma `/@usuario/123`), intenta leer
Open Graph/JSON-LD (título, descripción, imagen, autor) y, si la plataforma
bloquea la página, **guarda igualmente la URL** con avisos. Después se puede
completar todo desde la TUI (`E` en Details) o con
`sentri edit ID --set source=instagram --set author_handle=@cuenta`.
Nunca se usan cookies ni credenciales de cuentas personales.

## Noticias tecnológicas (probadas el 2026-11-13)

| Fuente | Mecanismo | Resultado |
|---|---|---|
| Hacker News | API pública de Algolia (`hn.algolia.com/api/v1`), sin clave | ✅ portada, búsquedas y empleos |
| Google News | RSS de búsqueda (`news.google.com/rss/search?q=…`) | ✅ 100 ítems por consulta. Solo lectura personal no comercial; los enlaces pasan por `news.google.com`. El medio aparece al final del titular. |
| Ars Technica, The Verge, TechCrunch, Phoronix, LWN, BleepingComputer | RSS | ✅ |
| Blogs oficiales: Rust, Go, GitHub, OpenAI | RSS/Atom | ✅ (`stance = "official"`) |
| Blog de NVIDIA | RSS | ❌ el parser (`feed-rs`) falla con este feed; desactivado. |
| Reddit | — | ❌ RSS retirado el 13-nov-2026 (ver arriba). |

### Naturaleza de las noticias

SENTRI **no verifica hechos** y por eso no existe la etiqueta "verificada":

| Etiqueta | Cuándo |
|---|---|
| Reported | Por defecto: lo publica un medio; se atribuye a la fuente. |
| Official | La fuente es el blog oficial de la empresa/proyecto (configurado). |
| Unconfirmed | El titular o el resumen usa "reportedly", "rumor", "leak", "allegedly", "sources say", "según fuentes"… Tiene prioridad sobre las demás. |
| Opinion | El titular indica opinión, editorial o columna. |
| Discussion | Hilos comunitarios (Ask/Show HN, fuentes sociales configuradas así). |

## Eventos, hackathons y competiciones (probado el 2026-11-16)

| Fuente | Método | Autenticación | Estado en SENTRI |
|---|---|---|---|
| **Luma** | iCal por calendario (`webcal://api.lu.ma/ics/get?entity=calendar&id=cal-…`, botón *Add iCal Subscription*). Endpoint no documentado oficialmente pero ofrecido por Luma para suscribirse. La API pública (`public-api.luma.com`) es solo para calendarios propios con Luma Plus. | No | ✅ `kind = "luma"` (probado: 190 eventos, 2 futuros). Páginas de evento con JSON-LD → `sentri add` funciona. |
| **Meetup** | iCal por grupo `https://www.meetup.com/<grupo>/events/ical/`. La API GraphQL requiere OAuth y Meetup Pro. | No (iCal) | ✅ `kind = "meetup"` (probado con Rust NYC; zonas horarias con horario de verano resueltas). |
| **GDG / Bevy** (`gdg.community.dev`) | Sin API pública ni feed; `robots.txt` prohíbe `/api/`. La página del capítulo enlaza eventos y cada página de evento publica JSON-LD `Event`. | No | ✅ `kind = "gdg"`: 1 página de capítulo + máx. 10 páginas de evento por ejecución. GDG Cusco probado (hoy solo tiene eventos pasados; se descartan). |
| **Devpost** | `devpost.com/api/hackathons` (JSON usado por la propia web, **no documentado**). `robots.txt` lo permite y los Términos no prohíben el acceso automatizado (solo la automatización de votos). Los feeds `.atom`/`.rss` devuelven 406. | No | 🟡 `kind = "devpost"`, **desactivado por defecto**; probado (20 hackathons con plazo, premios, temas y organizador). Páginas de hackathon con JSON-LD `Hackathon` → `sentri add`. |
| **HackerRank** | `robots.txt` prohíbe `/rest/` y `/contests/` a bots. Sin API pública de concursos. | — | ❌ Solo `sentri add` (se guarda la URL; completar con `E`). |
| **Kaggle** | API oficial `GET /api/v1/competitions/list` (401 sin credenciales). | Sí: `username:key` de `kaggle.json` o token de API | 🟡 `kind = "kaggle"` con `token_env`; probado con servidor simulado, **no contra la API real** (sin credenciales). |
| **Eventbrite** | La búsqueda pública de la API se retiró (`/v3/events/search/` → 404, comprobado). `robots.txt` bloquea `/rss/` y `/atom/`. La API solo lista eventos de organizadores con token privado. | Sí | ❌ Sin conector; `sentri add` con la URL del evento. |

### iCal genérico

`kind = "ical"` acepta cualquier `.ics` o `webcal://` (universidades, comunidades,
calendarios públicos de Google). Soporta líneas plegadas, escapes, fechas UTC,
`TZID` con `VTIMEZONE` (reglas anuales de horario de verano), zonas sin DST
conocidas (America/Lima, Bogotá…) y fechas de día completo. Descarta eventos
cancelados, pasados y sin URL pública (nunca inventa enlaces).
