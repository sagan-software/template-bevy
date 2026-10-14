"""Verify release disk checks and the explicit reserve override."""
import contextlib
import io
import os
from pathlib import Path
import runpy
import unittest
from unittest.mock import patch
from collections import namedtuple

class ReserveTests(unittest.TestCase):
    def check(self, free, env):
        usage = namedtuple("Usage", "total used free")(1000 * 1024**3, 0, free * 1024**3)
        with patch.dict(os.environ, env, clear=True), patch("sys.argv", ["reserve.py", "5"]), patch("shutil.disk_usage", return_value=usage), contextlib.redirect_stdout(io.StringIO()):
            runpy.run_path(str(Path(__file__).with_name("reserve.py")), run_name="__main__")

    def test_default_reserve(self):
        self.check(106, {})
        with self.assertRaises(SystemExit):
            self.check(104, {})

    def test_explicit_override(self):
        self.check(56, {"FACTORY_DISK_RESERVE_GIB": "50"})
        with self.assertRaises(SystemExit):
            self.check(54, {"FACTORY_DISK_RESERVE_GIB": "50"})

    def test_invalid_override(self):
        for reserve in ["nan", "inf", "-1", "1001"]:
            with self.subTest(reserve=reserve), self.assertRaises(SystemExit):
                self.check(900, {"FACTORY_DISK_RESERVE_GIB": reserve})
