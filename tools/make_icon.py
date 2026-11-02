"""Draw the launcher icon ("die der das" around a ring) into android/res."""
import math
import subprocess
from pathlib import Path

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
RES = ROOT / "android" / "res"
FONT = TTFont(ROOT / "assets" / "bricolage-800.ttf")
GLYPHS = FONT.getGlyphSet()
CMAP = FONT.getBestCmap()
UPM = FONT["head"].unitsPerEm
BG = "#ffffff"
INK = "#18202b"
# Word, colour, centre angle (degrees, 0 = right, clockwise); below the middle reads flipped.
WORDS = [("die", "#c8372f", -90), ("der", "#2563c9", 30), ("das", "#238a4f", 150)]


def word_svg(word, color, angle, size, r_in, height):
    s = size / UPM
    adv = [GLYPHS[CMAP[ord(c)]].width * s + 0.08 * size for c in word]
    total = sum(adv)
    bottom = math.sin(math.radians(angle)) > 0
    r = r_in + height if bottom else r_in  # baseline radius; letters always span r_in..r_in+height
    out, pos = [], -total / 2
    for c, a in zip(word, adv):
        mid = pos + a / 2
        pos += a
        step = math.degrees(mid / (r_in + height / 2))
        theta = angle - step if bottom else angle + step
        spin = theta - 90 if bottom else theta + 90
        base = r if bottom else -r
        pen = SVGPathPen(GLYPHS)
        # Font units are y-up; centre the glyph on its advance.
        GLYPHS[CMAP[ord(c)]].draw(TransformPen(pen, (s, 0, 0, -s, -a / 2, base)))
        out.append(f'<path transform="rotate({spin:.3f})" d="{pen.getCommands()}"/>')
    return f'<g fill="{color}">{"".join(out)}</g>'


def ring(scale):
    """The ring in a 108-unit box centred on the origin, scaled by `scale`."""
    size, r_in = 19 * scale, 15 * scale
    bp = BoundsPen(GLYPHS)
    GLYPHS[CMAP[ord("d")]].draw(bp)
    height = bp.bounds[3] * size / UPM
    words = "".join(word_svg(w, c, a, size, r_in, height) for w, c, a in WORDS)
    dots = "".join(
        f'<circle cx="{math.cos(math.radians(a)) * (r_in + height / 2):.3f}" '
        f'cy="{math.sin(math.radians(a)) * (r_in + height / 2):.3f}" r="{1.8 * scale:.3f}" fill="{INK}"/>'
        for a in (-30, 90, 210)
    )
    return words + dots


def svg(body, bg_shape=""):
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="-54 -54 108 108">'
        f"{bg_shape}{body}</svg>"
    )


def render(doc, path, px):
    subprocess.run(["rsvg-convert", "-w", str(px), "-h", str(px), "-o", str(path)], input=doc.encode(), check=True)


def main():
    # Adaptive foreground: content inside the 66-unit safe circle.
    fg = svg(ring(1.0))
    # Legacy icons fill the whole 48dp square, so the ring is drawn larger there.
    square = svg(ring(1.45), f'<rect x="-54" y="-54" width="108" height="108" rx="20" fill="{BG}"/>')
    round_ = svg(ring(1.45), f'<circle r="54" fill="{BG}"/>')
    for d, k in [("mdpi", 1), ("hdpi", 1.5), ("xhdpi", 2), ("xxhdpi", 3), ("xxxhdpi", 4)]:
        out = RES / f"mipmap-{d}"
        render(fg, out / "ic_launcher_foreground.png", int(108 * k))
        render(square, out / "ic_launcher.png", int(48 * k))
        render(round_, out / "ic_launcher_round.png", int(48 * k))
    render(round_, ROOT / "target" / "icon-preview.png", 512)


if __name__ == "__main__":
    main()
