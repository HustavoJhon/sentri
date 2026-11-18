"""Convert a `tmux capture-pane -e -p` dump into a fixed-grid HTML page."""
import html, re, sys, unicodedata

BASE16 = ["#282828","#cc241d","#98971a","#d79921","#458588","#b16286","#689d6a","#a89984",
          "#928374","#fb4934","#b8bb26","#fabd2f","#83a598","#d3869b","#8ec07c","#ebdbb2"]
FG, BG = "#ebdbb2", "#282828"

def xterm(n):
    if n < 16: return BASE16[n]
    if n < 232:
        n -= 16; r, g, b = n // 36, (n // 6) % 6, n % 6
        c = lambda v: 0 if v == 0 else 55 + 40 * v
        return "#%02x%02x%02x" % (c(r), c(g), c(b))
    v = 8 + 10 * (n - 232); return "#%02x%02x%02x" % (v, v, v)

SGR = re.compile(r"\x1b\[([0-9;:]*)m")

def apply(state, params):
    p = [int(x) if x else 0 for x in params.replace(":", ";").split(";")] if params else [0]
    i = 0
    while i < len(p):
        c = p[i]
        if c == 0: state.clear()
        elif c == 1: state["b"] = 1
        elif c == 2: state["d"] = 1
        elif c == 3: state["i"] = 1
        elif c == 4: state["u"] = 1
        elif c == 7: state["r"] = 1
        elif c == 9: state["s"] = 1
        elif c == 22: state.pop("b", None); state.pop("d", None)
        elif c == 23: state.pop("i", None)
        elif c == 24: state.pop("u", None)
        elif c == 27: state.pop("r", None)
        elif c == 29: state.pop("s", None)
        elif 30 <= c <= 37: state["fg"] = BASE16[c - 30]
        elif 90 <= c <= 97: state["fg"] = BASE16[c - 90 + 8]
        elif c == 39: state.pop("fg", None)
        elif 40 <= c <= 47: state["bg"] = BASE16[c - 40]
        elif 100 <= c <= 107: state["bg"] = BASE16[c - 100 + 8]
        elif c == 49: state.pop("bg", None)
        elif c in (38, 48) and i + 1 < len(p):
            key = "fg" if c == 38 else "bg"
            if p[i + 1] == 5 and i + 2 < len(p):
                state[key] = xterm(p[i + 2]); i += 2
            elif p[i + 1] == 2 and i + 4 < len(p):
                state[key] = "#%02x%02x%02x" % tuple(p[i + 2:i + 5]); i += 4
        i += 1

def style(state):
    fg, bg = state.get("fg", FG), state.get("bg", BG)
    if state.get("r"): fg, bg = bg, fg
    css = [f"color:{fg}", f"background:{bg}"]
    if state.get("b"): css.append("font-weight:700")
    if state.get("d"): css.append("opacity:.7")
    if state.get("i"): css.append("font-style:italic")
    deco = [d for k, d in (("u", "underline"), ("s", "line-through")) if state.get(k)]
    if deco: css.append("text-decoration:" + " ".join(deco))
    return ";".join(css)

def wide(ch):
    return unicodedata.east_asian_width(ch) in ("W", "F")

def row_html(line, cols):
    state, out, width = {}, [], 0
    pos = 0
    for m in list(SGR.finditer(line)) + [None]:
        end = m.start() if m else len(line)
        text = line[pos:end]
        if text:
            st = style(state)
            for ch in text:
                if ord(ch) < 32: continue
                if wide(ch):
                    out.append(f'<span class="w" style="{st}">{html.escape(ch)}</span>'); width += 2
                elif ch == "▀":  # upper half block: draw it exactly
                    fg, bg = state.get("fg", FG), state.get("bg", BG)
                    if state.get("r"): fg, bg = bg, fg
                    out.append(f'<span class="hb" style="background:linear-gradient({fg} 50%,{bg} 50%)"> </span>'); width += 1
                elif 0x2500 <= ord(ch) <= 0x257F:  # box drawing: stretch to the row
                    out.append(f'<span class="bx" style="{st}">{html.escape(ch)}</span>'); width += 1
                else:
                    out.append(f'<span style="{st}">{html.escape(ch)}</span>'); width += 1
        if m:
            apply(state, m.group(1)); pos = m.end()
    if width < cols:
        out.append(f'<span style="{style(state)}">{" " * (cols - width)}</span>')
    return "".join(out)

def main(src, dst, cols, rows, font):
    lines = open(src, encoding="utf-8", errors="replace").read().split("\n")[:rows]
    lines += [""] * (rows - len(lines))
    body = "\n".join(f'<div class="r">{row_html(l, cols)}</div>' for l in lines)
    page = f"""<!doctype html><meta charset="utf-8"><style>
html,body{{margin:0;background:{BG}}}
.t{{padding:14px 16px;font-family:"{font}",monospace;font-size:15px;line-height:19px;font-variant-ligatures:none}}
.r{{white-space:pre;height:19px;overflow:hidden}}
.r span{{display:inline-block;height:19px;vertical-align:top;width:1ch;text-align:center}}
.r span:last-child{{width:auto;text-align:left}}
.w{{width:2ch!important}}
.bx{{transform:scaleY(1.45)}}
</style><div class="t">{body}</div>"""
    open(dst, "w", encoding="utf-8").write(page)

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]),
         sys.argv[5] if len(sys.argv) > 5 else "IosevkaTerm Nerd Font Mono")
