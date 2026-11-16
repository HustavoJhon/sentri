# ADR-0006: Imágenes con half-blocks propios

- Estado: aceptada
- Fecha: 2026-11-15

## Contexto

Details y el avatar de GitHub muestran imágenes. Deben funcionar en tmux, por
SSH y desde Android, y la descarga nunca debe bloquear el renderizado.

## Opciones

| Opción | Pros | Contras |
|---|---|---|
| `ratatui-image` (Kitty/Sixel/iTerm2/half-block) | Calidad alta en terminales gráficas | Última versión en RC (12.0.0-rc.3); protocolos gráficos fallan en tmux/SSH/Termux; más dependencias |
| Half-block propio (`▀` fg/bg) | Funciona en cualquier terminal con color; ~150 líneas con tests | Resolución 1×2 píxeles por celda |
| ASCII en escala de grises | Funciona sin colores | Poco detalle |

## Decisión

Crate `image` solo con decodificadores PNG/JPEG/WebP/GIF y un widget
half-block propio, con modo ASCII y `off` (`[ui] images`). Truecolor o
paleta xterm-256 según la terminal. Se reevaluará `ratatui-image` cuando
tenga versión estable, como modo opcional.

## Seguridad

Descarga vía `SafeClient` (anti-SSRF), lista blanca de MIME de imagen (sin
SVG), máximo 3 MB, decodificación con límites (8192 px, 128 MB) en un hilo
bloqueante, caché en `~/.cache/sentri/images` con nombres opacos y permisos
`0600`.
