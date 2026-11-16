# ADR-0008: Settings de la TUI escritos en `config.toml`

- Estado: aceptada
- Fecha: 2026-11-15

## Decisión

Settings edita el mismo `config.toml` con `toml_edit` (mismo proyecto que
`toml`; comparte `toml_datetime`), que conserva comentarios y orden.
Antes de escribir: se valida cada valor y se carga el documento completo con
el cargador normal; si algo falla no se toca el fichero. Cada guardado crea
`config.toml.bak-AAAAMMDD-HHMMSS`. "Restore defaults" escribe la plantilla
comentada, también con backup.

## Consecuencias

- Una sola fuente de verdad, editable a mano o desde la TUI.
- Apariencia se aplica en caliente; perfil, scoring y recopilación tras
  reiniciar (la TUI lo indica con `restart`).
- Atajos de teclado: consultables (`?`); personalizarlos queda pendiente.
