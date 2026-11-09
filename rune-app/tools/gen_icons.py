#!/usr/bin/env python3
"""Generate rune-app/src/icons.rs from lucide-react's CommonJS bundle (dist/cjs/lucide-react.js
of the version SONE pins, 0.563.0):

    python3 -I rune-app/tools/gen_icons.py path/to/lucide-react.js rune-app/src/icons.rs

Edit `want` to add icons (React export names; aliases such as MoreHorizontal resolve).
"""
import re, json, sys

src = open(sys.argv[1]).read()
out = sys.argv[2]


def balanced(text, i):
    """Index of the bracket closing the one at text[i], skipping string contents."""
    depth, instr, j = 0, False, i
    while True:
        c = text[j]
        if c == '"' and text[j - 1] != "\\":
            instr = not instr
        elif not instr:
            if c == "[":
                depth += 1
            elif c == "]":
                depth -= 1
                if depth == 0:
                    return j
        j += 1


nodes = {}
for m in re.finditer(r"const (__iconNode\$?\w*) = \[", src):
    i = m.end() - 1
    js = src[i:balanced(src, i) + 1]
    js = re.sub(r'([\s{,])([A-Za-z_][\w-]*):', lambda k: f'{k.group(1)}"{k.group(2)}":', js)
    js = re.sub(r",(\s*[}\]])", r"\1", js)
    nodes[m.group(1)] = json.loads(js)
consts = {m.group(1): m.group(2) for m in re.finditer(r'const (\w+) = createLucideIcon\("[\w-]+", (__iconNode\$?\w*)\);', src)}
exports = {m.group(1): m.group(2) for m in re.finditer(r"exports\.(\w+) = (\w+);", src)}

want = """Home Compass Bell Library Heart FolderOpen Music User ChevronLeft ChevronRight ChevronDown ChevronUp
Search X Headphones Shield Settings Keyboard Info LogOut Clock Loader2 Play Pause MoreHorizontal Shuffle SkipBack
SkipForward Repeat Mic2 VolumeX Volume1 Volume2 ListMusic PictureInPicture2 Maximize2 Minimize2 Minus Square Copy
Plus ListPlus ListEnd Video GripVertical Sparkles Users Share Share2 Pencil Lock Unlock Check Radio RefreshCw Link
Trash2 UserPlus UserCheck FolderInput Palette Cpu Tv2 AppWindow Globe FileText AlertCircle MessageSquare ArrowUpDown""".split()
filled = {"Play", "Pause", "SkipBack", "SkipForward", "Heart"}


def el(tag, a, fill):
    a = {k: v for k, v in a.items() if k != "key"}
    if tag in ("polyline", "polygon"):
        pts = a.pop("points").replace(",", " ").split()
        pairs = [f"{pts[i]} {pts[i + 1]}" for i in range(0, len(pts), 2)]
        a = {"d": "M" + " L".join(pairs) + (" Z" if tag == "polygon" else "")}
        tag = "path"
    paint = 'fill="currentColor" stroke="currentColor"' if fill else 'fill="none" stroke="currentColor"'
    attrs = " ".join(f'{k}="{v}"' for k, v in a.items())
    return f'<{tag} {attrs} {paint} stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>'


def snake(n):
    return re.sub(r"(?<!^)(?=[A-Z])|(?<=[a-z])(?=[0-9])", "_", n).upper()


lines = [
    "//! lucide icons (lucide-react 0.563.0, the version SONE's React UI uses), generated from the",
    "//! package's bundle: each element stroked with `currentColor` at stroke-width 2 on lucide's",
    "//! 24-unit grid, tinted with `.color()`. `*_FILLED` variants also fill (React's",
    '//! `fill="currentColor"`). Polylines and polygons are written as paths.',
    "//!",
    "//! lucide is ISC licensed: Copyright (c) for portions of Lucide are held by Cole Bemis",
    "//! 2013-2022 as part of Feather (MIT). All other copyright (c) for Lucide are held by Lucide",
    "//! Contributors 2022. Permission to use, copy, modify, and/or distribute this software for any",
    "//! purpose with or without fee is hereby granted, provided that the above copyright notice and",
    "//! this permission notice appear in all copies.",
    "#![allow(dead_code)]",
    "",
    "use rune::Icon;",
    "",
]
missing = []
for name in want:
    var = consts.get(exports.get(name, name))
    if not var:
        missing.append(name)
        continue
    for f in ([False, True] if name in filled else [False]):
        body = "".join(el(t, a, f) for t, a in nodes[var])
        ident = snake(name) + ("_FILLED" if f else "")
        lines.append(f'pub static {ident}: Icon = Icon {{ name: "lucide-{ident.lower()}", view: 24.0, body: r#"{body}"# }};')
open(out, "w").write("\n".join(lines) + "\n")
print("missing:", missing, "icons:", len(lines) - 14)
