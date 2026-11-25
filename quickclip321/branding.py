"""Render the supplied transparent mark consistently across Windows themes."""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw


def brand_tile(path: Path) -> Image.Image:
    with Image.open(path) as source:
        mark = source.convert("RGBA")
    bounds = mark.getchannel("A").getbbox()
    if bounds:
        mark = mark.crop(bounds)
    side = round(max(mark.size) * 1.16)
    tile = Image.new("RGBA", (side, side))
    ImageDraw.Draw(tile).rounded_rectangle((0, 0, side - 1, side - 1), radius=round(side * 0.18), fill="#171322")
    tile.alpha_composite(mark, ((side - mark.width) // 2, (side - mark.height) // 2))
    return tile
