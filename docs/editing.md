# Edición y datos manuales

## Tres capas

| Capa | Dónde | Quién la cambia |
|---|---|---|
| Original | `opportunity_sightings` (URL original, ID externo por fuente) | Cada recopilación |
| Detectado | `opportunity_detected` (snapshot normalizado, JSON) | Cada recopilación, fusionando: solo completa campos desconocidos |
| Manual | `opportunity_overrides` (campo → valor) | Solo tú |

La fila de `opportunities` guarda el **valor efectivo** (detectado + manual),
así filtros, búsqueda y vistas respetan tus correcciones. Una recopilación nueva
actualiza el detectado y recalcula el efectivo: **nunca pisa un campo manual**.
Restablecer un campo borra su override y vuelve el valor detectado.

Notas, prioridad y campos personalizados viven aparte
(`opportunity_notes`, `opportunity_custom_fields`): no tienen equivalente
detectado.

## TUI (`E`)

29 campos: título, descripción, tipo (oportunidad/noticia), categoría,
plataforma, URL, autor, usuario, URL de perfil, avatar, imagen, etiquetas,
ubicación, país, modalidad, inicio, fin, deadline, coste, precio, moneda,
requisitos, elegibilidad, premios, beneficios, certificado, viaje,
alojamiento y naturaleza de la noticia; más notas, prioridad y campos
personalizados.

| Tecla | Acción |
|---|---|
| `↑` `↓` | Moverse |
| `Enter` | Editar texto / avanzar opción |
| `←` `→` | Cambiar opción |
| `r` | Restablecer al valor detectado |
| `+` | Añadir campo personalizado (`nombre=valor`) |
| `x` | Borrar campo personalizado |
| `Ctrl-s` | Guardar (valida todo antes) |
| `Esc` | Cancelar (pide confirmación si hay cambios) |

Cada campo muestra `✎ manual` o `↺ detected`, y abajo su valor detectado.

## CLI

```sh
sentri fields                                   # campos editables
sentri edit 12 --title "Beca en Japón" --category scholarship
sentri edit 12 --set source=instagram --set author_handle=@becas
sentri edit 12 --reset title                    # vuelve al valor detectado
sentri edit 12 --notes "Pedir carta" --priority high
```

Lo que escribes en `sentri add URL --title …` también se guarda como manual.
