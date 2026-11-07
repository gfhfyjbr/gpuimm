"""Compare native platform captures without conflating font rasterization with shader parity."""
import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

parser = argparse.ArgumentParser()
parser.add_argument('platform', choices=['linux', 'windows'])
args = parser.parse_args()
prefix = f'glimm-{args.platform}'
routes = ['sheet', 'join', 'invoice', 'upgrade', 'reset', 'publish', 'autopilot', 'theme', 'tasks']
layout = []
for route in routes:
    for state in ['before', 'after']:
        original = Image.open(f'glimm-original-{route}-{state}.png').convert('RGB')
        native = Image.open(f'{prefix}-{route}-{state}.png').convert('RGB')
        if original.size != native.size:
            original = original.resize(native.size, Image.Resampling.LANCZOS)
        delta = np.abs(np.asarray(original).astype(np.int16) - np.asarray(native).astype(np.int16))
        layout.append({'demo': route, 'state': state, 'mae_255': float(delta.mean())})
        comparison = Image.new('RGB', (1268, 506), '#fafafa')
        comparison.paste(original.resize((634, 476), Image.Resampling.LANCZOS), (0, 30))
        comparison.paste(native.resize((634, 476), Image.Resampling.LANCZOS), (634, 30))
        draw = ImageDraw.Draw(comparison)
        draw.text((12, 9), f'glimm.dev / {route} / {state}', fill='#242529')
        draw.text((646, 9), f'Native GPUI / {args.platform}', fill='#242529')
        comparison.save(f'{prefix}-compare-{route}-{state}.png')

before = np.asarray(Image.open(f'{prefix}-sheet-before.png').convert('RGB'))
gather = np.asarray(Image.open(f'{prefix}-sheet-gather.png').convert('RGB'))
if before.shape != gather.shape:
    raise ValueError('before/gather capture dimensions differ')
scale_x, scale_y = before.shape[1] / 634, before.shape[0] / 476
regions = {'three_top_panels': (0, 0, 634, 116), 'composer_prompt': (24, 366, 586, 40), 'composer_add_button': (24, 418, 34, 34)}
isolation = []
for name, (x, y, width, height) in regions.items():
    left, right = round(x * scale_x), round((x + width) * scale_x)
    top, bottom = round(y * scale_y), round((y + height) * scale_y)
    delta = np.abs(before[top:bottom, left:right].astype(np.int16) - gather[top:bottom, left:right].astype(np.int16))
    isolation.append({'region': name, 'max_channel_delta': int(delta.max()), 'changed_pixels': int((delta.max(axis=2) > 0).sum())})
result = {'platform': args.platform, 'layout': layout, 'sheet_isolation': isolation, 'note': 'Actual native captures. Font antialiasing, corner and shadow rasterization may differ. Shader parity is tested separately.'}
Path(f'{prefix}-comparison.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
if any(region['changed_pixels'] for region in isolation):
    raise SystemExit('Pixels outside the table effect changed')
