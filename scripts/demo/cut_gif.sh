#!/bin/sh
# Usage: cut_gif.sh <in.mp4> <start s> <end s> <out.gif> [width] [fps]
# Cuts a segment of a screen recording into a GIF with a per-clip palette.
set -eu
in=$1; start=$2; end=$3; out=$4; width=${5:-1100}; fps=${6:-12}
ffmpeg -loglevel error -y -ss "$start" -to "$end" -i "$in" \
  -vf "fps=$fps,scale=$width:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
  -loop 0 "$out"
ls -la "$out"
