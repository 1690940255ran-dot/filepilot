"""在宿主机的局域网地址上提供一个静态文件服务，供虚拟机下载验收包。

为什么需要它：虚拟机装了/没装 VMware Tools 决定能否拖放，
但不该把"能不能验收"押在 Tools 上。HTTP 下载是最不依赖的通道。

绑定到**局域网地址**而不是 127.0.0.1：虚拟机的 NAT/桥接网络要能访问到宿主。
地址不写死，启动时探测并打印，避免换网络后失效。
"""

from __future__ import annotations

import functools
import http.server
import socket
import socketserver
import sys

PORT = 8080
DIRECTORY = r"C:\fp-verify"


def lan_address() -> str:
    """探测本机在局域网上的地址。

    连一个外部地址（不会真的发包）只为让系统选出"出去时会用哪张网卡"。
    失败时退回回环地址，至少本机可用。
    """
    probe = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        probe.connect(("192.168.1.1", 80))
        return probe.getsockname()[0]
    except OSError:
        return "127.0.0.1"
    finally:
        probe.close()


class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, fmt: str, *args: object) -> None:  # noqa: A003
        # 每次都打印会淹没输出；只在出错时留痕
        if not str(args[1] if len(args) > 1 else "").startswith("2"):
            super().log_message(fmt, *args)


def main() -> int:
    host = lan_address()
    handler = functools.partial(Handler, directory=DIRECTORY)

    try:
        with socketserver.TCPServer((host, PORT), handler) as httpd:
            print(f"服务目录: {DIRECTORY}")
            print(f"虚拟机里用浏览器打开: http://{host}:{PORT}/")
            print("按 Ctrl+C 停止")
            sys.stdout.flush()
            httpd.serve_forever()
    except OSError as error:
        print(f"启动失败: {error}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
