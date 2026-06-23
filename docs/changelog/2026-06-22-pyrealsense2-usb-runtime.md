# 2026-06-22 - pyrealsense2 USB Runtime

## Problem

A temporary project with `pyrealsense2` installed by uv failed at import time
after first-bootstrap inference generated only the base runtime:

```bash
robo run python - <<'PY'
import pyrealsense2 as rs
ctx = rs.context()
print(len(ctx.devices))
PY
```

The import failed with:

```text
ImportError: libusb-1.0.so.0: cannot open shared object file: No such file or directory
```

Inspecting the installed wheel's native extensions also showed missing
`libudev.so.1` and `libstdc++.so.6`.

## Decision

- Add a narrow `camera-usb` component for USB camera runtime libraries.
- Back `camera-usb` with Nix-provided `libusb1` and systemd's `libudev`.
- Infer `native-build` and `camera-usb` for `pyrealsense2`.
- Keep this as static package evidence; do not resolve Python metadata or patch
  project files.

## Verification

- Reproduced the missing-library import failure before changing code.
- Confirmed `libstdc++.so.6` is covered by the existing `native-build`
  component.
- In an Ubuntu 20.04 distrobox, a fresh `pyrealsense2` project inferred
  `native-build` and `camera-usb`, imported `pyrealsense2`, created an
  `rs.context()` with zero devices, and constructed `rs.pipeline(ctx)`.
- Inside that Ubuntu-launched `robo run` environment, `ldd` over the installed
  `pyrealsense2` extension modules had no unresolved entries; `libusb`,
  `libudev`, and `libstdc++` resolved from Nix runtime paths.
