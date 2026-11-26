# Despertar del Observer y onboarding

## El Observer

Al abrir `sentri`, un *ward* observador en 3D dibujado con caracteres (diseño
propio, sin imágenes ni protocolos gráficos) despierta mientras SENTRI se
inicia: una cabeza con un ojo en forma de lente en ambas caras, dos alas con
plumas barridas hacia atrás, una cresta y un bastón con un núcleo de energía
sobre una base. El modelo se muestrea como puntos con normales; cada fotograma
lo gira 360° sobre su eje vertical, lo proyecta con perspectiva y búfer de
profundidad, y elige el carácter según la luz (`.,:;=+*#%`), así que la forma
se mantiene estable entre fotogramas (nada es aleatorio).

El despertar: el ward arranca quieto y oscuro, acelera hasta girar una vuelta
cada ~5 s y se ilumina poco a poco. El ojo se abre por etapas (cerrado,
entreabierto, medio, abierto) **sin adelantarse nunca al progreso real**; con
el ojo entreabierto aparecen anillos de energía alrededor del núcleo, con el
ojo a medias un radar barre el suelo, y despierto del todo pulsa el iris y
flotan más partículas. Colores Gruvbox Dark: amarillo y naranja en la cabeza,
el ojo y el bastón; verde y aqua en las alas, los anillos y el radar.

La pantalla es una **consola de observación**: una franja superior con el
estado del Observer (dormido, despertando, despierto · escaneando); a la
izquierda **INITIALIZATION** con cada paso y lo que encontró; en el centro el
Observer con la marca **SENTRI** y el lema *THE WORLD IS FULL OF
OPPORTUNITIES*; a la derecha **EVENT LOG**, con lo que realmente ocurrió y en
qué momento (terminal, configuración, ubicación, ámbitos, estado de cada fuente
según la última recolección, perfil de GitHub, dashboard); y abajo una barra
con el progreso, la ubicación, los ámbitos local · national · global y cómo
saltar. Los paneles laterales usan el fondo oscuro de Gruvbox en lugar de
bordes. En ventanas altas y estrechas el registro pasa debajo de los pasos para
dar más espacio al Observer; con menos espacio se oculta el registro, luego se
usa el ward plano de la fase 12 y, al final, solo la lista de pasos.

Los pasos se marcan solo cuando termina el trabajo real:

| Paso | Se completa cuando… |
|---|---|
| Initializing Observer | la terminal está lista y se eligió la paleta (truecolor o 256 colores). |
| Detecting terminal | terminó la detección de terminal, tmux y protocolo de imágenes. |
| Loading configuration | se leyó `config.toml` (o se usan los valores por defecto). |
| Loading user profile | se cargó el perfil de GitHub (o se confirmó que no hay usuario). |
| Preparing sources | se leyó el estado de las fuentes configuradas. |
| Preparing dashboard | el feed quedó ordenado según tu perfil. |

Marcas: `✓` hecho, `↻` sigue en segundo plano, `✗` falló, `◌` pendiente (un
spinner mientras está animado).

- **No retrasa el arranque.** El dashboard se carga detrás desde el primer
  instante. GitHub tiene 1,5 s de margen; pasado ese tiempo continúa en segundo
  plano (`↻`) y, si termina mientras la pantalla sigue visible, el paso pasa a
  `✓` y el registro lo anota. Cualquier tecla salta la pantalla al momento.
- **Ritmo.** `[onboarding] startup_style = "cinematic"` (por defecto): unos 7 s;
  el ojo avanza una etapa por segundo como mucho y nunca antes que el progreso
  real, luego el Observer escanea hasta completar el tiempo. `"brief"`: unos
  1,5 s. También en **Settings › Appearance › Startup pace**. La animación
  consume ~2 % de CPU (compilación release, 12,5 fotogramas por segundo).
- **Modos.** Animado por defecto (12,5 fotogramas por segundo, cada 80 ms).
  Estático, un solo fotograma en vista de tres cuartos redibujado solo cuando
  cambia el progreso, si `SENTRI_NO_ANIMATION` o `CI` están definidas o
  `TERM=dumb`. Sin pantalla si la salida no es una terminal o si
  `[onboarding] startup_animation = false` (también en **Settings ›
  Appearance › Startup animation**). Se adapta al redimensionar.
- **Welcome del asistente.** La primera página muestra el Observer girando
  junto a la marca, un saludo, qué hace SENTRI y `Enter` para empezar; sin
  animación (modo estático) queda quieto, y en paneles estrechos solo el texto.
  Todas las páginas comparten la cabecera de sección y una barra de pasos con
  los nombres (o puntos si no caben).
- **Sin Unicode.** Con `ui.ascii_icons = true`, un locale que no es UTF-8 o la
  consola de Linux (`TERM=linux`), el dibujo, la marca y las marcas son ASCII.
- **Vista previa.** `cargo run -p sentri-tui --example observer_frames -- 60 26 8`
  imprime ocho fotogramas de la rotación en texto plano (ancho, alto, número).
- **Terminales pequeñas:** desde 76×22 hay dos columnas (pasos y Observer);
  por debajo, el ward plano con la lista de pasos (44×21) o solo la lista.

`sentri` sin terminal interactiva (por ejemplo en un script o con la salida
redirigida) no abre la interfaz y sugiere los comandos equivalentes
(`sentri feed`, `sentri collect`).

## Onboarding

### Cuándo aparece

- **Primera ejecución:** sin `config.toml`, o con la plantilla de
  `sentri config init`, que trae `[onboarding] completed = false`.
- **Usuarios existentes:** un `config.toml` sin sección `[onboarding]` cuenta
  como completado; nunca se fuerza el asistente.
- **Para repetirlo:** `O` en **Settings** o `sentri onboarding` (lo abre en el
  próximo arranque). Las respuestas guardadas aparecen precargadas.

### Páginas

1. **Bienvenida.** `Enter` empieza; `Esc` lo salta por esta vez (vuelve en el
   próximo arranque).
2. **GitHub** (opcional). Se valida el formato del usuario y, en segundo
   plano, se consulta su perfil público con el mismo cliente y caché que la
   card de perfil. Si no existe o GitHub no responde, se avisa y el usuario se
   guarda igual; se corrige en Settings. **Nunca se pide un token.**
3. **Ubicación.** Código de país ISO 3166-1 alfa-2 (`PE`, `MX`, `ES`…; se
   muestra el nombre), ciudad, región/estado y zona horaria (por defecto la del
   sistema, `TZ` o `/etc/localtime`; `←`/`→` recorre zonas comunes). Todo es
   opcional.
4. **Dónde buscar.** Local, nacional, internacional, remoto, híbrido y
   presencial.
5. **Intereses.** Eventos, cursos, hackathons, becas, competencias,
   comunidades, open source, CFPs, noticias tech y empleos/prácticas
   (selección múltiple; `a` marca o desmarca todos).
6. **Resumen.** `Enter` guarda y abre el dashboard personalizado.

Teclas: `Enter` continuar, `Esc` atrás (nunca escribe), `↑`/`↓`/`Tab` moverse,
`Espacio` marcar, `Ctrl-C` salir.

### Guardado e interrupciones

Cada `Enter` valida la página y la guarda con el mismo escritor que Settings:
conserva tus comentarios, valida el documento completo antes de escribir y
escribe en un fichero temporal que luego renombra, así que una interrupción
no deja un `config.toml` a medias. Se hace **una copia de seguridad** antes
del primer cambio de cada sesión. Junto a las respuestas se guarda
`onboarding.step`; si sales a mitad, el asistente continúa en esa página la
próxima vez.

## Personalización

El perfil alimenta el mismo motor de reglas de siempre
([ADR 0003](adr/0003-rule-based-relevance.md)); no hay un segundo motor de
recomendaciones. Cada regla añade puntos con una razón visible en **Details › SENTRI
recommendation** (y en `sentri show`):

| Regla | Puntos por defecto | Razón mostrada |
|---|---|---|
| Ciudad (`discovery.local`) | `scoring.home_city` = 15 | Takes place in your city |
| País (`discovery.national`) | `scoring.home_country` = 8 | Takes place in your country |
| Extranjero abierto a ti (`discovery.international`) | `scoring.international` = 4 | International opportunity open to you |
| Modalidad preferida | `scoring.preferred_modality` = 5 | Online matches your preferred modality |
| Interés elegido | `scoring.topic` = 8 | Matches your interest in Hackathons |
| Lenguajes de GitHub | palabras clave × prioridad media | Matches interest 'your GitHub languages: rust, go' |

- **Nada se oculta.** Desactivar un ámbito solo retira su bonificación. Las
  oportunidades internacionales y remotas siguen en el feed aunque elijas solo
  lo local; "online", "remote" o "virtual" no cuentan como extranjero.
- **Ubicación.** El código de país añade su nombre en inglés y el local
  (`PE` → "Peru", "Perú") a la comparación con el lugar de cada oportunidad;
  la región cuenta como tu país.
- **GitHub.** Con usuario configurado, SENTRI lee los lenguajes y *topics* de
  tus 30 repositorios públicos propios más recientes (sin forks), una vez al
  día, y los guarda en caché. El ranking solo lee esa caché: nunca espera a la
  red. Cuando cambian, el feed se reordena solo.
- **Intereses** usan las categorías existentes: por ejemplo *Events* agrupa
  `event`, `conference`, `workshop` y `meetup`; *Tech News* son las noticias.

La privacidad de estos datos se describe en [privacy.md](privacy.md).
