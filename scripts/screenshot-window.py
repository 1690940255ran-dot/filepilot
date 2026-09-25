"""截取 FilePilot 桌面窗口，作为 P0 的 UI 证据。

为什么不用 PowerShell 的 System.Drawing：
  本环境的安全策略禁止 `Add-Type`（"compiles and loads .NET code at runtime"），
  所以改成纯 Python + ctypes 直接调 user32/gdi32，不引入任何第三方依赖。

用法：
    python scripts/screenshot-window.py [窗口标题关键字] [输出路径]
"""

from __future__ import annotations

import ctypes
import pathlib
import struct
import sys
import time
import zlib
from ctypes import wintypes

user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32

# 显式声明签名。不这么做时，ctypes 会把 Python 的 `-1` 当成 32 位 int 传给
# `SetWindowPos` 的 HWND 参数，而 64 位下的 `HWND_TOPMOST` 是 0xFFFFFFFFFFFFFFFF
# ——结果是置顶静默失败，截图拍到别的窗口。
user32.SetWindowPos.argtypes = [
    wintypes.HWND,  # hWnd
    wintypes.HWND,  # hWndInsertAfter
    ctypes.c_int,  # X
    ctypes.c_int,  # Y
    ctypes.c_int,  # cx
    ctypes.c_int,  # cy
    wintypes.UINT,  # uFlags
]
user32.SetWindowPos.restype = wintypes.BOOL
user32.GetDC.argtypes = [wintypes.HWND]
user32.GetDC.restype = wintypes.HDC
user32.GetWindowDC.argtypes = [wintypes.HWND]
user32.GetWindowDC.restype = wintypes.HDC
user32.PrintWindow.argtypes = [wintypes.HWND, wintypes.HDC, wintypes.UINT]
user32.PrintWindow.restype = wintypes.BOOL
user32.ReleaseDC.argtypes = [wintypes.HWND, wintypes.HDC]
user32.ReleaseDC.restype = ctypes.c_int
user32.ShowWindow.argtypes = [wintypes.HWND, ctypes.c_int]
user32.ShowWindow.restype = wintypes.BOOL

HWND_TOPMOST = wintypes.HWND(-1)
HWND_NOTOPMOST = wintypes.HWND(-2)

# GDI 侧同样必须声明签名：HDC / HBITMAP 都是 64 位句柄，
# 不声明时 ctypes 按 32 位 int 传递，句柄被**静默截断**，
# BitBlt 会往一个错误的 DC 上抓图——表现为"截图成功但内容是别的窗口"。
gdi32.CreateCompatibleDC.argtypes = [wintypes.HDC]
gdi32.CreateCompatibleDC.restype = wintypes.HDC
gdi32.CreateCompatibleBitmap.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int]
gdi32.CreateCompatibleBitmap.restype = wintypes.HBITMAP
gdi32.SelectObject.argtypes = [wintypes.HDC, wintypes.HGDIOBJ]
gdi32.SelectObject.restype = wintypes.HGDIOBJ
gdi32.BitBlt.argtypes = [
    wintypes.HDC,  # hdcDest
    ctypes.c_int,  # x
    ctypes.c_int,  # y
    ctypes.c_int,  # cx
    ctypes.c_int,  # cy
    wintypes.HDC,  # hdcSrc
    ctypes.c_int,  # x1
    ctypes.c_int,  # y1
    wintypes.DWORD,  # rop
]
gdi32.BitBlt.restype = wintypes.BOOL
gdi32.DeleteObject.argtypes = [wintypes.HGDIOBJ]
gdi32.DeleteObject.restype = wintypes.BOOL
gdi32.DeleteDC.argtypes = [wintypes.HDC]
gdi32.DeleteDC.restype = wintypes.BOOL
gdi32.GetDIBits.argtypes = [
    wintypes.HDC,
    wintypes.HBITMAP,
    wintypes.UINT,
    wintypes.UINT,
    ctypes.c_void_p,
    ctypes.c_void_p,
    wintypes.UINT,
]
gdi32.GetDIBits.restype = ctypes.c_int

SRCCOPY = 0x00CC0020
DIB_RGB_COLORS = 0


class RECT(ctypes.Structure):
    _fields_ = [
        ("left", ctypes.c_long),
        ("top", ctypes.c_long),
        ("right", ctypes.c_long),
        ("bottom", ctypes.c_long),
    ]


class POINT(ctypes.Structure):
    """`WindowFromPoint` 要按值传结构体，不能传两个 int。"""

    _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [
        ("biSize", wintypes.DWORD),
        ("biWidth", ctypes.c_long),
        ("biHeight", ctypes.c_long),
        ("biPlanes", wintypes.WORD),
        ("biBitCount", wintypes.WORD),
        ("biCompression", wintypes.DWORD),
        ("biSizeImage", wintypes.DWORD),
        ("biXPelsPerMeter", ctypes.c_long),
        ("biYPelsPerMeter", ctypes.c_long),
        ("biClrUsed", wintypes.DWORD),
        ("biClrImportant", wintypes.DWORD),
    ]


def png_encode(width: int, height: int, rows: list[bytes]) -> bytes:
    """RGB 行 -> PNG 字节流（不依赖 Pillow）。"""
    raw = b"".join(b"\x00" + row for row in rows)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)  # color type 2 = truecolor
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw, 6))
        + chunk(b"IEND", b"")
    )


def find_window(keyword: str) -> int:
    """按标题关键字找第一个可见顶层窗口。"""
    found: list[int] = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def enum_proc(hwnd, _lparam):
        if not user32.IsWindowVisible(hwnd):
            return True
        length = user32.GetWindowTextLengthW(hwnd)
        if length == 0:
            return True
        buf = ctypes.create_unicode_buffer(length + 1)
        user32.GetWindowTextW(hwnd, buf, length + 1)
        if keyword in buf.value:
            found.append(hwnd)
            return False
        return True

    user32.EnumWindows(enum_proc, 0)
    return found[0] if found else 0


# ---------------------------------------------------------------------------
# 遮挡检测（2026-09-24 加）
#
# 为什么必须有：抓图用的是「把窗口置顶再 BitBlt 屏幕」，抓到的是**屏幕像素**。
# 只要那个矩形上有别的窗口压着，拍出来的就是**别人的窗口**——而脚本会打印
# 「已保存」，看起来一切正常。实测就拍到过一台机器上的游戏助手窗口。
#
# **一张拍错对象的截图比没有截图更糟**：它会作为"证据"被引用。
# 所以这里在抓之前逐点确认「这个像素点上是我们的窗口」，不确定就**不产出文件**。
# ---------------------------------------------------------------------------

user32.WindowFromPoint.argtypes = [POINT]
user32.WindowFromPoint.restype = wintypes.HWND
user32.GetAncestor.argtypes = [wintypes.HWND, wintypes.UINT]
user32.GetAncestor.restype = wintypes.HWND
user32.GetWindowTextLengthW.argtypes = [wintypes.HWND]
user32.GetWindowTextLengthW.restype = ctypes.c_int
user32.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
user32.GetWindowTextW.restype = ctypes.c_int

GA_ROOT = 2


def window_title(hwnd: int) -> str:
    if not hwnd:
        return "(无)"
    length = user32.GetWindowTextLengthW(hwnd)
    buf = ctypes.create_unicode_buffer(length + 1)
    user32.GetWindowTextW(hwnd, buf, length + 1)
    return buf.value or "(无标题)"


def root_window_at(x: int, y: int) -> int:
    """屏幕点 (x, y) 上最上层的顶层窗口。"""
    hwnd = user32.WindowFromPoint(POINT(x, y))
    if not hwnd:
        return 0
    return user32.GetAncestor(hwnd, GA_ROOT) or hwnd


def occlusion_report(hwnd: int, rect: "RECT") -> tuple[bool, int, tuple[int, int]]:
    """窗口是否完全可见。返回 (是否可见, 遮挡者 hwnd, 被挡的那个点)。"""
    cx = (rect.left + rect.right) // 2
    cy = (rect.top + rect.bottom) // 2
    inset = 24
    points = [
        (cx, cy),
        (rect.left + inset, rect.top + inset),
        (rect.right - inset, rect.top + inset),
        (rect.left + inset, rect.bottom - inset),
        (rect.right - inset, rect.bottom - inset),
    ]
    for x, y in points:
        other = root_window_at(x, y)
        if other and other != hwnd:
            return False, other, (x, y)
    return True, 0, (0, 0)


def candidate_positions(
    screen_w: int, screen_h: int, win_w: int, win_h: int
) -> list[tuple[str, int, int]]:
    """被遮挡时依次尝试的位置（避开屏幕中央——那里最容易被别的窗口占着）。"""
    margin = 24
    return [
        ("右下角", screen_w - win_w - margin, screen_h - win_h - 64),
        ("左上角", margin, margin),
        ("左下角", margin, screen_h - win_h - 64),
        ("右上角", screen_w - win_w - margin, margin),
    ]


def capture(hwnd: int, rect: RECT, mode: str) -> tuple[int, int, list[bytes]]:
    """抓取窗口内容。

    两种模式的取舍（实测结论）：
    - `printwindow`：让目标窗口自己绘制到内存 DC，不要求它在最前面。
      但 WebView2 走 DirectComposition，PrintWindow 会**返回成功却拿到空白画面**
      （1822x1256 只有 ~8 KB，即纯色）。
    - `topmost`（默认）：把窗口临时置顶再抓屏幕。`SetWindowPos(HWND_TOPMOST)`
      不像 `SetForegroundWindow` 那样要求调用方是前台进程，因此在本环境可用。
    """
    width = rect.right - rect.left
    height = rect.bottom - rect.top

    window_dc = user32.GetWindowDC(hwnd)
    mem_dc = gdi32.CreateCompatibleDC(window_dc)
    bitmap = gdi32.CreateCompatibleBitmap(window_dc, width, height)
    gdi32.SelectObject(mem_dc, bitmap)

    HWND_TOPMOST = wintypes.HWND(-1)
    SWP_NOSIZE, SWP_NOMOVE, SWP_SHOWWINDOW = 0x0001, 0x0002, 0x0040
    raised = False

    if mode == "printwindow":
        user32.PrintWindow(hwnd, mem_dc, 0x00000002)  # PW_RENDERFULLCONTENT
    else:
        user32.SetWindowPos(
            hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_SHOWWINDOW
        )
        raised = True
        time.sleep(1.0)  # 等 WebView2 完成一次合成
        screen_dc = user32.GetDC(0)
        gdi32.BitBlt(mem_dc, 0, 0, width, height, screen_dc, rect.left, rect.top, SRCCOPY)
        user32.ReleaseDC(0, screen_dc)

    if raised:
        user32.SetWindowPos(hwnd, wintypes.HWND(-2), 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE)

    header = BITMAPINFOHEADER()
    header.biSize = ctypes.sizeof(BITMAPINFOHEADER)
    header.biWidth = width
    # 负高度 = 自上而下扫描，省掉一次整图翻转
    header.biHeight = -height
    header.biPlanes = 1
    header.biBitCount = 32
    header.biCompression = 0  # BI_RGB

    buffer = ctypes.create_string_buffer(width * height * 4)
    got = gdi32.GetDIBits(mem_dc, bitmap, 0, height, buffer, ctypes.byref(header), DIB_RGB_COLORS)

    gdi32.DeleteObject(bitmap)
    gdi32.DeleteDC(mem_dc)
    user32.ReleaseDC(hwnd, window_dc)

    if got == 0:
        raise OSError("GetDIBits 返回 0，未取到位图数据")

    # BGRA -> RGB，顺便统计不同颜色数，用来判断画面是不是空白
    rows: list[bytes] = []
    raw = buffer.raw
    distinct: set[bytes] = set()
    for y in range(height):
        offset = y * width * 4
        row = bytearray(width * 3)
        src = raw[offset : offset + width * 4]
        for x in range(width):
            i = x * 4
            row[x * 3 + 0] = src[i + 2]
            row[x * 3 + 1] = src[i + 1]
            row[x * 3 + 2] = src[i + 0]
        distinct.add(bytes(row[::97]))  # 抽样，避免为每个像素建对象
        rows.append(bytes(row))

    if len(distinct) <= 3:
        print(f"  警告：画面只有 {len(distinct)} 种采样颜色，很可能是空白/纯色")
    return width, height, rows


def main() -> int:
    keyword = sys.argv[1] if len(sys.argv) > 1 else "FilePilot"
    out = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else pathlib.Path("window.png")
    mode = "printwindow" if "--printwindow" in sys.argv else "topmost"

    # 高 DPI 屏幕上不做这步，抓到的只是逻辑分辨率的一小块
    try:
        ctypes.windll.shcore.SetProcessDpiAwareness(2)
    except Exception:
        try:
            user32.SetProcessDPIAware()
        except Exception:
            pass

    hwnd = find_window(keyword)
    if not hwnd:
        print(f"未找到标题含 {keyword!r} 的可见窗口")
        return 1

    length = user32.GetWindowTextLengthW(hwnd)
    buf = ctypes.create_unicode_buffer(length + 1)
    user32.GetWindowTextW(hwnd, buf, length + 1)
    print(f"窗口: {buf.value!r}  hwnd={hwnd}  模式={mode}")

    user32.ShowWindow(hwnd, 9)  # SW_RESTORE，确保不是最小化状态
    time.sleep(0.5)

    rect = RECT()
    if not user32.GetWindowRect(hwnd, ctypes.byref(rect)):
        print("GetWindowRect 失败")
        return 1

    # 多显示器下窗口可能落在负坐标的副屏上（本机副屏是 -2048..-341）。
    # `BitBlt` 抓的是主屏 DC，直接用负坐标会拍到错误内容，
    # 所以先把窗口临时挪进主屏，截完再还原。
    SWP_NOSIZE, SWP_NOZORDER = 0x0001, 0x0004
    screen_w = user32.GetSystemMetrics(0)
    screen_h = user32.GetSystemMetrics(1)
    win_w, win_h = rect.right - rect.left, rect.bottom - rect.top

    off_screen = rect.left < 0 or rect.top < 0 or rect.right > screen_w or rect.bottom > screen_h
    saved_pos = (rect.left, rect.top)

    if off_screen:
        target_x, target_y = 60, 60
        print(f"  窗口在 ({saved_pos[0]},{saved_pos[1]})，不在主屏内；临时移到 ({target_x},{target_y}) 截图")
        user32.SetWindowPos(
            hwnd, wintypes.HWND(-1), target_x, target_y, 0, 0, SWP_NOSIZE | 0x0040
        )  # HWND_TOPMOST
        # 窗口原先在屏幕外时 WebView2 不会真正渲染，移回来后要给它时间出画
        time.sleep(3.0)
        rect.left, rect.top = target_x, target_y
        rect.right, rect.bottom = target_x + win_w, target_y + win_h
    else:
        print(f"  窗口在 ({saved_pos[0]},{saved_pos[1]})，已在主屏内")

    # 置顶之后**确认那个矩形上真的是我们的窗口**再抓。
    # 置顶不保证赢过别的置顶窗口（实测本机有游戏助手类窗口压在上面），
    # 而抓屏幕拿到的是"最上层那个"——不检查就会拍出别人的窗口。
    #
    # `--no-move`：只报告遮挡、**不挪窗口**。挪动会让 WebView2 重新合成，
    # 短时间内抓到的是一块白底——那是抓图的产物，不是应用的画面。
    clear, blocker, point = occlusion_report(hwnd, rect)
    if clear:
        print("  遮挡检查：窗口完全可见")
    elif "--no-move" in sys.argv:
        print(
            f"  遮挡检查：({point[0]},{point[1]}) 上压着 {window_title(blocker)!r}"
            "，但指定了 --no-move，不挪窗口（画面可能是遮挡者）"
        )
    else:
        print(f"  遮挡检查：({point[0]},{point[1]}) 上压着 {window_title(blocker)!r}")
        for label, nx, ny in candidate_positions(screen_w, screen_h, win_w, win_h):
            user32.SetWindowPos(
                hwnd, wintypes.HWND(-1), nx, ny, 0, 0, SWP_NOSIZE | 0x0040
            )
            time.sleep(1.5)
            rect.left, rect.top = nx, ny
            rect.right, rect.bottom = nx + win_w, ny + win_h
            clear, blocker, point = occlusion_report(hwnd, rect)
            print(f"  移到{label}再检查：{'可见' if clear else '仍被 ' + window_title(blocker) + ' 遮挡'}")
            if clear:
                break

    if not clear and "--no-move" not in sys.argv:
        # 不产出文件：一张拍错对象的截图会被当成证据引用，比没有更糟。
        print(
            "  放弃：窗口始终被其他窗口遮挡，抓到的会是别人的画面。\n"
            "  请关掉/移开遮挡窗口后重试，或改用 --printwindow（WebView2 下常为空白）。"
        )
        if off_screen:
            user32.SetWindowPos(
                hwnd, wintypes.HWND(-2), saved_pos[0], saved_pos[1], 0, 0, SWP_NOSIZE
            )
        return 2

    width, height, rows = capture(hwnd, rect, mode)

    # 抓完还原到用户原来的位置（只在真的挪过窗口时才需要）
    if (rect.left, rect.top) != saved_pos:
        user32.SetWindowPos(
            hwnd, wintypes.HWND(-2), saved_pos[0], saved_pos[1], 0, 0, SWP_NOSIZE
        )
        print("  已还原窗口位置")
    data = png_encode(width, height, rows)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(data)
    print(f"已保存 {out}  ({width}x{height}, {len(data)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
