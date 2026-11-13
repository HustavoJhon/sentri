# ADR-0001: Monolito modular en un workspace Cargo

- Estado: aceptada
- Fecha: 2026-11-13

## Contexto

SENTRI necesita una TUI, una CLI, un daemon y en el futuro una API web que
compartan la misma lógica de negocio. Es un proyecto personal mantenido por una
persona.

## Decisión

Workspace Cargo con crates por responsabilidad (`core`, `storage`, `sources`,
`app`, `tui`, `cli`). Sin microservicios. Los crates `calendar`,
`applications`, `daemon` y `api` se añadirán cuando haya código real para ellos.

## Consecuencias

- Los límites entre capas los impone el compilador (dependencias explícitas).
- Compilación incremental más rápida que un crate único grande.
- Se evita crear crates vacíos "por si acaso".
