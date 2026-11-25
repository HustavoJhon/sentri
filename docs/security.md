# Seguridad

Todo contenido externo es **no confiable**: se descarga con límites, se
sanea antes de guardarse y nunca se ejecuta.

## Peticiones salientes (anti-SSRF)

`sentri-sources::SafeClient` es el único cliente HTTP del proyecto:

| Protección | Detalle |
|---|---|
| Esquemas | Solo `http` y `https`. Sin credenciales en la URL. |
| Direcciones | Solo IPs públicas. IPv4: se bloquean loopback, privadas, link-local (incl. `169.254.169.254` de metadatos cloud), CGNAT `100.64.0.0/10` (Tailscale), multicast, reservadas, documentación, relay 6to4. IPv6: **lista blanca** de `2000::/3` menos `2001::/23`, `2001:db8::/32`, `2002::/16` y `3fff::/20`; las IPv4-mapped se validan como IPv4. |
| DNS rebinding | Un resolver propio filtra las direcciones; la conexión usa solo las IPs ya validadas. |
| Literales IP | Se validan antes de conectar (incluye formas como `0x7f000001`). |
| Redirecciones | Máximo configurable; cada salto se valida igual que la URL inicial; nunca de https a http. Las peticiones con cabeceras de autenticación no siguen redirecciones, para no reenviar credenciales a otro origen. |
| Proxies | Se ignoran los proxies del sistema para que no eludan las comprobaciones. |
| Tamaño | `max_body_bytes` por respuesta, comprobado en streaming. |
| Tipo | Lista blanca de `Content-Type` (HTML, XML, feeds, JSON, iCal, texto). |
| Tiempo | Timeout total y de conexión. |
| Identidad | `User-Agent: sentri/<versión>`, sin datos personales. |

## Texto externo

`sentri_core::text::clean` elimina caracteres de control (incluido `ESC`, que
permitiría inyectar secuencias ANSI en la terminal), caracteres bidi/zero-width
y limita longitudes. Las URLs de imagen solo se aceptan con `https://`.

## Secretos

- Nunca en el repositorio ni en `config.toml`: la configuración solo guarda el
  **nombre** de la variable de entorno (`token_env`).
- Los tokens OAuth de Google se guardarán en `~/.local/share/sentri/` con
  permisos `0600` (fase 7); se evaluará el keyring del sistema.
- `.gitignore` excluye `*.db`, `.env`, `*.secret` y `secrets/`.
- Los logs nunca incluyen tokens ni cabeceras de autorización.

## Datos personales

- Base de datos y backups con permisos `0600` desde su creación; directorio `0700`.
- El perfil reutilizable para inscripciones (fase 10) es opcional campo a campo.
- Las respuestas del onboarding (GitHub, ubicación, ámbitos, intereses) viven
  solo en `config.toml`; únicamente el usuario de GitHub sale del equipo, hacia
  la API pública de GitHub y sin token. Detalle en [privacy.md](privacy.md).
- `config.toml` se escribe en un fichero temporal que luego se renombra, tras
  validar el documento completo: una interrupción nunca lo deja a medias.
- Ninguna acción externa (inscribirse, postular, aceptar términos, pagar,
  adjuntar documentos, publicar) se ejecuta sin confirmación explícita.

## Plataformas

Sin scraping de LinkedIn ni X, sin cookies de sesión de cuentas personales
(ver [ADR-0004](adr/0004-no-scraping-of-restricted-platforms.md)).

## Perfil de GitHub e IA

- La card de perfil nunca muestra direcciones IP: si la ubicación, la empresa
  o la bio públicas de GitHub contienen una, ese campo no se muestra.
- SENTRI no usa IA al ejecutarse y no envía datos a servicios de IA
  ([AI_USAGE.md](../AI_USAGE.md)).
