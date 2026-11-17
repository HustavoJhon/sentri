# ADR-0009: Ecosistema tech peruano (Peruanos.dev, GDG y hack0.dev)

- Estado: aceptada
- Fecha: 2026-11-17

## Contexto

SENTRI ya integraba RSS/Atom, GitHub, GDG, Luma/Meetup (iCal), Devpost y
Kaggle, pero no el ecosistema peruano. El objetivo era cubrir eventos,
comunidades, proyectos open source y Call for Speakers (CFS) de Perú sin
duplicar lo que ya existía y sin scrapear plataformas que lo prohíben
([ADR-0004](0004-no-scraping-of-restricted-platforms.md)).

## Decisión

### Peruanos.dev como fuente principal

Se usa la API JSON pública `https://www.peruanos.dev/api/{events,communities,projects,cfs}`
(sin autenticación, sin escritura). Un solo conector `kind = "peruanos"` con
`target` selecciona el endpoint; las cuatro fuentes son independientes, de modo
que un cambio en un endpoint no rompe las demás.

- Los eventos se normalizan a `Opportunity` con título, descripción,
  organizador, inicio, ciudad/venue, modalidad, etiquetas, URL de registro y URL
  original. No hay fecha de fin, zona horaria, precio ni moneda: la hora se
  interpreta como hora estándar del Perú (UTC-5, sin horario de verano) y el
  coste queda desconocido en lugar de inventarse.
- Las comunidades usan una categoría propia (`Community`) y una vista separada;
  el sitio web es la URL original y guardar equivale a seguir.
- Los proyectos son objetos de repositorio de GitHub REST: se reutiliza el
  parser del conector GitHub y `html_url` como identidad canónica, así que el
  mismo repositorio no se importa dos veces.
- Los CFS usan `cfs_url` como URL original, `deadline` como fecha límite y
  `event_date` como fecha del evento. Se conservan aunque el plazo haya vencido
  y se ordenan por fecha límite.
- Las fechas sin hora se tratan como el final del día peruano (los plazos) o
  como medianoche peruana (los eventos). Las fechas inválidas se dejan
  desconocidas; nunca se descarta un elemento por un campo opcional ausente.

### GDG reutilizado

No se reimplementa GDG: los capítulos de Lima y Cloud Lima se añaden como
instancias del conector existente, que lee páginas públicas y su JSON-LD.
`gdg.community.dev/api/` está prohibido por `robots.txt`.

### hack0.dev con límites conservadores

hack0.dev no ofrece API, RSS ni iCal públicos y `/api/` está prohibido. El
conector lee solo el listado público `/eventos` y hasta 10 páginas de evento por
ejecución, extrae el JSON-LD `Event` y conserva únicamente eventos de Perú
(`addressCountry` peruano o una ciudad peruana conocida cuando falta el país).
Si el marcado cambia, se deja de importar y se recurre a `sentri add`; no se
adivina ni se amplía el scraping.

### Deduplicación y atribución

La identidad lógica se resuelve por URL canónica (incluida la URL de
registro/oferta), huella de noticia y `(source, external_id)`. Al fusionar, cada
fuente queda en `opportunity_sightings` con su URL, identificador y fechas de
primera/última observación. Los detalles muestran todas las fuentes. Las
correcciones manuales viven en `opportunity_overrides` y la recolección nunca
las sobrescribe.

### TUI

Se añaden las vistas *Communities* y *Call for Speakers*. Eventos y proyectos
usan las vistas *Events* y *Open Source* existentes. Los filtros rápidos
permiten modalidad (`M`), ciudad (`L`, empezando por la ciudad del perfil) y
pasados (`H`), manteniendo los existentes. Call for Speakers ordena por fecha
límite al entrar. Se reutilizan los componentes, el tema Gruvbox y los
comandos de la paleta.

## Consecuencias

- La integración con Peruanos.dev es de solo lectura y no requiere credenciales.
- No se promete cobertura total: los eventos actuales de Peruanos.dev son
  presenciales y los CFS pueden estar vencidos. Los filtros de modalidad y
  ciudad funcionan sobre los datos disponibles.
- hack0.dev puede dejar de aportar si cambia su HTML; el resto de fuentes sigue
  funcionando. Igual ocurre con cualquier endpoint de Peruanos.dev.
- Los validadores condicionales solo existen en `projects`; el resto se
  consulta una vez por ejecución (por defecto cada 120 minutos), con timeout,
  backoff y aislamiento por fuente ya existentes.
- Probar contra la API real requiere red; las pruebas automáticas usan respuestas
  recortadas y un servidor mock.
