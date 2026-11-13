# Daemon

`sentri daemon` recopila de las fuentes habilitadas al arrancar y después
cada `daemon.interval_minutes` (por defecto 120, mínimo 15). Es independiente
de la TUI: puede cerrarse la interfaz y seguir recopilando.

## Garantías

| Aspecto | Comportamiento |
|---|---|
| Aislamiento | Cada fuente se ejecuta por separado; un fallo se registra en `collection_runs` y no detiene a las demás. |
| Timeouts | Límite por petición (`http.timeout_secs`) y presupuesto total por fuente (×3). |
| Deduplicación | Por URL canónica e ID externo; repetir un ciclo no duplica nada. |
| Instancia única | Bloqueo de fichero (`sentri.daemon.lock` junto a la base); el SO lo libera si el proceso muere. |
| Recuperación | Al arrancar, las ejecuciones que quedaron `running` (caída, kill) se marcan como `failed: interrupted`. |
| Parada | Ctrl-C o SIGTERM terminan el ciclo de espera limpiamente. |
| Logs | `tracing` a stderr (journald con systemd). `SENTRI_LOG=debug` para más detalle. Nunca se registran tokens. |
| Avisos | Solo en terminal (`sentri status`, Dashboard de la TUI). Sin correo ni mensajería. |

## systemd (usuario)

```sh
cargo install --path crates/sentri-cli
install -Dm644 contrib/systemd/sentri.service ~/.config/systemd/user/sentri.service
systemctl --user daemon-reload
systemctl --user enable --now sentri.service
journalctl --user -u sentri -f
```

Para que siga activo sin sesión iniciada (VPS): `loginctl enable-linger $USER`.

Tokens opcionales en `~/.config/sentri/env` (permisos `0600`):

```sh
SENTRI_GITHUB_TOKEN=ghp_...
```

## Mover al VPS

1. `sentri backup ~/sentri-backup.db` en el PC.
2. Copiar el backup y `config.toml` al VPS (`scp`), instalar el binario y la unidad.
3. En el VPS, `storage.database` apunta a la copia; activar la unidad y `linger`.
4. Desde el móvil/PC: `ssh vps -t sentri` (por Tailscale) abre la misma TUI.
