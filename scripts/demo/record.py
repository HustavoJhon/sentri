"""Drive the TUI in tmux and capture colored frames per scene.

Usage: record.py <session> <outdir> <scene-file.json>
Scene file: {"name": [[keys or null, hold_seconds, settle_seconds], ...], ...}
Each step sends keys (tmux send-keys syntax, list of tokens), waits `settle`
seconds, captures one frame and shows it for `hold` seconds in the GIF.
"""
import json, os, subprocess, sys, time

session, outdir, scene_file = sys.argv[1:4]
scenes = json.load(open(scene_file))
for name, steps in scenes.items():
    d = os.path.join(outdir, name)
    os.makedirs(d, exist_ok=True)
    durations = []
    for i, (keys, hold, settle) in enumerate(steps):
        if keys:
            for k in keys:
                if k.startswith("TYPE:"):
                    subprocess.run(["tmux", "send-keys", "-t", session, "-l", k[5:]], check=True)
                else:
                    subprocess.run(["tmux", "send-keys", "-t", session, k], check=True)
                time.sleep(0.05)
        time.sleep(settle)
        ans = subprocess.run(["tmux", "capture-pane", "-e", "-p", "-t", session],
                             check=True, capture_output=True).stdout
        open(os.path.join(d, f"{i:04d}.ans"), "wb").write(ans)
        durations.append(hold)
    json.dump(durations, open(os.path.join(d, "durations.json"), "w"))
    print(name, len(steps), "frames")
