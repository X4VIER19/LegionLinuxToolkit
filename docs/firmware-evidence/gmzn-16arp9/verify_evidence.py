"""Validate the standalone GMZN evidence bundle without accessing hardware."""
from __future__ import annotations

from hashlib import sha256
from pathlib import Path


ROOT = Path(__file__).resolve().parent
EXPECTED = {
    "GMZN_BMOF.bin": "7FB153CDE36147518B78D94AFD7F6B7634FF5C9C1AAF4137E6EC21A99A5F196F",
    "GMZN.decompressed.bmf": "37077ABFD89B6DA495B5ABBCF17E4FEAD3D798FDABF99D49A0E2A7F604D91382",
    "GMZN.fan-method.mof": "2191EEFBD9C8BBCAE1E09B1532C3D5E8EBD08C6F5815C0C58D128422EAA82395",
    "GMZN.utf16-strings.txt": "05A8DF4C1083BE6A0CD9D74086B0ED15AED07D32EB6292E0944BF23F2BB23716",
}


def main() -> None:
    for name, expected in EXPECTED.items():
        actual = sha256((ROOT / name).read_bytes()).hexdigest().upper()
        assert actual == expected, f"{name}: expected {expected}, got {actual}"

    mof = (ROOT / "GMZN.fan-method.mof").read_text(encoding="utf-8")
    assert "[WmiMethodId(5), Implemented" in mof
    for parameter in (
        "[in, ID(0)] uint8 FanID",
        "[in, ID(1)] uint8 SensorID",
        "[out, ID(2)] uint32 FanTableSize",
        '[out, WmiSizeIs("FanTableSize"), ID(3)] uint32 FanTable[]',
        "[out, ID(4)] uint32 SensorTableSize",
        '[out, WmiSizeIs("SensorTableSize"), ID(5)] uint32 SensorTable[]',
    ):
        assert parameter in mof


if __name__ == "__main__":
    main()
