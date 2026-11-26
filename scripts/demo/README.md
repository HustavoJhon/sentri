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

## Grabar en kitty (con imágenes)

El método anterior captura texto desde tmux, así que nunca muestra imágenes.
Para que aparezcan el avatar de GitHub y las imágenes de Details (protocolo
gráfico de kitty), se graba la ventana real de kitty. Necesita Hyprland,
`kitty`, `gpu-screen-recorder` y `ffmpeg`.

```sh
# 1. Un script que exporte un HOME/XDG aislado (base de datos de demo) y
#    ejecute `sleep 1.5; exec sentri`. Ejemplo: /tmp/demo/run.sh

# 2. Abre kitty flotante (1400×860) en el workspace 9, lo muestra, graba esa
#    región y envía las teclas de cada escena por control remoto de kitty.
#    Al terminar cierra la ventana y vuelve al workspace anterior.
python3 -I scripts/demo/record_kitty.py /tmp/demo/run.sh \
  scripts/demo/scenes-kitty-tour.json /tmp/demo/tour.mp4 /tmp/demo/tour-marks.json

# 3. Un GIF por escena con los tiempos de tour-marks.json
scripts/demo/cut_gif.sh /tmp/demo/tour.mp4 9.8 14.9 docs/assets/demo/02-dashboard.gif 1100 10
```

Durante la grabación no uses el teclado: las teclas irían a la ventana de
kitty. Si el driver NVIDIA no coincide con la versión de NVENC de FFmpeg,
`gpu-screen-recorder` usa `-encoder cpu` (ya incluido). Escenas: cada paso es
`[teclas, segundos de espera]`; un carácter se escribe, `enter`/`down`/`space`
se envían como teclas y `TYPE:texto` escribe texto literal.
`scenes-kitty-first-run.json` graba una primera ejecución completa (HOME vacío).
