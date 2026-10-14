"""Generate original mono PCM sound assets with no external dependencies."""
import math
import struct
import wave
from pathlib import Path
Path("assets/audio").mkdir(parents=True, exist_ok=True)
for name, duration, frequencies in [("tap",0.09,[880]),("win",0.45,[523,659,784]),("music",4,[261,329,392,329])]:
    rate = 22050
    count = round(duration * rate)
    with wave.open(f"assets/audio/{name}.wav", "wb") as out:
        out.setparams((1,2,rate,count,"NONE","not compressed"))
        data = bytearray()
        for i in range(count):
            frequency = frequencies[min(len(frequencies)-1, i * len(frequencies) // count)]
            phase = (i / rate) * frequency * 2 * math.pi
            envelope = min(1, i / 220) * min(1, (count-i) / 440)
            sample = round(math.sin(phase) * envelope * (1300 if name == "music" else 5000))
            data.extend(struct.pack("<h", sample))
        out.writeframes(data)
