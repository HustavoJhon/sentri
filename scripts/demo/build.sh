#!/bin/sh
# Usage: build.sh <scene dir> <out.gif> <cols> <rows> <width> <height>
set -eu
dir=$1; out=$2; cols=$3; rows=$4; w=$5; h=$6
tools=$(dirname "$0")
for a in "$dir"/*.ans; do
  python3 -I "$tools/ansi2html.py" "$a" "${a%.ans}.html" "$cols" "$rows"
done
ls "$dir"/*.html | xargs -P 6 -I{} sh -c 'chromium --headless=new --hide-scrollbars --window-size='"$w,$h"' --screenshot="${1%.html}.png" "file://$1" >/dev/null 2>&1' _ {}
python3 -I - "$dir" <<'PY'
import json, os, sys
d = sys.argv[1]
dur = json.load(open(os.path.join(d, "durations.json")))
pngs = sorted(f for f in os.listdir(d) if f.endswith(".png"))
with open(os.path.join(d, "list.txt"), "w") as f:
    for p, t in zip(pngs, dur):
        f.write(f"file '{os.path.join(d, p)}'\nduration {t}\n")
    f.write(f"file '{os.path.join(d, pngs[-1])}'\n")
PY
ffmpeg -loglevel error -y -f concat -safe 0 -i "$dir/list.txt" \
  -vf "scale=1100:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
  -loop 0 "$out"
ls -la "$out"
