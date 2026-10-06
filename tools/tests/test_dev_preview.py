"""Exercise the real adapter process with a retained publisher-owned listener."""

import errno
import http.client
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import tomllib
import unittest


SOURCE = Path(__file__).resolve().parents[1] / "dev-preview.py"
SPEC = importlib.util.spec_from_file_location("dev_preview", SOURCE)
ADAPTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADAPTER)


class PreviewTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.worktree = Path(self.temporary.name).resolve()
        self.web = self.worktree / "apps/analytical-workspace-lab/web"
        self.web.mkdir(parents=True)
        tools = self.worktree / "tools"
        tools.mkdir()
        self.script = tools / "dev-preview.py"
        shutil.copyfile(SOURCE, self.script)
        for name in ADAPTER.ASSETS:
            path = self.web / name
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(b"\x00asm\x01\x00\x00\x00" if name.endswith(".wasm")
                             else f"fixture for {name}\n".encode())
        self.listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.listener.bind(("127.0.0.1", 0))
        self.listener.listen()
        self.addCleanup(self.listener.close)
        self.port = self.listener.getsockname()[1]
        self.identity = {
            "project": ADAPTER.PROJECT,
            "worktree": str(self.worktree),
            "run_id": "0123456789abcdef0123456789abcdef",
            "origin": "https://p-example.preview.invalid",
        }

    def start(self, overrides=None, arguments=()):
        environment = os.environ.copy()
        environment.update({f"DEV_PREVIEW_{key.upper()}": value
                            for key, value in self.identity.items()})
        environment.update({"DEV_PREVIEW_LISTEN_FD": str(self.listener.fileno()),
                            "DEV_PREVIEW_PORT": str(self.port)})
        environment.update(overrides or {})
        process = subprocess.Popen(
            [sys.executable, str(self.script), *arguments], cwd=self.worktree,
            env=environment, pass_fds=(self.listener.fileno(),),
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )

        def stop():
            if process.poll() is None:
                process.terminate()
            process.communicate(timeout=5)

        self.addCleanup(stop)
        return process

    def test_record_desk_route_adopts_listener_and_only_serves_consumer_assets(self):
        consumer = self.worktree / "consumers/record-desk"
        web = consumer / "web"
        (web / "pkg").mkdir(parents=True)
        for name in ["index.html", "bootstrap.js", "pkg/record_desk.js", "pkg/record_desk_bg.wasm"]:
            (web / name).write_bytes(b"\x00asm\x01\x00\x00\x00" if name.endswith(".wasm") else b"consumer fixture")
        process = self.start()
        status, _, body = self.wait_ready(process)
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(body), self.identity)
        self.assertEqual(self.request("/record-desk/pkg/record_desk_bg.wasm")[0], 200)
        for path in ["/record-desk/Cargo.toml", "/record-desk/../../Cargo.toml", "/record-desk/pkg/analytical_workspace_lab.js"]:
            self.assertEqual(self.request(path)[0], 404)
        (web / "bootstrap.js").unlink()
        (web / "bootstrap.js").symlink_to(self.web / "bootstrap.js")
        self.assertEqual(self.request("/record-desk/bootstrap.js")[0], 404)
        self.assertEqual(self.request(ADAPTER.READINESS_PATH)[0], 200)

    def gallery_assets(self):
        web = self.worktree / "apps/polyorama-gallery/web"
        (web / "pkg").mkdir(parents=True)
        for name in ADAPTER.GALLERY_ASSETS:
            (web / name).write_bytes(b"\x00asm\x01\x00\x00\x00" if name.endswith(".wasm")
                                    else f"gallery fixture {name}".encode())
        return web

    def test_gallery_route_serves_only_allowlisted_regular_assets(self):
        web = self.gallery_assets()
        self.wait_ready(self.start())
        self.assertEqual(self.request("/gallery/")[2], (web / "index.html").read_bytes())
        for name, mime in ADAPTER.GALLERY_ASSETS.items():
            with self.subTest(name=name):
                status, headers, body = self.request("/gallery/" + name + "?reload=1")
                self.assertEqual(status, 200)
                self.assertEqual(headers["Content-Type"], mime)
                self.assertEqual(headers["Cache-Control"], "no-store")
                self.assertEqual(headers["X-Content-Type-Options"], "nosniff")
                self.assertEqual(body, (web / name).read_bytes())
                status, headers, body = self.request("/gallery/" + name, method="HEAD")
                self.assertEqual(status, 200)
                self.assertEqual(body, b"")
                self.assertEqual(int(headers["Content-Length"]), (web / name).stat().st_size)
        for path in ("/gallery/Cargo.toml", "/gallery/pkg/record_desk.js", "/gallery/pkg/",
                     "/gallery/../bootstrap.js", "/gallery/%2e%2e/bootstrap.js",
                     "/gallery/pkg\\secret.js", "/gallery/extra.txt"):
            with self.subTest(path=path):
                self.assertEqual(self.request(path)[0], 404)
        self.assert_port_owned()

    def test_gallery_route_rejects_missing_invalid_and_symlink_assets(self):
        web = self.gallery_assets()
        self.wait_ready(self.start())
        wasm = web / "pkg/polyorama_gallery_bg.wasm"
        for contents in (None, b"", b"not WASM"):
            with self.subTest(contents=contents):
                wasm.unlink(missing_ok=True)
                if contents is not None:
                    wasm.write_bytes(contents)
                self.assertEqual(self.request("/gallery/pkg/polyorama_gallery_bg.wasm")[0], 404)
        wasm.unlink()
        os.mkfifo(wasm)
        self.assertEqual(self.request("/gallery/pkg/polyorama_gallery_bg.wasm")[0], 404)
        asset = web / "bootstrap.js"
        asset.unlink()
        asset.symlink_to(self.web / "bootstrap.js")
        self.assertEqual(self.request("/gallery/bootstrap.js")[0], 404)
        shutil.rmtree(web / "pkg")
        (web / "pkg").symlink_to(self.web / "pkg", target_is_directory=True)
        self.assertEqual(self.request("/gallery/pkg/polyorama_gallery.js")[0], 404)
        shutil.rmtree(web)
        web.symlink_to(self.web, target_is_directory=True)
        self.assertEqual(self.request("/gallery/bootstrap.js")[0], 404)
        # Gallery availability is independent of the Lab's readiness contract.
        self.assertEqual(self.request(ADAPTER.READINESS_PATH)[0], 200)

    def request(self, path, method="GET"):
        connection = http.client.HTTPConnection("127.0.0.1", self.port, timeout=0.2)
        try:
            connection.request(method, path)
            response = connection.getresponse()
            return response.status, dict(response.getheaders()), response.read()
        finally:
            connection.close()

    def wait_ready(self, process):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if process.poll() is not None:
                self.fail(f"adapter exited: {process.communicate()!r}")
            try:
                response = self.request(ADAPTER.READINESS_PATH)
                if response[0] == 200:
                    return response
            except (OSError, http.client.HTTPException):
                pass
            time.sleep(0.01)
        self.fail("adapter did not return ready")

    def assert_port_owned(self):
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as competing:
            with self.assertRaises(OSError) as caught:
                competing.bind(("127.0.0.1", self.port))
            self.assertEqual(caught.exception.errno, errno.EADDRINUSE)

    def test_inherited_socket_readiness_assets_and_publisher_ownership(self):
        self.assert_port_owned()
        process = self.start()
        status, headers, body = self.wait_ready(process)
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(body), self.identity)
        self.assertEqual(headers["Cache-Control"], "no-store")
        self.assert_port_owned()
        for name, mime in ADAPTER.ASSETS.items():
            with self.subTest(name=name):
                status, headers, body = self.request("/" + name)
                self.assertEqual(status, 200)
                self.assertEqual(headers["Content-Type"], mime)
                self.assertEqual(headers["Cache-Control"], "no-store")
                self.assertEqual(headers["X-Content-Type-Options"], "nosniff")
                self.assertEqual(body, (self.web / name).read_bytes())
                status, headers, body = self.request("/" + name, method="HEAD")
                self.assertEqual(status, 200)
                self.assertEqual(body, b"")
                self.assertEqual(int(headers["Content-Length"]), (self.web / name).stat().st_size)
        self.assertEqual(self.request("/")[2], (self.web / "index.html").read_bytes())
        self.assertEqual(self.request("/worker.js?reload=1")[0], 200)
        process.terminate()
        process.communicate(timeout=5)
        # The gateway/publisher's original descriptor still owns the allocation.
        self.assert_port_owned()

    def test_unpublished_paths_and_traversal_are_rejected(self):
        (self.worktree / "secret.txt").write_text("must not be published")
        (self.web / "extra.txt").write_text("also not published")
        (self.web / "escape.js").symlink_to(self.worktree / "secret.txt")
        self.wait_ready(self.start())
        for path in ("/../secret.txt", "/%2e%2e/secret.txt", "/pkg/../index.html",
                     "/%2f..%2fsecret.txt", "/pkg\\secret.txt", "/pkg/",
                     "/extra.txt", "/escape.js", "/.git/config", "/tools/dev-preview.py",
                     "/evidence/report.json", "/pkg/%252e%252e/secret.txt", "/%00"):
            with self.subTest(path=path):
                status, headers, body = self.request(path)
                self.assertEqual(status, 404)
                self.assertEqual(headers["Cache-Control"], "no-store")
                self.assertNotIn(b"must not be published", body)

    def test_readiness_fails_after_any_required_asset_is_removed(self):
        self.wait_ready(self.start())
        for name in ADAPTER.ASSETS:
            with self.subTest(name=name):
                path = self.web / name
                original = path.read_bytes()
                path.unlink()
                self.assertEqual(self.request(ADAPTER.READINESS_PATH)[0], 503)
                self.assertEqual(self.request("/" + name)[0], 404)
                path.write_bytes(original)
                self.assertEqual(self.request(ADAPTER.READINESS_PATH)[0], 200)

    def test_symlink_asset_directory_and_web_root_are_rejected(self):
        outside = self.worktree / "outside"
        outside.mkdir()
        (outside / "private.js").write_bytes(b"private contents")
        self.wait_ready(self.start())
        asset = self.web / "bootstrap.js"
        asset.unlink()
        asset.symlink_to(outside / "private.js")
        self.assertEqual(self.request("/bootstrap.js")[0], 404)
        self.assertEqual(self.request(ADAPTER.READINESS_PATH)[0], 503)
        asset.unlink()
        asset.write_bytes(b"restored")
        shutil.rmtree(self.web / "pkg")
        (self.web / "pkg").symlink_to(outside, target_is_directory=True)
        self.assertEqual(self.request("/pkg/analytical_workspace_lab.js")[0], 404)
        self.assertEqual(self.request(ADAPTER.READINESS_PATH)[0], 503)
        # A new process must also refuse a symlink at the static root boundary.
        shutil.rmtree(self.web)
        self.web.symlink_to(outside, target_is_directory=True)
        process = self.start()
        _, errors = process.communicate(timeout=5)
        self.assertEqual(process.returncode, 1, errors)

    def test_missing_empty_invalid_and_non_regular_assets_fail_startup(self):
        wasm = self.web / "pkg/analytical_workspace_lab_bg.wasm"
        for contents in (None, b"", b"not WASM"):
            with self.subTest(contents=contents):
                wasm.unlink(missing_ok=True)
                if contents is not None:
                    wasm.write_bytes(contents)
                process = self.start()
                _, errors = process.communicate(timeout=5)
                self.assertEqual(process.returncode, 1, errors)
        wasm.unlink()
        os.mkfifo(wasm)
        process = self.start()
        _, errors = process.communicate(timeout=5)
        self.assertEqual(process.returncode, 1, errors)

    def test_wrong_identity_or_non_listening_descriptor_is_rejected(self):
        for key, value in (("PROJECT", "other-project"), ("WORKTREE", "/other-checkout"),
                           ("RUN_ID", ""), ("ORIGIN", "http://preview.invalid"),
                           ("PORT", str(self.port + 1))):
            with self.subTest(key=key):
                process = self.start({f"DEV_PREVIEW_{key}": value})
                _, errors = process.communicate(timeout=5)
                self.assertEqual(process.returncode, 1, errors)
                self.assert_port_owned()
        self.listener.close()
        self.listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.addCleanup(self.listener.close)
        self.listener.bind(("127.0.0.1", 0))
        self.port = self.listener.getsockname()[1]
        process = self.start()
        _, errors = process.communicate(timeout=5)
        self.assertEqual(process.returncode, 1, errors)

    def test_manifest_and_existing_relative_worker_urls(self):
        root = SOURCE.parent.parent
        manifest = tomllib.loads((root / ".dev-preview.toml").read_text())
        # Gateway v1 accepts at most 31 lower-case project-identifier characters.
        # Comparing only the adapter's constant would miss a shared invalid ID.
        self.assertRegex(manifest["project"], r"\A[a-z][a-z0-9-]{0,30}\Z")
        self.assertEqual(manifest["schema_version"], 1)
        self.assertEqual(set(manifest), {"schema_version", "project", "command", "cwd", "readiness"})
        readiness = manifest["readiness"]
        self.assertIsNotNone(re.fullmatch(r"/[A-Za-z0-9_/-]{1,120}", readiness["path"]))
        self.assertNotIn("..", readiness["path"])
        self.assertEqual(set(readiness), {"path", "timeout_seconds"})
        self.assertIs(type(readiness["timeout_seconds"]), int)
        self.assertGreaterEqual(readiness["timeout_seconds"], 1)
        self.assertLessEqual(readiness["timeout_seconds"], 60)
        self.assertEqual(manifest["project"], ADAPTER.PROJECT)
        self.assertEqual(manifest["command"], ["python3", "tools/dev-preview.py"])
        self.assertEqual(manifest["readiness"]["path"], ADAPTER.READINESS_PATH)
        bootstrap = (root / "apps/analytical-workspace-lab/web/bootstrap.js").read_text()
        worker = (root / "apps/analytical-workspace-lab/web/worker.js").read_text()
        self.assertIn("new URL('./worker.js', import.meta.url)", bootstrap)
        self.assertIn("'./worker-pkg/polyorama_tile_worker.js'", worker)


if __name__ == "__main__":
    unittest.main()
