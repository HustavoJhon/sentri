# Grabar las demos

Graba la TUI real en tmux (con colores), dibuja cada fotograma con Chromium y
arma GIF/MP4 con ffmpeg. Necesita `tmux`, `chromium`, `ffmpeg` y Python 3.

Usa siempre una base de datos de demo, nunca la tuya:

```sh
# 1. Configuración aislada (database apunta a un directorio temporal)
sentri --config /tmp/demo/config.toml collect

# 2. Abrir la TUI a 140×38 y grabar las escenas de scenes.json
tmux new-session -d -s demo -x 140 -y 38 \
  "COLORTERM=truecolor sentri --config /tmp/demo/config.toml"
python3 scripts/demo/record.py demo /tmp/demo/frames scripts/demo/scenes.json

# 3. Un GIF por escena (ventana de 1300×760 px)
for d in /tmp/demo/frames/*/; do
  scripts/demo/build.sh "$d" "/tmp/demo/$(basename "$d").gif" 140 38 1300 760
done
```

Cada paso de `scenes.json` es `[teclas, segundos visible, espera antes de
capturar]`; `TYPE:texto` escribe texto literal.
