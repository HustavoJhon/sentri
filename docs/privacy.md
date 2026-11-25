# Privacidad del perfil

SENTRI no tiene servidor ni cuenta: todo lo que respondes en el onboarding se
queda en tu equipo.

## Qué se guarda y dónde

| Dato | Dónde | Notas |
|---|---|---|
| Usuario de GitHub, código de país, país, ciudad, región, zona horaria | `config.toml` (`[github]`, `[profile]`) | Texto plano, editable a mano o en Settings. Los campos opcionales pueden quedar vacíos. |
| Ámbitos (local, nacional, internacional), modalidades e intereses | `config.toml` (`[discovery]`, `profile.preferred_modalities`) | |
| Progreso del onboarding | `config.toml` (`[onboarding]`) | `completed`, `step` y `startup_animation`. |
| Copias de seguridad | `config.toml.bak-AAAAMMDD-HHMMSS`, junto al original | Una antes del primer cambio de cada sesión del asistente y una por cada guardado en Settings. Bórralas cuando quieras. |
| Perfil público de GitHub | `~/.cache/sentri/github-<usuario>.json` | Respuesta pública de `GET /users/<usuario>`, 24 h. |
| Lenguajes y *topics* de GitHub | `~/.cache/sentri/github-<usuario>-signals.json` | Solo los nombres de lenguajes y *topics* más usados, no la lista de repositorios. |
| Avatar | `~/.cache/sentri/images/` | |

Nada de esto se escribe en la base de datos ni en `sentri.log`. Los registros
no incluyen tu ubicación, tu usuario ni las URLs de tu perfil; los errores de
GitHub se registran sin detalle.

## Qué se envía a terceros

Solo a **GitHub**, y solo si escribes un usuario:

- `GET https://api.github.com/users/<usuario>`: al comprobarlo en el
  onboarding y, como mucho, una vez al día para la card de perfil.
- `GET https://api.github.com/users/<usuario>/repos?type=owner&sort=pushed&per_page=30`:
  como mucho una vez al día, para los lenguajes.
- La descarga del avatar desde el dominio de imágenes de GitHub.

Son peticiones anónimas a datos públicos: **sin token, sin login, sin
cookies**. GitHub ve tu dirección IP y el usuario consultado, como en
cualquier visita a un perfil público.

**Tu país, ciudad, región, zona horaria, ámbitos e intereses no se envían a
ningún sitio**: solo se usan en tu equipo para ordenar el feed. Las fuentes
que consulta SENTRI son las de `config.toml`, con las mismas peticiones que
sin perfil.

SENTRI no usa IA al ejecutarse ni envía datos a servicios de IA
([AI_USAGE.md](../AI_USAGE.md)).

## Cambiar o borrar

- **Settings › Profile / Discovery** edita cada campo; vaciar un campo de
  texto lo elimina del fichero.
- `O` en Settings o `sentri onboarding` repite el asistente.
- Para borrar todo: quita esas claves de `config.toml`, borra las copias
  `config.toml.bak-*` y los ficheros `github-*.json` de `~/.cache/sentri/`.
