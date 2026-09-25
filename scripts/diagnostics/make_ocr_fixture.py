"""生成 T11 的 OCR 测试夹具：一张清晰的白底黑字中文 PNG。

为什么必须做成夹具文件：Rust 侧（image crate）只能生成像素，没有字体渲染，
造不出一张「有中文文字」的图。所以这张图由 GDI/字体渲染一次、入库为夹具，
测试用 `include_bytes!` 读它。
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

PROJECT = Path(r"C:\Users\cj169\Desktop\开发\filepilot")
FIXTURES = PROJECT / "src-tauri" / "tests" / "fixtures"
FIXTURES.mkdir(parents=True, exist_ok=True)

FONT_PATH = r"C:\Windows\Fonts\msyh.ttc"

WIDTH, HEIGHT = 560, 200
LINES = [("会议纪要", 30), ("项目进度报告 2026", 110)]

image = Image.new("RGB", (WIDTH, HEIGHT), "white")
draw = ImageDraw.Draw(image)
font = ImageFont.truetype(FONT_PATH, 44)

for text, y in LINES:
    draw.text((24, y), text, fill="black", font=font)

out = FIXTURES / "ocr-chinese.png"
image.save(out, "PNG")

# 故意留一条纯白图（无文字）也在夹具里：它是「OCR 跑成功但读不出东西」
# 这条分支的输入，和「OCR 不可用」是完全不同的两种情况。
blank = Image.new("RGB", (320, 120), "white")
blank_out = FIXTURES / "ocr-blank.png"
blank.save(blank_out, "PNG")

print(f"wrote {out} ({out.stat().st_size} bytes)")
print(f"wrote {blank_out} ({blank_out.stat().st_size} bytes)")
