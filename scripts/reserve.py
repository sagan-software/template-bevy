"""Require 50 GiB or ten percent free after the estimated output."""
import os
import shutil
import sys
usage = shutil.disk_usage(".")
reserve = max(50 * 1024**3, usage.total // 10)
if "FACTORY_DISK_RESERVE_GIB" in os.environ:
    reserve = float(os.environ["FACTORY_DISK_RESERVE_GIB"]) * 1024**3
    if not 0 <= reserve <= usage.total:
        sys.exit("FACTORY_DISK_RESERVE_GIB must be finite and within filesystem capacity")
expected = float(sys.argv[1]) * 1024**3
if usage.free - expected < reserve:
    sys.exit(f"Insufficient disk space: need {int(reserve + expected)} bytes free; have {usage.free}")
print(f"Disk reserve checked: {usage.free // 1024**3} GiB free")
