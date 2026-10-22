# GMZN firmware evidence — Lenovo Legion Slim 5 16ARP9

This folder is a standalone, read-only evidence bundle produced from the ACPI device `GMZN`. Its scripts operate only on files: they do not invoke WMI, ACPI, EC, fan controls, or firmware write paths.

## Files

| File | SHA-256 | Description |
| --- | --- | --- |
| `GMZN_BMOF.bin` | `7FB153CDE36147518B78D94AFD7F6B7634FF5C9C1AAF4137E6EC21A99A5F196F` | Original captured BMOF, copied unchanged. |
| `GMZN.decompressed.bmf` | `37077ABFD89B6DA495B5ABBCF17E4FEAD3D798FDABF99D49A0E2A7F604D91382` | DS-01 decompressed BMF. |
| `GMZN.fan-method.mof` | `2191EEFBD9C8BBCAE1E09B1532C3D5E8EBD08C6F5815C0C58D128422EAA82395` | Readable focused MOF reconstruction. |
| `GMZN.utf16-strings.txt` | `05A8DF4C1083BE6A0CD9D74086B0ED15AED07D32EB6292E0944BF23F2BB23716` | Offset-indexed UTF-16 strings from the BMF. |

## Confirmed `Fan_Get_Table` contract

`LENOVO_FAN_METHOD.Fan_Get_Table` has `WmiMethodId(5)` and its logical MOF parameter order is:

1. `in uint8 FanID` — `ID(0)`
2. `in uint8 SensorID` — `ID(1)`
3. `out uint32 FanTableSize` — `ID(2)`
4. `out uint32 FanTable[]` — `ID(3)`, `WmiSizeIs("FanTableSize")`
5. `out uint32 SensorTableSize` — `ID(4)`
6. `out uint32 SensorTable[]` — `ID(5)`, `WmiSizeIs("SensorTableSize")`

The size fields are counts of `uint32` elements at the MOF boundary. The firmware does not disclose valid FanID/SensorID values, table-element semantics, byte packing, limits, or write behavior. `Fan_Set_Table` (`WmiMethodId(6)`) is documented only as evidence and is never invoked.

## Reproduction

`decode_gmzn_bmof.py` is a local bounds-checked DS-01 decoder. It requires Python 3 and writes only the output file explicitly named on its command line:

```powershell
python .\decode_gmzn_bmof.py .\GMZN_BMOF.bin .\reproduced.bmf
python .\verify_evidence.py
```

The capture was cross-checked against `pali/bmfdec` commit `c7b72f6ece126a8f686d66402009a14cb032a769`. Its GPLv2 source is not included in this bundle; obtain it separately from https://github.com/pali/bmfdec if desired.

## Tool provenance and licensing

`decode_gmzn_bmof.py` is a separate, offline Python transcription derived from
`pali/bmfdec`'s DS-01 decompressor. Upstream `bmfdec.c` is GPLv2-only; this
script is retained exclusively to reproduce the evidence bundle and is not
called, linked, or incorporated by the MIT OR Apache-2.0 Rust crates.

Treat redistribution of this script as a GPLv2-only matter, including the
applicable notice and corresponding-source obligations. It requires separate
licensing review before redistributing the evidence tooling outside this
repository. `verify_evidence.py` only hashes local evidence files and has no
hardware interaction.
