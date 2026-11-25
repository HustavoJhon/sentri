# TUI

```sh
sentri            # abre la TUI
```

## Arranque y onboarding

Mientras SENTRI se inicia, el Observer despierta y muestra los pasos reales
de inicialización; cualquier tecla lo salta. En la primera ejecución se abre
después un asistente de seis páginas (GitHub, ubicación, ámbitos e intereses)
que guarda cada página al continuar y retoma donde lo dejaste. `O` en Settings
lo repite. Detalle en [onboarding.md](onboarding.md).

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
- 60–79 columnas: una barra estrecha con la tecla y el icono de cada sección.
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
| `z` | ver la imagen en grande (`Esc` cierra) |
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
| Dashboard | Métricas, descubiertas por día (barras), likes/guardados (sparklines), distribución por categoría/fuente/modalidad, recomendaciones, próximas fechas, últimas noticias y empleos (abiertos, remotos, con salario, alta coincidencia). Widgets, orden, métricas y diseño se configuran en `[dashboard]` |
| Discover | Oportunidades (no noticias). Buscar desde aquí busca en todo |
| Tech News | Noticias con naturaleza (Reported, Official, Unconfirmed, Opinion, Discussion), tema, medio y autor |
| Jobs … Travel | Vistas por categoría |
| Saved, Likes, Applications | Listas personales (oportunidades y noticias) |
| Calendar | Mes, semana, día y agenda ([calendar.md](calendar.md)) |
| History | Mapa de actividad anual, rachas y resumen por día (`h`/`l` día, `H`/`L` semana, `t` hoy) |
| Sources | Gestor: añadir/editar/activar/borrar/refrescar fuentes, último éxito y último error ([sources.md](sources.md)) |
| Settings | Formulario editable ([configuration.md](configuration.md)); la cabecera indica cómo se dibujan las imágenes en esta terminal y por qué (`sentri terminal` da el detalle). Los cambios sin guardar se marcan con `●` en la barra lateral y salir con `q` pide confirmación (`s` vuelve a Settings, `y` sale sin guardar). `O` repite el onboarding con tus respuestas precargadas |

## Cards

Todas las secciones comparten la misma estructura de card (rail, kicker,
título, datos, estado), pero cada tipo ordena sus datos a su manera:

```text
┃ JOB         GB Get on Board                                ●●●●○
┃ Senior Rust Engineer · Acme
┃ USD 3500–4000 / month · ◎ remote · Lima · posted 3d ago · rust sql

│ MEETUP      PE Peruanos.dev                                ●●●○○
│ Rust Lima #12
│ ◷ Sat 28 Nov 18:30 · in 8d · Rust Lima · Miraflores · ⌂ in person

│ COMMUNITY   PE Peruanos.dev                              ♥ ●●●○○
│ GDG Huancayo
│ ⌂ Huancayo · ↗ Website GitHub LinkedIn · cloud · android
```

| Tipo | Datos |
|---|---|
| Empleos y prácticas | salario, modalidad, lugar, antigüedad, cierre de postulación, habilidades |
| Eventos, meetups, talleres, conferencias | fecha y hora, organizador, lugar, modalidad, cierre de inscripción, costo |
| Cursos | proveedor, costo (gratis / certificado pagado / precio), certificado, modalidad, inicio, cierre de inscripción |
| Hackathons y concursos | cierre, premio (primera línea), viaje cubierto, modalidad, lugar, organizador |
| Comunidades | ciudad, enlaces (web, GitHub, redes), temas, próximo evento |
| Open source | organización, lenguajes/etiquetas, última actualización, enlaces |
| Call for Speakers | cierre del CFP, fecha del evento, organizador, lugar, modalidad, viaje |
| Tech News | tema, autor, fecha (el medio va en el kicker) |

1. **Kicker**: tipo (categoría, o la clasificación de la noticia: Reported,
   Official, Unconfirmed, Opinion, Discussion) y plataforma.
2. **Título**; en empleos va seguido de la empresa.
3. **Datos** según la tabla anterior.
4. **Estado** a la derecha: `NEW` (descubierto en 24 h y sin abrir), tus
   marcas (`♥` like, `★` guardado, `✓` postulado…), `closed` y el medidor de
   coincidencia `●●●○○` (cinco pasos de la puntuación de relevancia; en
   compact, el número).

La primera columna (**rail**) resume el estado: `▌` amarillo es la card
seleccionada, `┃` del color de la categoría marca lo importante (hackathons,
becas, empleos, premios, viajes, cursos gratuitos), `┃` verde un empleo con
alta coincidencia y `│` apagado lo vencido o descartado. En modo ASCII el rail
pasa a `>` y `|`, así que la selección no depende del color.

`v` alterna **compact** (1 línea, columnas alineadas), **comfortable** (3
líneas) y **detailed** (dos líneas más: la descripción, o una línea de
descripción y las habilidades en empleos y proyectos, o los enlaces en
comunidades). Qué datos se ven, su orden, el formato de las fechas, el estilo
de selección y la visibilidad de tipo, plataforma y medidor se configuran en
`[cards]` ([configuration.md](configuration.md#cards)). Las recomendaciones
del dashboard mezclan plataformas (máximo tres seguidas de la misma) para que
una bolsa de empleo activa no ocupe toda la lista.

## Details y editor

Cabecera con tipo, plataforma y tus marcas, título, organizador o autor (con
avatar si la fuente lo declara). Debajo, **Key facts** (fechas con su
distancia, lugar, modalidad, salario, costo, premios, lo que incluye, enlace),
**Overview** (la descripción con sus párrafos, viñetas y listas numeradas),
requisitos y elegibilidad, etiquetas y notas. Beneficios y premios de varias
líneas se alinean en su propia columna. En paneles anchos la imagen y la
recomendación de SENTRI van en una columna a la derecha; en estrechos, la
imagen arriba con un tercio de la altura como máximo. Los datos desconocidos
no ocupan filas. Se ven también campos editados frente a detectados e
historial. `E` abre el editor ([editing.md](editing.md)); en él, `¶` separa
párrafos y `↵` marca un salto de línea dentro del campo de una línea. La
lista de atajos al pie de Details usa tus teclas personalizadas y solo ofrece
`z` (ampliar imagen) cuando hay una imagen cargada.

Las descripciones se convierten desde el HTML de cada fuente conservando su
estructura (párrafos, `<br>`, listas con viñetas y numeradas) y se limpian de
caracteres de control y secuencias de escape antes de guardarse; nunca se
dibuja HTML. Al volver a recolectar, una descripción ya guardada solo se
reemplaza si es el mismo texto con su estructura recuperada, y una edición
manual nunca se sobrescribe.

Las imágenes usan Kitty, Sixel o iTerm2 si la terminal lo soporta y medios
bloques si no ([configuration.md](configuration.md#imágenes)).

## Perfil

Con `[github] username`, la parte inferior de la barra lateral muestra una
card de GitHub: avatar cuadrado (pedido a 460 px y remuestreado al tamaño
real), nombre, usuario, repositorios y seguidores (si caben), y tu ubicación,
empresa o bio públicas. Sin imagen se ven tus iniciales. En terminales bajas
la card se reduce a una fila, y en las de 60–79 columnas la barra estrecha no
la muestra.

## Badges de fuentes

`++` manual · `RS` RSS · `IC` iCal · `GH` GitHub · `HN` Hacker News ·
`r/` Reddit · `in` LinkedIn · `X` X · `LU` Luma · `GDG` GDG · `DP` Devpost ·
`MU` Meetup · `EB` Eventbrite · `KG` Kaggle · `MS` Microsoft Learn · `WW` web.

## Atajos personalizables y ASCII

Los atajos de acciones se cambian en **Settings › Shortcuts** o en `[keys]`
([configuration.md](configuration.md)); `?` muestra siempre las teclas
efectivas. `ascii_icons = true` dibuja toda la interfaz en ASCII.
