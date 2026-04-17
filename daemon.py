"""Long-running CDP WebSocket holder + Unix socket relay.

Chrome 144+: reads ws URL from <profile>/DevToolsActivePort (written when user
enables chrome://inspect/#remote-debugging). Avoids the per-connect "Allow?"
dialog that the classic /json/version endpoint would trigger.
"""
import asyncio, json, os, sys
from collections import deque
from pathlib import Path

from cdp_use.client import CDPClient

SOCK = "/tmp/harnesless.sock"
LOG = "/tmp/harnesless.log"
BUF = 500
PROFILES = [
    Path.home() / "Library/Application Support/Google/Chrome",
    Path.home() / ".config/google-chrome",
    Path.home() / "AppData/Local/Google/Chrome/User Data",
]
INTERNAL = ("chrome://", "chrome-untrusted://", "devtools://", "chrome-extension://", "about:")


def log(msg):
    open(LOG, "a").write(f"{msg}\n")


def get_ws_url():
    for base in PROFILES:
        try:
            port, path = (base / "DevToolsActivePort").read_text().strip().split("\n", 1)
        except (FileNotFoundError, NotADirectoryError):
            continue
        return f"ws://127.0.0.1:{port.strip()}{path.strip()}"
    raise RuntimeError(f"DevToolsActivePort not found in {[str(p) for p in PROFILES]} — enable chrome://inspect/#remote-debugging")


def is_real_page(t):
    return t["type"] == "page" and not t.get("url", "").startswith(INTERNAL)


class Daemon:
    def __init__(self):
        self.cdp = None
        self.session = None
        self.events = deque(maxlen=BUF)

    async def attach_first_page(self):
        """Attach to a real page (or any page). Sets self.session. Returns attached target or None."""
        targets = (await self.cdp.send_raw("Target.getTargets"))["targetInfos"]
        pages = [t for t in targets if is_real_page(t)] or [t for t in targets if t["type"] == "page"]
        if not pages:
            self.session = None
            return None
        self.session = (await self.cdp.send_raw(
            "Target.attachToTarget", {"targetId": pages[0]["targetId"], "flatten": True}
        ))["sessionId"]
        log(f"attached {pages[0]['targetId']} ({pages[0].get('url','')[:80]}) session={self.session}")
        for d in ("Page", "DOM", "Runtime", "Network"):
            try:
                await self.cdp.send_raw(f"{d}.enable", session_id=self.session)
            except Exception as e:
                log(f"enable {d}: {e}")
        return pages[0]

    async def start(self):
        url = get_ws_url()
        log(f"connecting to {url}")
        self.cdp = CDPClient(url)
        await self.cdp.start()
        await self.attach_first_page()
        orig = self.cdp._event_registry.handle_event
        async def tap(method, params, session_id=None):
            self.events.append({"method": method, "params": params, "session_id": session_id})
            return await orig(method, params, session_id)
        self.cdp._event_registry.handle_event = tap

    async def handle(self, req):
        meta = req.get("meta")
        if meta == "drain_events":
            out = list(self.events); self.events.clear()
            return {"events": out}
        if meta == "session":     return {"session_id": self.session}
        if meta == "set_session": self.session = req.get("session_id"); return {"session_id": self.session}
        if meta == "shutdown":    return {"ok": True, "_shutdown": True}

        method = req["method"]
        params = req.get("params") or {}
        # Browser-level Target.* calls must not use a session (stale or otherwise).
        # For everything else, explicit session in req wins; else default.
        sid = None if method.startswith("Target.") else (req.get("session_id") or self.session)
        try:
            return {"result": await self.cdp.send_raw(method, params, session_id=sid)}
        except Exception as e:
            msg = str(e)
            if "Session with given id not found" in msg and sid == self.session and sid:
                log(f"stale session {sid}, re-attaching")
                if await self.attach_first_page():
                    return {"result": await self.cdp.send_raw(method, params, session_id=self.session)}
            return {"error": msg}


async def serve(d):
    if os.path.exists(SOCK):
        os.unlink(SOCK)

    async def handler(reader, writer):
        try:
            line = await reader.readline()
            if not line: return
            resp = await d.handle(json.loads(line))
            writer.write((json.dumps(resp, default=str) + "\n").encode())
            await writer.drain()
            if resp.get("_shutdown"):
                asyncio.get_event_loop().stop()
        except Exception as e:
            log(f"conn: {e}")
            try:
                writer.write((json.dumps({"error": str(e)}) + "\n").encode())
                await writer.drain()
            except Exception:
                pass
        finally:
            writer.close()

    server = await asyncio.start_unix_server(handler, path=SOCK)
    os.chmod(SOCK, 0o600)
    log(f"listening on {SOCK}")
    async with server:
        await server.serve_forever()


async def main():
    d = Daemon()
    await d.start()
    await serve(d)


if __name__ == "__main__":
    open(LOG, "w").close()
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
    except Exception as e:
        log(f"fatal: {e}")
        sys.exit(1)
