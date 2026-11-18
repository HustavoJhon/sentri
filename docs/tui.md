# TUI

```sh
sentri            # abre la TUI
```

## Primer arranque y recolección

Al abrir la TUI con la base vacía, SENTRI recolecta enseguida en segundo plano
(`[ui] collect_on_start = "auto"`): una pantalla de bienvenida muestra cuántas
fuentes respondieron y cuántos elementos nuevos llegaron, y cada sección se
llena a medida que responde cada fuente, sin esperar a la más lenta. Si los
datos tienen más de `daemon.interval_minutes`, se refrescan del mismo modo. Las
fuentes se consultan de cuatro en cuatro; el progreso aparece en la cabecera
(`⟳ collecting 7/31 · +117`) y en la barra de estado. `R` lanza una recolección
manual con el mismo progreso.

Mientras la TUI está abierta, los avisos de registro no se escriben en la
terminal: van a `sentri.log`, junto a la base de datos (permisos `600`).
`SENTRI_LOG=debug` aumenta el detalle.

## Distribución

- ≥ 110 columnas: barra lateral · feed · detalles.
- ≥ 80 columnas: barra lateral · feed (Enter abre los detalles).
- Más estrecho (móvil por SSH): solo feed; `Tab` cambia de sección.

## Atajos

| Tecla | Acción |
|---|---|
| `j` `k` `↓` `↑` | mover |
| `g` `G` | primero / último |
| `Enter` | abrir detalles (registra en el historial) |
| `Esc` | cerrar detalles / limpiar búsqueda |
| `Tab` `⇧Tab` `[` `]` | sección siguiente / anterior |
| `1`…`9` `0` | saltar a sección |
| `←` `→` | foco barra lateral / lista / detalles |
| `l` | like |
| `s` | guardar |
| `d` | descartar |
| `i` | interesante |
| `p` `e` `c` | postulado / inscrito / completado |
| `a` | agregar al calendario (fase 7; hoy muestra aviso) |
| `o` | abrir enlace original (`xdg-open`; por SSH muestra la URL) |
| `n` | añadir enlace |
| `/` | buscar |
| `:` `Ctrl-p` | paleta de comandos |
| `S` | cambiar orden (relevancia → recientes → deadline) |
| `F` `T` `U` `P` `C` | filtros: gratis, viaje, próximos, abierto a tu país, certificado |
| `E` | editar todos los campos, notas y campos personalizados |
| `v` | densidad de cards: compact / comfortable / detailed |
| `r` | recargar |
| `R` | recopilar ahora de las fuentes habilitadas |
| `?` | ayuda |
| `q` `Ctrl-c` | salir |

Ratón: rueda para desplazarse, clic para seleccionar, segundo clic abre detalles.

## Secciones

La barra lateral muestra cuántos elementos tiene cada sección, y la barra de
estado los atajos de la sección actual (con tus atajos personalizados).

| Sección | Contenido |
|---|---|
| Dashboard | Contadores, descubiertas por día (barras), likes/guardados (sparklines), distribución por categoría/fuente/modalidad, recomendaciones, próximas fechas, últimas noticias |
| Discover | Oportunidades (no noticias). Buscar desde aquí busca en todo |
| Tech News | Noticias con naturaleza (Reported, Official, Unconfirmed, Opinion, Discussion), tema, medio y autor |
| Jobs … Travel | Vistas por categoría |
| Saved, Likes, Applications | Listas personales (oportunidades y noticias) |
| Calendar | Mes, semana, día y agenda ([calendar.md](calendar.md)) |
| History | Mapa de actividad anual, rachas y resumen por día (`h`/`l` día, `H`/`L` semana, `t` hoy) |
| Sources | Gestor: añadir/editar/activar/borrar/refrescar fuentes, último éxito y último error ([sources.md](sources.md)) |
| Settings | Formulario editable ([configuration.md](configuration.md)) |

## Cards

`v` alterna **compact** (1 línea), **comfortable** (2) y **detailed**
(con descripción). `◆` marca hackathons, becas, empleos, premios, viajes y
cursos gratuitos; `NEW` marca lo descubierto en 24 h y aún no abierto. Las
cards muestran además la organización o empresa, el salario publicado de los
empleos, el día de la semana de los eventos y hace cuánto se publicó un empleo.
Las recomendaciones del dashboard mezclan plataformas (máximo tres seguidas de
la misma) para que una bolsa de empleo activa no ocupe toda la lista.

## Details y editor

Cabecera con plataforma, autor/usuario y avatar si la fuente lo declara;
imagen (half-blocks) si existe; fechas separadas (evento, deadline, inicio,
fin); notas; recomendación de SENTRI con barras; campos editados frente a
detectados; historial. `E` abre el editor ([editing.md](editing.md)).

## Perfil

Con `[github] username`, la parte inferior de la barra lateral muestra tu
avatar, nombre, usuario y repositorios públicos.

## Badges de fuentes

`++` manual · `RS` RSS · `IC` iCal · `GH` GitHub · `HN` Hacker News ·
`r/` Reddit · `in` LinkedIn · `X` X · `LU` Luma · `GDG` GDG · `DP` Devpost ·
`MU` Meetup · `EB` Eventbrite · `KG` Kaggle · `MS` Microsoft Learn · `WW` web.

## Atajos personalizables y ASCII

Los atajos de acciones se cambian en **Settings › Shortcuts** o en `[keys]`
([configuration.md](configuration.md)); `?` muestra siempre las teclas
efectivas. `ascii_icons = true` dibuja toda la interfaz en ASCII.
