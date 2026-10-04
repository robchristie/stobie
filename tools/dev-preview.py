#!/usr/bin/env python3
"""Serve only the built Lab through the gateway's inherited listening socket."""

import json
import os
from pathlib import Path
import re
import socket
import socketserver
import stat
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import unquote, urlsplit


PROJECT = "polyorama-lab"
READINESS_PATH = "/_dev_preview/ready"
ASSETS = {
    "index.html": "text/html; charset=utf-8",
    "styles.css": "text/css; charset=utf-8",
    "bootstrap.js": "text/javascript; charset=utf-8",
    "browser-startup.js": "text/javascript; charset=utf-8",
    "worker.js": "text/javascript; charset=utf-8",
    "pkg/analytical_workspace_lab.js": "text/javascript; charset=utf-8",
    "pkg/analytical_workspace_lab_bg.wasm": "application/wasm",
    "worker-pkg/polyorama_tile_worker.js": "text/javascript; charset=utf-8",
    "worker-pkg/polyorama_tile_worker_bg.wasm": "application/wasm",
}
DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC


def open_web_root(worktree):
    """Pin the Lab directory, rejecting symlinks in every worktree-relative part."""
    descriptor = os.open(worktree, DIRECTORY_FLAGS)
    try:
        for part in ("apps", "analytical-workspace-lab", "web"):
            child = os.open(part, DIRECTORY_FLAGS, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def open_asset(root, name, read_content=True):
    """Open an allowlisted regular file without following directory/file symlinks."""
    if name not in ASSETS:
        raise FileNotFoundError("asset is not published")
    descriptor = os.dup(root)
    try:
        parts = name.split("/")
        for part in parts[:-1]:
            child = os.open(part, DIRECTORY_FLAGS, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
        file_descriptor = os.open(
            parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK,
            dir_fd=descriptor,
        )
    finally:
        os.close(descriptor)
    with os.fdopen(file_descriptor, "rb") as asset:
        metadata = os.fstat(asset.fileno())
        if not stat.S_ISREG(metadata.st_mode) or not metadata.st_size:
            raise ValueError(f"empty or non-regular Lab asset: {name}")
        if name.endswith(".wasm") and asset.read(8) != b"\x00asm\x01\x00\x00\x00":
            raise ValueError(f"invalid WASM Lab asset: {name}")
        if read_content:
            asset.seek(0)
            return asset.read()


def check_assets(root):
    for name in ASSETS:
        open_asset(root, name, read_content=False)


class PreviewHandler(BaseHTTPRequestHandler):
    server_version = "PolyoramaPreview"
    sys_version = ""

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        super().end_headers()

    def do_GET(self):
        self.serve_asset()

    def do_HEAD(self):
        self.serve_asset()

    def respond(self, status, body, content_type):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def serve_asset(self):
        try:
            url = urlsplit(self.path)
            path = unquote(url.path, errors="strict")
            if (url.scheme or url.netloc or not path.startswith("/")
                    or "\\" in path or "\x00" in path
                    or any(part in (".", "..") for part in path.split("/"))):
                raise ValueError("invalid asset path")
        except (ValueError, UnicodeError):
            self.respond(404, b"not found\n", "text/plain; charset=utf-8")
            return
        if path == READINESS_PATH:
            try:
                check_assets(self.server.web_root)
            except (OSError, ValueError):
                self.respond(503, b'{"error":"Lab assets unavailable; run cargo xtask build-web"}',
                             "application/json")
                return
            body = json.dumps(self.server.identity).encode("utf-8")
            self.respond(200, body, "application/json")
            return
        name = "index.html" if path == "/" else path[1:]
        try:
            body = open_asset(self.server.web_root, name)
        except (OSError, ValueError):
            self.respond(404, b"not found\n", "text/plain; charset=utf-8")
            return
        self.respond(200, body, ASSETS[name])


class PreviewServer(ThreadingHTTPServer):
    def __init__(self, listener, web_root, identity):
        # BaseServer owns dispatch only: TCPServer.__init__ would create a socket.
        # Adopt the transferred descriptor directly; never close/rebind its port.
        socketserver.BaseServer.__init__(self, listener.getsockname(), PreviewHandler)
        self.socket = listener
        self.server_name = "localhost"
        self.server_port = self.server_address[1]
        self.web_root = web_root
        self.identity = identity


def main():
    worktree = Path(__file__).resolve().parent.parent
    identity = {
        key: os.environ[f"DEV_PREVIEW_{key.upper()}"]
        for key in ("project", "worktree", "run_id", "origin")
    }
    if identity["project"] != PROJECT or identity["worktree"] != str(worktree):
        raise ValueError("preview project/worktree does not match this Lab checkout")
    if not re.fullmatch(r"[0-9a-f]{32}", identity["run_id"]):
        raise ValueError("preview run ID must be the launcher's 128-bit hex nonce")
    origin = urlsplit(identity["origin"])
    if (origin.scheme != "https" or not origin.hostname or origin.username
            or origin.password or origin.path or origin.query or origin.fragment):
        raise ValueError("preview origin must be the launcher's HTTPS origin")
    web_root = open_web_root(worktree)
    try:
        check_assets(web_root)
        # socket(fileno=...) adopts the descriptor; it does not bind or listen.
        with socket.socket(fileno=int(os.environ["DEV_PREVIEW_LISTEN_FD"])) as listener:
            if (listener.family != socket.AF_INET or listener.type != socket.SOCK_STREAM
                    or listener.getsockopt(socket.SOL_SOCKET, socket.SO_ACCEPTCONN) != 1
                    or listener.getsockname() != ("127.0.0.1", int(os.environ["DEV_PREVIEW_PORT"]))):
                raise ValueError("preview descriptor must be the launcher's listening loopback TCP socket")
            with PreviewServer(listener, web_root, identity) as server:
                print("Analytical Workspace Lab private preview ready", flush=True)
                server.serve_forever()
    finally:
        os.close(web_root)


if __name__ == "__main__":
    try:
        main()
    except (KeyError, OSError, ValueError) as error:
        print(f"Lab private preview: {error}. Build with cargo xtask build-web before dev-preview up.",
              file=sys.stderr)
        sys.exit(1)
