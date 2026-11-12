"""Draws scripts/dmg/background.tiff, the window of the disk image: a page of the notebook the boy
of Noteki's icon writes in, warm paper with faint rules and a margin, in the Everforest light
colours the app opens with, and an arrow from Noteki to Applications. Rendered with Google Chrome
at 1× and 2×, joined by tiffutil so Finder picks the sharp one on Retina screens. The icon
positions in make-dmg.sh match ICON_Y, NOTEKI_X and APPLICATIONS_X. From Takku's and Dimmela's.

    python3 scripts/dmg/background.py
"""
import math
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

WIDTH, HEIGHT = 640, 400
ICON_Y, NOTEKI_X, APPLICATIONS_X = 200, 175, 465

# A soft curve over the gap between the icons, its head along the curve's last direction.
start, end = NOTEKI_X + 82, APPLICATIONS_X - 86
mid, top = (start + end) / 2, ICON_Y - 46
angle = math.atan2((ICON_Y - 6) - top, end - mid)
head = [
    (end - 15 * math.cos(angle + side * 0.55), ICON_Y - 6 - 15 * math.sin(angle + side * 0.55)) for side in (-1, 1)
]
arrow = (
    f"M{start} {ICON_Y - 6} Q{mid} {top} {end} {ICON_Y - 6}"
    f"M{head[0][0]:.1f} {head[0][1]:.1f} L{end} {ICON_Y - 6} L{head[1][0]:.1f} {head[1][1]:.1f}"
)

page = f"""<!doctype html>
<html><head><meta charset="utf-8"><style>
  html, body {{ margin: 0; width: {WIDTH}px; height: {HEIGHT}px; overflow: hidden; }}
  body {{
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Display", "Helvetica Neue", sans-serif;
    color: #5c6a72;
    background-color: #fdf6e3;
    background-image:
      radial-gradient(120% 90% at 50% 0%, rgba(255, 255, 255, .55) 0%, transparent 60%),
      linear-gradient(90deg, transparent 47px, rgba(248, 85, 82, .28) 47px, rgba(248, 85, 82, .28) 48px, transparent 48px),
      repeating-linear-gradient(180deg, transparent 0 27px, rgba(58, 148, 197, .13) 27px 28px);
  }}
  svg {{ position: absolute; inset: 0; }}
  h1 {{
    position: absolute; top: 44px; left: 0; right: 0; margin: 0; text-align: center;
    font-size: 26px; font-weight: 700; letter-spacing: -0.4px; color: #4d5a61;
  }}
  .then {{
    position: absolute; top: 324px; left: 0; right: 0; margin: 0; text-align: center;
    font-size: 14px; color: #829181;
  }}
</style></head><body>
<h1>Drag Noteki into Applications</h1>
<svg viewBox="0 0 {WIDTH} {HEIGHT}" fill="none" stroke-linecap="round" stroke-linejoin="round">
  <path d="{arrow}" stroke="#8da101" stroke-width="3.5"/>
</svg>
<p class="then">Then open it and choose the folder of your notes.</p>
</body></html>"""

with tempfile.TemporaryDirectory() as tmp:
    html = Path(tmp) / "background.html"
    html.write_text(page)
    pngs = []
    for scale in (1, 2):
        png = Path(tmp) / f"background-{scale}x.png"
        subprocess.run(
            [
                CHROME, "--headless", "--disable-gpu", "--hide-scrollbars", f"--force-device-scale-factor={scale}",
                "--virtual-time-budget=2000", f"--window-size={WIDTH},{HEIGHT}", f"--screenshot={png}", html.as_uri(),
            ],
            check=True,
            capture_output=True,
        )
        pngs.append(str(png))
    # 72 and 144 dpi, so Finder lays both out at 640 × 400 points.
    subprocess.run(["sips", "-s", "dpiWidth", "144", "-s", "dpiHeight", "144", pngs[1]], check=True, capture_output=True)
    subprocess.run(["tiffutil", "-cathidpicheck", *pngs, "-out", str(HERE / "background.tiff")], check=True, capture_output=True)
print(f"background.tiff: {(HERE / 'background.tiff').stat().st_size // 1024} KB")
