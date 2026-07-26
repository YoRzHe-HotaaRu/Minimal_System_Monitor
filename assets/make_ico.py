from PIL import Image
from pathlib import Path

src = Path(r"G:\DEVELKOPEMNT CURSOR\System_Monitor\assets\pulse_icon_1024.png")
dst = Path(r"G:\DEVELKOPEMNT CURSOR\System_Monitor\assets\pulse.ico")
img = Image.open(src).convert("RGBA")
sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
img.save(dst, format="ICO", sizes=sizes)
print(f"wrote {dst} ({dst.stat().st_size} bytes)")
