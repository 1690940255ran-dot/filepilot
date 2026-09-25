"""生成 FilePilot 应用图标。

不依赖 Pillow：直接手写 PNG（zlib + struct）与 ICO（内嵌 PNG）编码。
图案：蓝色渐变圆角方块 + 白色右向箭头，寓意「把文件领到该去的位置」。

运行：
    python scripts/make-icons.py

图标是 Tauri 构建的必需输入（`tauri.conf.json` 的 `bundle.icon` 指向它们），
缺失时 `tauri-build` 会失败。改动图案后重跑本脚本即可。
"""

import pathlib
import struct
import zlib

# 脚本位于 <repo>/scripts/，图标输出到 <repo>/src-tauri/icons/
REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = REPO_ROOT / "src-tauri" / "icons"


def png_encode(width: int, height: int, rows: list[bytes]) -> bytes:
    raw = b"".join(b"\x00" + row for row in rows)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def inside_rounded_rect(x: float, y: float, r: float) -> bool:
    if x < 0.0 or x > 1.0 or y < 0.0 or y > 1.0:
        return False
    cx = min(max(x, r), 1.0 - r)
    cy = min(max(y, r), 1.0 - r)
    dx = x - cx
    dy = y - cy
    return dx * dx + dy * dy <= r * r or (x >= r and x <= 1 - r) or (y >= r and y <= 1 - r)


def inside_arrow(x: float, y: float) -> bool:
    # 水平箭杆
    if 0.26 <= x <= 0.60 and 0.435 <= y <= 0.565:
        return True
    # 箭头三角
    if 0.54 <= x <= 0.78:
        t = (y - 0.5) / 0.20
        if abs(t) <= 1.0 and x <= 0.78 - abs(t) * 0.24:
            return True
    return False


def render(size: int) -> bytes:
    r = 0.22
    rows: list[bytes] = []
    for py in range(size):
        row = bytearray()
        for px in range(size):
            x = (px + 0.5) / size
            y = (py + 0.5) / size
            if not inside_rounded_rect(x, y, r):
                row += bytes((0, 0, 0, 0))
                continue
            # 对角渐变 #2f6fed -> #7fb0ff
            k = (x + y) / 2.0
            br = int(0x2F + (0x7F - 0x2F) * k)
            bg = int(0x6F + (0xB0 - 0x6F) * k)
            bb = int(0xED + (0xFF - 0xED) * k)
            if inside_arrow(x, y):
                row += bytes((0xFF, 0xFF, 0xFF, 0xFF))
            else:
                row += bytes((br, bg, bb, 0xFF))
        rows.append(bytes(row))
    return png_encode(size, size, rows)


def ico_encode(png_data: bytes, size: int) -> bytes:
    header = struct.pack("<HHH", 0, 1, 1)
    dim = 0 if size >= 256 else size
    entry = struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(png_data), 22)
    return header + entry + png_data


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)

    targets = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
    }
    for name, size in targets.items():
        data = render(size)
        (OUT / name).write_bytes(data)
        print(f"{name}: {size}x{size}, {len(data)} bytes")

    ico = ico_encode(render(256), 256)
    (OUT / "icon.ico").write_bytes(ico)
    print(f"icon.ico: 256x256 embedded PNG, {len(ico)} bytes")

    print(f"\nwritten to {OUT}")


if __name__ == "__main__":
    main()
