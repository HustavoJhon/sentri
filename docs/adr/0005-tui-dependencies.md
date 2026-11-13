# ADR-0005: Dependencias de la TUI

- Estado: aceptada
- Fecha: 2026-11-13

## Decisión

| Crate | Decisión | Motivo |
|---|---|---|
| `ratatui` 0.30 | ✅ | Base de la TUI. Se usa su reexport `ratatui::crossterm` para no duplicar versiones de Crossterm. |
| `crossterm` 0.29 | ✅ (vía ratatui) | Backend portable; funciona por SSH y en Termux. |
| `tui-input` | ❌ | Un campo de una línea propio (`input.rs`, ~100 líneas con tests) evita acoplarse a otra versión de Crossterm. |
| `tui-realm` | ❌ | Añade un framework de componentes que no aporta frente a un estado + `on_key → efectos` simple y testeable. |
| `tachyonfx` | ⏳ | Animaciones discretas en una fase posterior; no bloquean la funcionalidad. |
| `ratatui-image` | ⏳ | Requiere Kitty/Sixel/iTerm2. Los logos se representan con badges de texto (`GH`, `r/`, `in`…) que funcionan en cualquier terminal; se evaluará como mejora opcional. |

## Arquitectura de la TUI

- `state.rs`: estado y transiciones puras (`on_key`, `on_mouse`, `on_msg`)
  que devuelven `Effect`s. Cubierto por tests sin terminal.
- `ui/`: renderizado; tests con `TestBackend` en anchos 60/100/140.
- `runtime.rs`: hilo lector de eventos + tareas tokio para efectos (BD, red,
  navegador). La UI nunca se bloquea esperando la red.

## Compatibilidad

Solo Unicode estándar (sin Nerd Fonts). Truecolor si `COLORTERM` lo anuncia;
si no, paleta xterm-256. Ratón opcional (si la terminal lo soporta).
