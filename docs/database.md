# Base de datos

SQLite embebido, un único fichero (`~/.local/share/sentri/sentri.db` por
defecto) en modo WAL.

## Migraciones

- Ficheros versionados en `crates/sentri-storage/migrations/NNNN_descripcion.sql`.
- Se embeben en el binario (`sqlx::migrate!`) y se aplican al abrir la base.
- SQLx guarda un checksum de cada migración aplicada: **nunca** se edita una
  migración publicada; los cambios van en una nueva.
- Cada migración queda verificada por los tests de `sentri-storage`, que crean
  bases nuevas desde cero.

## Modelo

| Tabla | Contenido |
|---|---|
| `opportunities` | Oportunidad normalizada. `fingerprint` (SHA-256 de la URL canónica) es único. |
| `opportunity_tags` | Etiquetas normalizadas (N:M simple). |
| `opportunity_sightings` | Cada fuente donde se vio la oportunidad, con su `external_id` y URL original. |
| `interactions` | Estado actual: like, guardado, descartado, interesante, postulado, inscrito, completado. |
| `interaction_events` | Historial append-only de acciones. |
| `collection_runs` | Ejecuciones de cada conector: estado, contadores, error. |
| `sources` | Registro de fuentes configuradas. |

Tablas que llegarán con sus fases (en migraciones nuevas): `calendar_links`
(ID de evento de Google por oportunidad y tipo de fecha), `applications`
(postulaciones con estado), `learning_items` (progreso de cursos),
`profile_documents` (metadatos del CV, nunca el contenido en claro en logs).

## Deduplicación

1. Por `fingerprint` de la URL canónica (sin `utm_*`, `fbclid`, `www.`,
   fragmento, barra final; query ordenada).
2. Por `(source_name, external_id)` vía `opportunity_sightings`, para ítems
   cuyo enlace cambia dentro de la misma fuente.

Al fusionar, solo se completan campos desconocidos: un dato conocido (o una
corrección manual) nunca se sobrescribe con el de otra fuente.

## Fechas

Enteros con segundos Unix UTC: comparaciones y ordenación exactas.

## Backups

```sh
sentri backup ~/backups/sentri-$(date +%F).db
```

Usa `VACUUM INTO`, que produce una copia consistente aunque el daemon esté
escribiendo. El fichero de destino se crea con permisos `0600` y nunca se
sobrescribe. Para el VPS se recomienda un timer de systemd diario y cifrar la
copia si sale de la máquina (p. ej. `age`).

## Permisos

El directorio de datos se crea con `0700` y la base (incluidos `-wal`/`-shm`)
con `0600` desde el primer momento, porque contiene datos personales.
