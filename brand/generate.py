#!/usr/bin/env python3
"""Writes the Kiri logo SVG family into brand/logo and brand/icon."""
import os, sys

OUT = sys.argv[1]  # brand dir

EMBER = "#E0B420"  # dark mustard, as on the site
EMBER_LID = "#F2CF5B"
INK = "#0F1419"
CHALK = "#F6F2EA"

# ---------- mark (64 x 64 design grid) ----------
# Box: rounded square sheared forward (skewX -10deg) so it leans into motion.
# Eyes look forward (right). Three speed lines trail on the left.
BOX = '<rect x="22" y="12" width="34" height="40" rx="10"/>'
LID = '<rect x="10" y="0" width="60" height="21.5"/>'
EYES = '<ellipse cx="41.5" cy="29" rx="3.1" ry="3.6"/><ellipse cx="50.5" cy="29" rx="3.1" ry="3.6"/>'
LINES = ('<rect x="5" y="21.5" width="13" height="5" rx="2.5"/>'
         '<rect x="1" y="32" width="15" height="5" rx="2.5"/>'
         '<rect x="7" y="42.5" width="7" height="5" rx="2.5"/>')
SKEW = 'translate(6 0) skewX(-10)'


def mark_color(line_color, uid):
    return f'''<g transform="{SKEW}">
    <clipPath id="{uid}-clip">{BOX}</clipPath>
    <g fill="{EMBER}">{BOX}</g>
    <g clip-path="url(#{uid}-clip)" fill="{EMBER_LID}">{LID}</g>
    <g fill="{INK}">{EYES}</g>
  </g>
  <g fill="{line_color}">{LINES}</g>'''


def mark_mono(color, uid):
    # single colour: eyes and lid seam knocked out with a mask
    return f'''<mask id="{uid}-m" maskUnits="userSpaceOnUse" x="0" y="0" width="64" height="64">
    <g transform="{SKEW}"><g fill="#fff">{BOX}</g><g fill="#000">{EYES}<rect x="10" y="20.5" width="60" height="2"/></g></g>
  </mask>
  <g fill="{color}">
    <rect width="64" height="64" mask="url(#{uid}-m)"/>
    {LINES}
  </g>'''

# ---------- wordmark (geometric strokes, baseline y=56) ----------
# Stroke 9 with round caps. Dots of the i's are tiny ember containers.
SW = 9


def wordmark(ink, dot):
    return f'''<g fill="none" stroke="{ink}" stroke-width="{SW}" stroke-linecap="round" stroke-linejoin="round">
    <path d="M4.5 14.5V51.5"/>
    <path d="M29 30.5L8 46"/>
    <path d="M16.5 40.5L29.5 51.5"/>
    <path d="M45 30.5V51.5"/>
    <path d="M61 51.5V41C61 34 66 30.5 74 30.5"/>
    <path d="M88.5 30.5V51.5"/>
  </g>
  <g fill="{dot}">
    <rect x="40.5" y="10.5" width="9" height="9" rx="2.4" transform="rotate(-10 45 15)"/>
    <rect x="84" y="10.5" width="9" height="9" rx="2.4" transform="rotate(-10 88.5 15)"/>
  </g>'''

WM_W = 93  # wordmark width in its own units


def svg(w, h, body, title, extra=''):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w*4:g}" height="{h*4:g}" '
            f'role="img" aria-labelledby="t">{extra}\n  <title id="t">{title}</title>\n  {body}\n</svg>\n')


def write(path, text):
    full = os.path.join(OUT, path)
    with open(full, 'w') as f:
        f.write(text)
    print('wrote', full)

variants = {
    'light': dict(lines=INK, ink=INK, dot=EMBER, mono=False),
    'dark': dict(lines=CHALK, ink=CHALK, dot=EMBER, mono=False),
    'mono': dict(color=INK, mono=True),
    'mono-white': dict(color=CHALK, mono=True),
}

for name, v in variants.items():
    uid = f'k{name.replace("-", "")}'
    if v['mono']:
        m = mark_mono(v['color'], uid)
        wm = wordmark(v['color'], v['color'])
    else:
        m = mark_color(v['lines'], uid)
        wm = wordmark(v['ink'], v['dot'])
    # mark only
    write(f'logo/kiri-mark-{name}.svg', svg(64, 64, m, 'Kiri'))
    # horizontal: mark 64 tall, wordmark scaled 1.0 sitting on the box baseline
    # box spans y 12..52; wordmark spans y 10..56 -> shift so baseline 56 -> 52
    hw = 64 + 12 + WM_W * 0.8
    body = f'{m}\n  <g transform="translate(76 7.2) scale(0.8)">{wm}</g>'
    write(f'logo/kiri-horizontal-{name}.svg', svg(hw, 64, body, 'Kiri'))
    # stacked: mark scaled 1.6 above, wordmark centred below
    sw = 120
    mx = (sw - 64 * 1.6) / 2
    body = (f'<g transform="translate({mx:g} 0) scale(1.6)">{m}</g>\n'
            f'  <g transform="translate({(sw - WM_W) / 2:g} 104)">{wm}</g>')
    write(f'logo/kiri-stacked-{name}.svg', svg(sw, 164, body, 'Kiri'))

# ---------- favicon: simplified mark, no speed lines, fills the tile ----------
fav = f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="32" height="32">
  <title>Kiri</title>
  <g transform="translate(2.6 0) skewX(-10)">
    <clipPath id="fc"><rect x="5" y="2" width="26" height="28" rx="7"/></clipPath>
    <rect x="5" y="2" width="26" height="28" rx="7" fill="{EMBER}"/>
    <rect x="0" y="0" width="40" height="9" fill="{EMBER_LID}" clip-path="url(#fc)"/>
    <ellipse cx="18.6" cy="15" rx="2.4" ry="2.9" fill="{INK}"/>
    <ellipse cx="25.4" cy="15" rx="2.4" ry="2.9" fill="{INK}"/>
  </g>
</svg>
'''
write('logo/kiri-favicon.svg', fav)

# ---------- app icon 1024 ----------
def app_icon(m, rx, width, tile_note):
    """The app icon: a rounded tile `m` px in from each edge, and the mark
    drawn `width` of the tile wide, centred. The drawing spans x 1..60 and
    y 12..52 of its 64-unit grid, so it is centred on (30.5, 32), not the
    grid's middle."""
    t = 1024 - 2 * m
    scale = round(width * t / 59, 3)
    ax, ay = round(512 - 30.5 * scale, 1), round(512 - 32 * scale, 1)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
  <title>Kiri app icon</title>
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#1B232C"/>
      <stop offset="1" stop-color="{INK}"/>
    </linearGradient>
    <radialGradient id="glow" cx="0.62" cy="0.46" r="0.55">
      <stop offset="0" stop-color="{EMBER}" stop-opacity="0.28"/>
      <stop offset="1" stop-color="{EMBER}" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="box" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#EDC23A"/>
      <stop offset="1" stop-color="#CC9A0C"/>
    </linearGradient>
  </defs>
  <!-- {tile_note} -->
  <rect x="{m}" y="{m}" width="{t}" height="{t}" rx="{rx}" fill="url(#bg)"/>
  <rect x="{m}" y="{m}" width="{t}" height="{t}" rx="{rx}" fill="url(#glow)"/>
  <rect x="{m + 1}" y="{m + 1}" width="{t - 2}" height="{t - 2}" rx="{rx - 1}" fill="none" stroke="#FFFFFF" stroke-opacity="0.07" stroke-width="2"/>
  <!-- mark: {width:.0%} of the tile wide, centred -->
  <g transform="translate({ax:g} {ay:g}) scale({scale:g})">
    <g transform="{SKEW}">
      <clipPath id="ic">{BOX}</clipPath>
      <g fill="url(#box)">{BOX}</g>
      <g clip-path="url(#ic)" fill="{EMBER_LID}">{LID}</g>
      <g fill="{INK}">{EYES}</g>
      <g fill="#FFFFFF" opacity="0.9"><circle cx="42.6" cy="27.6" r="0.9"/><circle cx="51.6" cy="27.6" r="0.9"/></g>
    </g>
    <g fill="{CHALK}">{LINES}</g>
  </g>
</svg>
'''


write('icon/kiri-app-icon.svg', app_icon(100, 185, 0.84, 'macOS-style rounded tile with 100px safe margin'))
# Windows draws icons to the edge of their square, and Apple's margin makes
# Kiri look a size small in the taskbar: the tile fills the canvas (same
# corner ratio) and the mark is drawn larger. Rendered to PNGs and an .ico
# by windows-icons.ps1.
write('icon/kiri-app-icon-windows.svg', app_icon(0, 230, 0.86, 'Windows: the tile fills the square, the mark drawn larger'))
