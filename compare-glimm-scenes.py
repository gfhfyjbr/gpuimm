"""Compare real embedded Glimm references with native GPUI captures at the same 634x476 size."""
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

routes = ["sheet", "join", "invoice", "upgrade", "reset", "publish", "autopilot", "theme", "tasks"]
results = []
for route in routes:
    for state in ["before", "after"]:
        original = Image.open(f"glimm-original-{route}-{state}.png").convert("RGB")
        native = Image.open(f"glimm-native-{route}-{state}.png").convert("RGB")
        if original.size != native.size:
            raise ValueError((route, original.size, native.size))
        delta = np.abs(np.asarray(original).astype(np.int16) - np.asarray(native).astype(np.int16))
        results.append({"demo": route, "state": state, "mae_255": float(delta.mean()), "pixels_over_16_pct": float((delta.max(axis=2) > 16).mean() * 100)})
        diff = Image.fromarray(np.minimum(delta * 6, 255).astype(np.uint8))
        diff.save(f"glimm-layout-diff-{route}-{state}.png")
        comparison = Image.new("RGB", (1268, 506), "#fafafa")
        comparison.paste(original.resize((634, 476), Image.Resampling.LANCZOS), (0, 30))
        comparison.paste(native.resize((634, 476), Image.Resampling.LANCZOS), (634, 30))
        draw = ImageDraw.Draw(comparison)
        draw.text((12, 9), f"glimm.dev / {route} / {state}", fill="#242529")
        draw.text((646, 9), "Native GPUI / Metal", fill="#242529")
        comparison.save(f"glimm-compare-{route}-{state}.png")

before = np.asarray(Image.open("glimm-native-sheet-before.png").convert("RGB"))
gather = np.asarray(Image.open("glimm-native-sheet-gather.png").convert("RGB"))
regions = {"three_top_panels": (0, 0, 634, 116), "composer_prompt": (24, 366, 586, 40), "composer_add_button": (24, 418, 34, 34)}
isolation = []
for name, (x, y, width, height) in regions.items():
    a = before[y*2:(y+height)*2, x*2:(x+width)*2]
    b = gather[y*2:(y+height)*2, x*2:(x+width)*2]
    delta = np.abs(a.astype(np.int16)-b.astype(np.int16))
    isolation.append({"region": name, "max_channel_delta": int(delta.max()), "changed_pixels": int((delta.max(axis=2)>0).sum())})
output = {"viewport": [634,476], "layout": results, "sheet_isolation": isolation, "note": "Different browser/CoreText antialiasing and CSS/GPUI shadow kernels remain visible in pixel differences. This is not the independent shader golden test."}
Path("glimm-scene-comparison.json").write_text(json.dumps(output, indent=2)+"\n")
print(json.dumps(output, indent=2))
if any(region["max_channel_delta"] != 0 for region in isolation):
    raise SystemExit("Pixels outside the table effect changed")
