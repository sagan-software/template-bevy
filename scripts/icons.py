#!/usr/bin/env python3
"""Generate platform icons from assets/icon.svg using ImageMagick."""
import json
import subprocess
from pathlib import Path

def icon(path, size):
    path.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["magick", "-background", "none", "assets/icon.svg", "-resize", f"{size}x{size}", str(path)], check=True)

for size in [32, 180, 192, 512]:
    icon(Path(f"web/icon-{size}.png"), size)
subprocess.run(["magick", "web/icon-32.png", "web/favicon.ico"], check=True)
Path("web/manifest.webmanifest").write_text(json.dumps({"name": "Hello target", "short_name": "Hello", "start_url": "./", "display": "standalone", "orientation": "portrait", "background_color": "#14262d", "theme_color": "#14262d", "icons": [{"src": f"icon-{s}.png", "sizes": f"{s}x{s}", "type": "image/png"} for s in [192, 512]]}, indent=2))
for density, size in [("mdpi",48),("hdpi",72),("xhdpi",96),("xxhdpi",144),("xxxhdpi",192)]:
    icon(Path(f"platform/android/app/src/main/res/mipmap-{density}/ic_launcher.png"), size)
base = Path("platform/ios/ios-src/Assets.xcassets/AppIcon.appiconset")
items = []
for idiom, sizes in [("iphone", [20,29,40,60]), ("ipad", [20,29,40,76,83.5])]:
    for size in sizes:
        for scale in ([2,3] if idiom == "iphone" else ([2] if size == 83.5 else [1,2])):
            filename = f"{idiom}-{size}-{scale}.png"
            icon(base / filename, round(size * scale))
            items.append({"idiom":idiom,"size":f"{size}x{size}","scale":f"{scale}x","filename":filename})
icon(base / "store.png", 1024)
items.append({"idiom":"ios-marketing","size":"1024x1024","scale":"1x","filename":"store.png"})
(base / "Contents.json").write_text(json.dumps({"images":items,"info":{"version":1,"author":"sagan.software"}}, indent=2))
