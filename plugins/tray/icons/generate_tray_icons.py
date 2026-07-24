#!/usr/bin/env python3
"""
Generate the Notare tray / menubar icon set.

These icons are used by the `tauri-plugin-hypr-tray` plugin. On macOS they are
rendered as *template* images (see plugins/tray/src/ext.rs -> `.icon_as_template(true)`),
which means only the ALPHA channel matters: the OS recolors the shape to match the
menubar (black in light mode, white in dark mode). Therefore every glyph here is a
SOLID BLACK shape on a fully-transparent background -- no color, no anti-alias into
gray-that-reads-as-color; just black + alpha.

The Notare mark is a serif "N". This is a FIRST-PASS template using DejaVu Serif Bold
and needs a visual eyeball on a real macOS menubar + Windows tray before it ships.

Run:
    python3 -m venv /tmp/trayvenv
    /tmp/trayvenv/bin/pip install Pillow
    /tmp/trayvenv/bin/python plugins/tray/icons/generate_tray_icons.py
"""

import os
from PIL import Image, ImageDraw, ImageFont

# ---- config -------------------------------------------------------------
SIZE = 160                      # final canvas (matches the original assets)
SS = 4                          # supersample factor for anti-aliasing
BIG = SIZE * SS                 # 640
GLYPH = "N"
FONT_CANDIDATES = [
    "/usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSerif-Bold.ttf",
]
BLACK = (0, 0, 0, 255)
TARGET_GLYPH_FRAC = 0.72        # glyph occupies ~72% of the canvas height

OUT_DIR = os.path.dirname(os.path.abspath(__file__))


def load_font(px):
    for path in FONT_CANDIDATES:
        if os.path.exists(path):
            return ImageFont.truetype(path, px)
    raise RuntimeError("No serif bold font found among: %s" % FONT_CANDIDATES)


def base_canvas():
    """Return a transparent BIG x BIG RGBA canvas."""
    return Image.new("RGBA", (BIG, BIG), (0, 0, 0, 0))


def draw_n(img):
    """Draw a centered bold serif black N onto the (supersampled) canvas."""
    draw = ImageDraw.Draw(img)

    # Pick a font size so the glyph's tight bbox is ~TARGET_GLYPH_FRAC of BIG.
    px = int(BIG * TARGET_GLYPH_FRAC)
    font = load_font(px)

    # Measure the tight bounding box of the glyph and adjust so the *ink*
    # (not the font metrics box) is what we scale to the target fraction.
    bbox = draw.textbbox((0, 0), GLYPH, font=font)
    glyph_h = bbox[3] - bbox[1]
    scale = (BIG * TARGET_GLYPH_FRAC) / glyph_h
    px = max(1, int(px * scale))
    font = load_font(px)

    # Re-measure with the corrected size and center by the ink bbox.
    bbox = draw.textbbox((0, 0), GLYPH, font=font)
    gw = bbox[2] - bbox[0]
    gh = bbox[3] - bbox[1]
    x = (BIG - gw) / 2 - bbox[0]
    y = (BIG - gh) / 2 - bbox[1]
    draw.text((x, y), GLYPH, font=font, fill=BLACK)


def add_dot(img, cx_frac, cy_frac, r_frac, alpha=255):
    """Draw a filled black dot (monochrome + alpha) at fractional coords/radius."""
    draw = ImageDraw.Draw(img)
    cx = cx_frac * BIG
    cy = cy_frac * BIG
    r = r_frac * BIG
    draw.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(0, 0, 0, alpha))


def finalize(img, name):
    """Downscale to SIZE with LANCZOS and save."""
    out = img.resize((SIZE, SIZE), Image.LANCZOS)
    path = os.path.join(OUT_DIR, name)
    out.save(path, "PNG")
    return path


def main():
    written = []

    # tray_default: the N.
    img = base_canvas()
    draw_n(img)
    written.append(finalize(img, "tray_default.png"))

    # tray_degraded: identical shape (template mode cannot show color; the
    # degraded signal can't be encoded in a monochrome template icon).
    img = base_canvas()
    draw_n(img)
    written.append(finalize(img, "tray_degraded.png"))

    # tray_update: the N + a small solid dot badge in the top-right corner
    # (indicates an update is available).
    img = base_canvas()
    draw_n(img)
    add_dot(img, cx_frac=0.82, cy_frac=0.18, r_frac=0.11, alpha=255)
    written.append(finalize(img, "tray_update.png"))

    # tray_recording_0..3: the N + a bottom-right "recording" dot that PULSES
    # across the 4 frames. The 250ms animation loop cycles these, so varying the
    # radius reads as a pulsing REC indicator. Monochrome: we modulate BOTH the
    # radius and the alpha (small/faint -> large/solid -> small/faint).
    pulse = [
        (0.055, 150),   # frame 0: small, faint
        (0.085, 210),   # frame 1: medium
        (0.110, 255),   # frame 2: large, solid  (peak)
        (0.085, 210),   # frame 3: medium (falling)
    ]
    for i, (r_frac, alpha) in enumerate(pulse):
        img = base_canvas()
        draw_n(img)
        add_dot(img, cx_frac=0.82, cy_frac=0.82, r_frac=r_frac, alpha=alpha)
        written.append(finalize(img, "tray_recording_%d.png" % i))

    for p in written:
        print("wrote", p)


if __name__ == "__main__":
    main()
