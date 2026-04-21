import importlib
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))

ENV_KEYS = [
    "BU_DAEMON_IMPL",
    "BU_NAME",
    "BU_RUST_ADMIN_BIN",
    "BU_RUST_DAEMON_BIN",
    "STUB_GREETING",
    "STUB_UNSUPPORTED_META",
]


class RustModeContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        subprocess.run(
            ["cargo", "build", "--bin", "bhctl"],
            cwd=REPO / "rust",
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        cls.bhctl_bin = REPO / "rust" / "target" / "debug" / "bhctl"
        cls.stub_daemon = REPO / "tests" / "stub_daemon.py"

    def setUp(self):
        self.previous_env = {key: os.environ.get(key) for key in ENV_KEYS}
        self.name = f"rust-contract-{os.getpid()}-{time.time_ns()}"
        self.temp_upload = None
        self.temp_shot = None
        os.environ["BU_DAEMON_IMPL"] = "rust"
        os.environ["BU_NAME"] = self.name
        os.environ["BU_RUST_ADMIN_BIN"] = str(self.bhctl_bin)
        os.environ["BU_RUST_DAEMON_BIN"] = str(self.stub_daemon)
        self.admin, self.helpers = self.reload_modules()

    def tearDown(self):
        try:
            self.admin.restart_daemon(self.name)
        except Exception:
            pass
        if self.temp_upload is not None:
            Path(self.temp_upload.name).unlink(missing_ok=True)
            self.temp_upload.close()
        if self.temp_shot is not None:
            Path(self.temp_shot.name).unlink(missing_ok=True)
            self.temp_shot.close()
        for suffix in ("sock", "pid", "log"):
            Path(f"/tmp/bu-{self.name}.{suffix}").unlink(missing_ok=True)
        for key, value in self.previous_env.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value

    def reload_modules(self):
        admin = importlib.import_module("admin")
        helpers = importlib.import_module("helpers")
        return importlib.reload(admin), importlib.reload(helpers)

    def test_rust_mode_daemon_lifecycle_contract(self):
        self.assertFalse(self.admin.daemon_alive(self.name))

        self.admin.ensure_daemon(
            wait=5.0,
            name=self.name,
            env={"STUB_GREETING": "hello-from-test"},
        )

        self.assertTrue(self.admin.daemon_alive(self.name))
        log_text = Path(f"/tmp/bu-{self.name}.log").read_text(encoding="utf-8")
        self.assertIn("env_STUB_GREETING=hello-from-test", log_text)

        self.admin.ensure_daemon(wait=5.0, name=self.name)
        self.assertTrue(self.admin.daemon_alive(self.name))

        self.admin.restart_daemon(self.name)
        self.assertFalse(self.admin.daemon_alive(self.name))
        self.assertFalse(Path(f"/tmp/bu-{self.name}.sock").exists())
        self.assertFalse(Path(f"/tmp/bu-{self.name}.pid").exists())

    def test_helpers_follow_socket_contract_in_rust_mode(self):
        self.admin.ensure_daemon(wait=5.0, name=self.name)

        self.assertEqual(self.helpers.drain_events(), [])
        initial = self.helpers.page_info()
        self.assertEqual(initial["url"], "about:blank")
        self.assertEqual(initial["title"], "")

        target_id = self.helpers.new_tab("https://example.com")
        self.assertTrue(target_id.startswith("target-"))
        after_new_tab = self.helpers.page_info()
        self.assertEqual(after_new_tab["url"], "https://example.com/")
        self.assertTrue(self.helpers.wait_for_load(timeout=1.0))
        self.assertEqual(self.helpers.js("location.href"), "https://example.com/")

        current = self.helpers.current_tab()
        self.assertEqual(current["targetId"], target_id)
        self.assertEqual(current["url"], "https://example.com/")

        tabs = self.helpers.list_tabs(include_chrome=False)
        self.assertEqual(len(tabs), 1)
        self.assertEqual(tabs[0]["targetId"], target_id)

        blank_target = self.helpers.new_tab()
        self.assertTrue(blank_target.startswith("target-"))
        real_tab = self.helpers.ensure_real_tab()
        self.assertEqual(real_tab["targetId"], target_id)
        session_id = self.helpers.switch_tab(target_id)
        self.assertTrue(session_id.startswith("session-"))
        iframe_target = self.helpers.iframe_target("frames.example.test")
        self.assertEqual(iframe_target, "iframe-1")
        self.assertEqual(self.helpers.js("location.href", target_id=iframe_target), "https://frames.example.test/embed")
        goto_result = self.helpers.goto("https://example.com/?via=typed-goto")
        self.assertEqual(goto_result["frameId"], "frame-1")
        self.assertTrue(self.helpers.wait_for_load(timeout=1.0))
        self.helpers.click(120, 220, button="right", clicks=2)
        self.helpers.type_text("typed text")
        self.helpers.press_key("Enter", modifiers=2)
        self.helpers.dispatch_key("#fake-input", key="Tab", event="keydown")
        self.helpers.scroll(320, 420, dy=64, dx=8)
        self.temp_upload = tempfile.NamedTemporaryFile("w", delete=False)
        self.temp_upload.write("upload payload")
        self.temp_upload.flush()
        self.helpers.upload_file("#file1", self.temp_upload.name, target_id=iframe_target)
        self.temp_shot = tempfile.NamedTemporaryFile(delete=False, suffix=".png")
        self.temp_shot.close()
        self.helpers.screenshot(self.temp_shot.name, full=True)
        self.assertEqual(Path(self.temp_shot.name).read_bytes(), b"stub-shot")

        after_nav = self.helpers.page_info()
        self.assertEqual(after_nav["url"], "https://example.com/?via=typed-goto")
        self.assertIn("Example Domain", after_nav["title"])

        log_text = Path(f"/tmp/bu-{self.name}.log").read_text(encoding="utf-8")
        for marker in (
            "typed_meta=page_info",
            "typed_meta=new_tab",
            "typed_meta=current_tab",
            "typed_meta=list_tabs",
            "typed_meta=switch_tab",
            "typed_meta=ensure_real_tab",
            "typed_meta=iframe_target",
            "typed_meta=wait_for_load",
            "typed_meta=goto",
            "typed_meta=js",
            "typed_meta=click",
            "typed_meta=type_text",
            "typed_meta=press_key",
            "typed_meta=dispatch_key",
            "typed_meta=scroll",
            "typed_meta=upload_file",
            "typed_meta=screenshot",
        ):
            self.assertIn(marker, log_text)

    def test_helpers_fall_back_when_typed_meta_is_unsupported(self):
        self.admin.ensure_daemon(
            wait=5.0,
            name=self.name,
            env={"STUB_UNSUPPORTED_META": "page_info,goto,js,wait_for_load"},
        )

        navigate = self.helpers.cdp("Page.navigate", url="https://example.com/?via=raw-cdp")
        self.assertEqual(navigate["frameId"], "frame-1")

        initial = self.helpers.page_info()
        self.assertEqual(initial["url"], "https://example.com/?via=raw-cdp")
        self.assertEqual(initial["title"], "Example Domain")

        goto_result = self.helpers.goto("https://example.com/?via=fallback")
        self.assertEqual(goto_result["frameId"], "frame-1")
        self.assertTrue(self.helpers.wait_for_load(timeout=1.0))
        self.assertEqual(self.helpers.js("location.href"), "https://example.com/?via=fallback")

        log_text = Path(f"/tmp/bu-{self.name}.log").read_text(encoding="utf-8")
        for marker in (
            "unsupported_meta=page_info",
            "unsupported_meta=goto",
            "unsupported_meta=wait_for_load",
            "unsupported_meta=js",
            "raw_method=Page.navigate",
            "raw_method=Runtime.evaluate",
        ):
            self.assertIn(marker, log_text)
        self.assertEqual(log_text.count("unsupported_meta=js"), 1)


if __name__ == "__main__":
    unittest.main()
