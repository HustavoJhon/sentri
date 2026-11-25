# Despertar del Observer y onboarding

## El Observer

Al abrir `sentri`, un *ward* observador dibujado con caracteres (diseño propio,
sin imágenes ni protocolos gráficos) despierta mientras SENTRI se inicia. Su
ojo se abre a medida que terminan pasos **reales**; nada se marca como hecho
antes de tiempo:

| Paso | Se completa cuando… |
|---|---|
| Initializing Observer | la terminal está lista y se eligió la paleta (truecolor o 256 colores). |
| Detecting environment | terminó la detección de terminal, tmux y protocolo de imágenes. |
| Connecting opportunity sources | se leyó el estado de las fuentes configuradas. |
| Loading personal profile | se cargó el perfil de GitHub (o se confirmó que no hay usuario). |
| Preparing dashboard | el feed quedó ordenado según tu perfil. |

Marcas: `✓` hecho, `↻` sigue en segundo plano, `✗` falló, `◌` pendiente.

- **No retrasa el arranque.** GitHub tiene 1,5 s de margen; pasado ese tiempo
  continúa en segundo plano. Ningún paso retiene la pantalla más de 4 s, y
  cualquier tecla la salta (el trabajo sigue). Cuando todo termina, el ojo
  abierto queda visible ~0,45 s.
- **Modos.** Animado por defecto (un fotograma cada 90 ms). Estático, sin
  temporizador de animación, si `SENTRI_NO_ANIMATION` o `CI` están definidas o
  `TERM=dumb`. Sin pantalla si la salida no es una terminal o si
  `[onboarding] startup_animation = false` (también en **Settings ›
  Appearance › Startup animation**).
- **Sin Unicode.** Con `ui.ascii_icons = true`, un locale que no es UTF-8 o la
  consola de Linux (`TERM=linux`), el dibujo y las marcas son ASCII.
- **Terminales pequeñas** (menos de 21 filas o 40 columnas): se omite el dibujo
  y quedan los pasos.

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
