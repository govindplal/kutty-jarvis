"""Draws the mock screen that `ask.rs` describes with its element list.

Usage: python3 mock_screen.py out.png   (needs Pillow)
"""

import sys

from PIL import Image, ImageDraw, ImageFont

# (label, x, y, width, height); keep in sync with MOCK_ELEMENTS in ask.rs.
BUTTONS = [
    ("Open", 40, 60, 120, 44),
    ("Save", 180, 60, 120, 44),
    ("Print", 320, 60, 120, 44),
    ("Settings", 1100, 60, 140, 44),
]
SEARCH = ("Search", 700, 60, 360, 44)


def font(size):
    for name in ("DejaVuSans.ttf", "arial.ttf"):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    return ImageFont.load_default()


def main(path):
    image = Image.new("RGB", (1280, 720), "#f3f3f3")
    draw = ImageDraw.Draw(image)
    draw.rectangle((0, 0, 1280, 36), fill="#2b579a")
    draw.text((16, 6), "Report - Editor", fill="white", font=font(18))
    for label, x, y, w, h in BUTTONS:
        draw.rounded_rectangle((x, y, x + w, y + h), 6, fill="#dde6f5", outline="#2b579a")
        draw.text((x + 20, y + 10), label, fill="black", font=font(20))
    label, x, y, w, h = SEARCH
    draw.rectangle((x, y, x + w, y + h), fill="white", outline="#888")
    draw.text((x + 12, y + 10), label + "...", fill="#777", font=font(20))
    draw.rectangle((40, 130, 1240, 690), fill="white", outline="#ccc")
    draw.text((60, 150), "Quarterly report", fill="black", font=font(28))
    draw.text((60, 200), "Revenue grew four percent over the last quarter.", fill="#333", font=font(20))
    image.save(path)


if __name__ == "__main__":
    main(sys.argv[1])
