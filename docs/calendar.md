# Calendario

SENTRI tiene un **calendario propio** (tabla `calendar_entries`). Añadir algo
al calendario **no equivale a inscribirse**: solo registra fechas.

## Tipos de entrada

| Tipo | Origen |
|---|---|
| Event | `starts_at` de un evento, conferencia, meetup, hackathon… |
| Deadline | Fecha límite de postulación o inscripción |
| Course starts / Course ends | `starts_at` / `ends_at` de cursos y formaciones |
| Reminder | Creado a mano (`remind …`) |
| Personal | Creado a mano, sin oportunidad asociada |

`a` (TUI) o `sentri schedule ID` añade **todas** las fechas de una oportunidad.
Repetirlo no duplica nada (índice único por oportunidad y tipo).

## Fechas sin hora

Una fecha sin hora (`2026-11-21`) se guarda como medianoche UTC y se muestra
siempre en **ese mismo día**, sin convertirla a la hora local (en Perú, UTC-5,
caería la tarde anterior). Ver `sentri_core::dates`.

## TUI

| Tecla | Acción |
|---|---|
| `←` `→` `h` `l` | Día anterior / siguiente |
| `↑` `↓` `k` `j` | Semana (vista mes) o día |
| `<` `>` | Periodo anterior / siguiente |
| `t` | Hoy |
| `v` | Vista: mes → semana → día → agenda |
| `J` `K` | Elegir entrada del día |
| `Enter` | Detalles de la oportunidad |
| `a` | Añadir una sugerencia al calendario |
| `+` | Nueva entrada: `18:00-20:00 Rust meetup`, `remind Enviar CV`, `Estudiar` (todo el día) |
| `e` | Editar entrada propia |
| `x` | Borrar entrada propia (pide confirmación) |
| `g` | Sincronizar con Google (hoy informa que no está conectado) |

Las entradas sólidas (`▪`) están en tu calendario; las tenues (`·`) son fechas
de oportunidades aún no añadidas.

## Preferencias

`[calendar] week_start` (`monday`/`sunday`) y `default_view`
(`month`/`week`/`day`/`agenda`), editables en Settings y aplicadas al instante.

## Google Calendar (pendiente)

**No está conectado.** `sentri sync` y `g` lo dicen explícitamente; nada se
simula. `sentri calendar-doctor` comprueba lo que ya puedes preparar: que el
fichero OAuth exista, tenga permisos `600`, sea JSON válido y sea un cliente
*Desktop app* (nunca muestra el secreto), y lista los pasos siguientes. Política ya implementada (`sentri_core::calendar::sync_policy`) para
cuando se conecte:

| Caso | Sincronización |
|---|---|
| Presencial en tu ciudad (Cusco) | Automática |
| Virtual | Automática |
| Presencial internacional / viajes | Manual (confirmación) |
| Sin fecha concreta | Nunca |

Preparación prevista (no inventamos credenciales):

1. Crear un proyecto en Google Cloud Console y habilitar *Google Calendar API*.
2. Crear credenciales OAuth 2.0 de tipo *Desktop app*.
3. Guardar el JSON en `~/.config/sentri/google-oauth.json` con permisos `0600`.
4. Scope mínimo: `https://www.googleapis.com/auth/calendar.app.created`
   (solo calendarios creados por la app, *verificar*), para crear el calendario
   `SENTRI Opportunities` sin tocar los personales.
5. `google_event_id` ya existe en la base para evitar duplicados y conservar
   el identificador de sincronización.
