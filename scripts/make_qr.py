#!/usr/bin/env python3
"""
BeThere — QR Code Generator for the Live Demo

Generates a high-resolution, print-ready QR PNG encoding the production
demo-event registration URL. The same asset is used for:

  1. Embedding in the pitch deck  → `slide_02_join_live_demo` (see make_pitch_deck.py)
  2. Printing as a tabletop stand-up card for the room to scan during the talk

Run:
    python3 scripts/make_qr.py
Output:
    scripts/assets/qr_register.png   (~900x900, scannable from a distance)

Dependencies:
    qrcode  (pure Python, depends on PIL — already installed system-wide)
    PIL / Pillow

NOTE — keep DEMO_REGISTER_URL in sync with the constant of the same name
in scripts/make_pitch_deck.py (used for the caption text under the QR).
"""

from pathlib import Path

import qrcode
from qrcode.constants import ERROR_CORRECT_H

# =============================================================================
# CONFIG — confirm the demo event slug before demo day
# =============================================================================
# Public registration page for the demo event created in production.
# Format: {SERVER}/e/{slug}
#
# >>> Confirm `islanddao-v4` matches the actual slug of your production event
#     before printing the stand-up card. Regeneration is one command:
#         python3 scripts/make_qr.py
SERVER = "https://bethere.solana-thailand.workers.dev"
DEMO_EVENT_SLUG = "islanddao-v4-demo"
DEMO_REGISTER_URL = f"{SERVER}/e/{DEMO_EVENT_SLUG}"

# Output
ASSETS_DIR = Path(__file__).resolve().parent / "assets"
OUT_PATH = ASSETS_DIR / "qr_register.png"

# QR rendering params — tuned for projector + print reliability
BOX_SIZE = 30  # pixels per module (higher = crisper on print)
QUIET_ZONE = 3  # modules of white border (spec minimum is 4; 3 still scans)
FILL_COLOR = "#000000"  # pure black — max contrast for live-demo reliability
BACK_COLOR = "#FFFFFF"


def main() -> None:
    qr = qrcode.QRCode(
        version=None,  # auto-fit smallest version that holds the data
        error_correction=ERROR_CORRECT_H,  # ~30% redundancy — robust for projection/print damage
        box_size=BOX_SIZE,
        border=QUIET_ZONE,
    )
    qr.add_data(DEMO_REGISTER_URL)
    qr.make(fit=True)

    img = qr.make_image(fill_color=FILL_COLOR, back_color=BACK_COLOR).convert("RGB")

    ASSETS_DIR.mkdir(parents=True, exist_ok=True)
    img.save(OUT_PATH)

    w, h = img.size
    print(f"✓ wrote {OUT_PATH}")
    print(f"  size : {w}x{h} px  ({OUT_PATH.stat().st_size:,} bytes)")
    print(f"  data : {DEMO_REGISTER_URL}")
    print(f"  ecc  : H (~30% recovery)")
    print()
    print("Reminder: confirm DEMO_EVENT_SLUG matches the production event")
    print(f"          before printing — currently '{DEMO_EVENT_SLUG}'")


if __name__ == "__main__":
    main()
