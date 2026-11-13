# ADR-0002: SQLite con SQLx (consultas en tiempo de ejecución)

- Estado: aceptada
- Fecha: 2026-11-13

## Decisión

- SQLite embebido (`sqlite-bundled`) en modo WAL: un único fichero, fácil de
  copiar al VPS y de respaldar.
- Migraciones versionadas en `crates/sentri-storage/migrations`, embebidas en el
  binario con `sqlx::migrate!` y aplicadas al arrancar.
- Se usan `sqlx::query`/`query_as` en tiempo de ejecución en lugar de las macros
  `query!` para no exigir `DATABASE_URL` ni ficheros `.sqlx` al compilar. Cada
  repositorio está cubierto por tests de integración contra una base temporal.

## Consecuencias

- Sin verificación de SQL en compilación: los tests de repositorio la sustituyen.
- Si en el futuro se migra a PostgreSQL, el cambio queda confinado a `sentri-storage`.
