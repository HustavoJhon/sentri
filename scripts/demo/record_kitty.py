"""Record SENTRI in a real kitty window, so terminal images (kitty graphics
protocol) appear in the demo. Hyprland + gpu-screen-recorder only.

Usage: record_kitty.py <command> <scenes.json> <out.mp4> <marks.json>

Opens a floating kitty window on workspace 9, shows that workspace, records
the window's region while sending each scene's keys through kitty remote
control, then closes the window and returns to the previous workspace.

Scene file: {"name": [[keys or null, seconds to wait after them], ...]}.
Single characters are typed; longer names (`enter`, `down`, `space`) are
sent as keys; `TYPE:text` types literal text. `marks.json` gets each scene's start and end
time in the video, for cutting one GIF per scene.
"""
import json
import os
import signal
import subprocess
import sys
import tempfile
import time

WIDTH, HEIGHT, WORKSPACE, CLASS = 1400, 860, 9, "sentri-demo"


def hypr(*args):
    return subprocess.run(["hyprctl", *args], check=True, capture_output=True, text=True).stdout


def dispatch(lua, *legacy):
    """Hyprland 0.56+ takes Lua dispatchers; older versions the classic form."""
    done = subprocess.run(["hyprctl", "dispatch", lua], capture_output=True, text=True)
    if done.returncode != 0 or done.stdout.startswith("error"):
        hypr("dispatch", *legacy)


def window():
    for c in json.loads(hypr("clients", "-j")):
        if c["class"] == CLASS:
            return c
    return None


def main():
    command, scene_file, out, marks_file = sys.argv[1:5]
    scenes = json.load(open(scene_file))
    sock = os.path.join(tempfile.mkdtemp(), "kitty.sock")
    previous = json.loads(hypr("activeworkspace", "-j"))["id"]
    rules = f"[workspace {WORKSPACE} silent; float; size {WIDTH} {HEIGHT}; center]"
    kitty = (
        f"kitty --class {CLASS} -o allow_remote_control=yes --listen-on unix:{sock} "
        f"-o font_size=13 -o confirm_os_window_close=0 {command}"
    )
    dispatch(f"hl.dsp.exec_cmd([[{rules} {kitty}]])", "exec", f"{rules} {kitty}")
    for _ in range(50):
        if window():
            break
        time.sleep(0.1)
    dispatch(f'hl.dsp.focus({{ workspace = "{WORKSPACE}" }})', "workspace", str(WORKSPACE))
    time.sleep(0.6)
    w = window()
    x, y = w["at"]
    width, height = w["size"]
    recorder = subprocess.Popen(
        ["gpu-screen-recorder", "-w", f"{width}x{height}+{x}+{y}",
         "-f", "30", "-cursor", "no", "-encoder", "cpu", "-o", out],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    time.sleep(1.0)
    t0 = time.time()

    def remote(*args):
        subprocess.run(["kitten", "@", "--to", f"unix:{sock}", *args],
                       check=False, capture_output=True)

    marks = {}
    try:
        for name, steps in scenes.items():
            start = time.time() - t0
            for keys, wait in steps:
                for k in keys or []:
                    if k.startswith("TYPE:"):
                        remote("send-text", k[5:])
                    elif len(k) == 1:
                        remote("send-text", "--", k)
                    else:
                        remote("send-key", k)
                    time.sleep(0.05)
                time.sleep(wait)
            marks[name] = [round(start, 2), round(time.time() - t0, 2)]
            print(name, marks[name], flush=True)
    finally:
        recorder.send_signal(signal.SIGINT)
        recorder.wait(timeout=20)
        remote("close-window")
        dispatch(f'hl.dsp.focus({{ workspace = "{previous}" }})', "workspace", str(previous))
        json.dump(marks, open(marks_file, "w"), indent=1)


main()
