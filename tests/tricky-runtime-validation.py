#!/usr/bin/env python3
"""Fast optional runtime probes for robotics packages with native edges."""

from __future__ import annotations

import importlib
import importlib.util
import sys


def available(module: str) -> bool:
    return importlib.util.find_spec(module) is not None


def probe(label: str, module: str, check) -> bool:
    if not available(module):
        print(f"SKIP {label} - missing {module}")
        return True
    try:
        check()
    except Exception as exc:
        print(f"FAIL {label} - {type(exc).__name__}: {exc}")
        return False
    print(f"PASS {label}")
    return True


def import_module(module: str) -> None:
    importlib.import_module(module)


def check_cuda() -> None:
    if available("cuda.bindings.runtime"):
        import_module("cuda.bindings.runtime")
    else:
        import_module("cuda")


def check_torchvision() -> None:
    import torch
    import torchvision

    _ = torchvision.__version__
    if torch.cuda.is_available():
        torch.empty((1,), device="cuda")


def main() -> int:
    probes = [
        ("cuda-python import", "cuda", check_cuda),
        ("PyAV import", "av", lambda: import_module("av")),
        ("LeRobot import", "lerobot", lambda: import_module("lerobot")),
        ("TorchVision import", "torchvision", check_torchvision),
        ("PyTorch3D import", "pytorch3d", lambda: import_module("pytorch3d")),
        ("torch3d import", "torch3d", lambda: import_module("torch3d")),
        ("FlashAttention import", "flash_attn", lambda: import_module("flash_attn")),
        ("evdev import", "evdev", lambda: import_module("evdev.ecodes")),
    ]
    ok = True
    for item in probes:
        ok = probe(*item) and ok
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
