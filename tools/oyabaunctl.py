#!/usr/bin/env python3
"""Oyabaun dev control: launch / stop local relay + static client, rebuild artifacts, Docker relay."""

from __future__ import annotations

import argparse
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path


def repo_root() -> Path:
    p = Path(__file__).resolve().parent.parent
    if not (p / "relay" / "go.mod").is_file() or not (p / "client" / "Cargo.toml").is_file():
        sys.stderr.write("oyabaunctl must live in <repo>/tools/ (relay/go.mod and client/Cargo.toml not found)\n")
        sys.exit(1)
    return p


ROOT = repo_root()
STATEDIR = ROOT / ".oyabaun"
STATE_PATH = STATEDIR / "state.json"
RELAY_LOG = STATEDIR / "relay.log"
HTTP_LOG = STATEDIR / "http.log"


def _relay_binary() -> Path:
    if sys.platform == "win32":
        return STATEDIR / "oyabaun-relay.exe"
    return STATEDIR / "oyabaun-relay"


def _ensure_relay_binary() -> Path:
    STATEDIR.mkdir(parents=True, exist_ok=True)
    exe = _relay_binary()
    if exe.is_file():
        return exe
    subprocess.run(
        ["go", "build", "-o", str(exe), "./cmd/oyabaun-relay"],
        cwd=ROOT / "relay",
        check=True,
    )
    return exe


def _load_state() -> dict | None:
    if not STATE_PATH.is_file():
        return None
    try:
        return json.loads(STATE_PATH.read_text())
    except (json.JSONDecodeError, OSError):
        return None


def _save_state(data: dict) -> None:
    STATEDIR.mkdir(parents=True, exist_ok=True)
    STATE_PATH.write_text(json.dumps(data, indent=2))


def _clear_state() -> None:
    try:
        STATE_PATH.unlink()
    except OSError:
        pass


def _pid_alive(pid: int) -> bool:
    if pid <= 0:
        return False
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    return True


def _kill_listeners_on_port(port: int) -> None:
    if port <= 0:
        return
    try:
        r = subprocess.run(
            ["lsof", "-t", f"-iTCP:{port}", "-sTCP:LISTEN"],
            capture_output=True,
            text=True,
            timeout=6,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return
    for tok in r.stdout.split():
        try:
            p = int(tok)
            if p > 0:
                try:
                    os.kill(p, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        except ValueError:
            pass


def _stop_session_leader(pid: int) -> None:
    """Stop PID and its whole process group (matches start_new_session Popen)."""
    if pid <= 0:
        return
    if sys.platform == "win32":
        subprocess.run(
            ["taskkill", "/PID", str(pid), "/T", "/F"],
            capture_output=True,
            check=False,
        )
        return
    try:
        pgid = os.getpgid(pid)
    except ProcessLookupError:
        return
    try:
        os.killpg(pgid, signal.SIGTERM)
    except ProcessLookupError:
        return
    except PermissionError as e:
        sys.stderr.write(f"oyabaunctl: cannot signal pgid {pgid}: {e}\n")
        return
    deadline = time.time() + 1.75
    while time.time() < deadline:
        try:
            os.killpg(pgid, 0)
        except ProcessLookupError:
            return
        time.sleep(0.06)
    try:
        os.killpg(pgid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def cmd_status(_: argparse.Namespace) -> None:
    st = _load_state()
    if not st:
        print("nothing tracked (no .oyabaun/state.json)")
        return
    mode = st.get("mode", "local")
    print(f"mode: {mode}")
    if mode == "docker":
        print(f"  compose: {st.get('compose_file')}")
        print("  stop with: oyabaunctl stop")
        return
    rp = st.get("relay_pid")
    hp = st.get("http_pid")
    port = st.get("http_port", 8080)
    rap = st.get("relay_port", 8765)
    if rp and _pid_alive(rp):
        print(f"  relay pid {rp} (ws http://127.0.0.1:{rap}/ws)")
    elif rp:
        print(f"  relay pid {rp} (dead, stale state)")
    if hp and _pid_alive(hp):
        print(f"  web   pid {hp} (http://127.0.0.1:{port}/)")
    elif hp:
        print(f"  web   pid {hp} (dead, stale state)")


def cmd_stop(_: argparse.Namespace) -> None:
    st = _load_state()
    relay_port = 8765
    http_port = 8080
    if st:
        relay_port = int(st.get("relay_port", 8765))
        http_port = int(st.get("http_port", 8080))
    if not st:
        print("no state file — sweeping common ports anyway")
        _kill_listeners_on_port(8765)
        _kill_listeners_on_port(8080)
        _clear_state()
        print("swept :8765 and :8080 listeners (if lsof found any)")
        return
    mode = st.get("mode", "local")
    if mode == "docker":
        cf = ROOT / st.get("compose_file", "infra/docker-compose.yml")
        subprocess.run(
            ["docker", "compose", "-f", str(cf), "down"],
            cwd=ROOT,
            check=False,
        )
        _clear_state()
        print("docker compose down")
        return
    rp, hp = st.get("relay_pid"), st.get("http_pid")
    if rp:
        _stop_session_leader(int(rp))
    if hp:
        _stop_session_leader(int(hp))
    time.sleep(0.15)
    _kill_listeners_on_port(relay_port)
    _kill_listeners_on_port(http_port)
    _clear_state()
    print("stopped relay + web (process groups + port sweep)")


def cmd_rebuild(ns: argparse.Namespace) -> None:
    if ns.wasm:
        subprocess.run(
            ["wasm-pack", "build", "--target", "web", "--out-dir", "pkg"],
            cwd=ROOT / "client",
            check=True,
        )
        print("wasm client rebuilt -> client/pkg/")
    if ns.relay:
        STATEDIR.mkdir(parents=True, exist_ok=True)
        exe = _relay_binary()
        subprocess.run(
            ["go", "build", "-o", str(exe), "./cmd/oyabaun-relay"],
            cwd=ROOT / "relay",
            check=True,
        )
        print(f"go relay built -> {exe}")


def cmd_launch(ns: argparse.Namespace) -> None:
    if ns.docker:
        _launch_docker(ns)
        return
    st = _load_state()
    if st and st.get("mode") == "local":
        if st.get("relay_pid") and _pid_alive(int(st["relay_pid"])):
            sys.stderr.write("relay already running (oyabaunctl stop first, or use --force)\n")
            if not ns.force:
                sys.exit(1)
        if st.get("http_pid") and _pid_alive(int(st["http_pid"])):
            sys.stderr.write("web server already running (oyabaunctl stop first, or use --force)\n")
            if not ns.force:
                sys.exit(1)
    if ns.force:
        cmd_stop(argparse.Namespace())

    if ns.build:
        cmd_rebuild(argparse.Namespace(wasm=not ns.relay_only, relay=not ns.web_only))

    relay_proc = None
    http_proc = None

    if not ns.web_only:
        STATEDIR.mkdir(parents=True, exist_ok=True)
        relay_exe = _ensure_relay_binary()
        env = os.environ.copy()
        env["OYABAUN_RELAY_ADDR"] = f":{ns.relay_port}"
        env["OYABAUN_GAME_LOG"] = str(STATEDIR / "gameplay.jsonl")
        rlog = open(RELAY_LOG, "a", encoding="utf-8")
        rlog.write(f"\n--- launch {time.strftime('%Y-%m-%d %H:%M:%S')} ---\n")
        rlog.flush()
        relay_proc = subprocess.Popen(
            [str(relay_exe)],
            cwd=ROOT / "relay",
            env=env,
            stdout=rlog,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )

    if not ns.relay_only:
        STATEDIR.mkdir(parents=True, exist_ok=True)
        hlog = open(HTTP_LOG, "a", encoding="utf-8")
        hlog.write(f"\n--- launch {time.strftime('%Y-%m-%d %H:%M:%S')} ---\n")
        hlog.flush()
        http_proc = subprocess.Popen(
            [
                sys.executable,
                "-m",
                "http.server",
                str(ns.port),
                "--bind",
                ns.bind,
            ],
            cwd=ROOT / "client",
            stdout=hlog,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )

    if ns.relay_only and relay_proc is not None:
        _save_state(
            {
                "mode": "local",
                "relay_pid": relay_proc.pid,
                "relay_port": ns.relay_port,
            }
        )
        glog = STATEDIR / "gameplay.jsonl"
        print(f"relay pid {relay_proc.pid}  {_relay_binary()}  ws://127.0.0.1:{ns.relay_port}/ws  log {RELAY_LOG}")
        print(f"  gameplay JSONL -> {glog}")
        print("stop: tools/oyabaunctl.py stop")
        return
    if ns.web_only and http_proc is not None:
        _save_state(
            {
                "mode": "local",
                "http_pid": http_proc.pid,
                "http_port": ns.port,
            }
        )
        print(f"web pid {http_proc.pid}  http://{ns.bind}:{ns.port}/  log {HTTP_LOG}")
        print("stop: tools/oyabaunctl.py stop")
        return

    assert relay_proc is not None and http_proc is not None
    _save_state(
        {
            "mode": "local",
            "relay_pid": relay_proc.pid,
            "http_pid": http_proc.pid,
            "http_port": ns.port,
            "relay_port": ns.relay_port,
        }
    )
    glog = STATEDIR / "gameplay.jsonl"
    print(f"relay pid {relay_proc.pid}  {_relay_binary()}  ws://127.0.0.1:{ns.relay_port}/ws  log {RELAY_LOG}")
    print(f"  gameplay JSONL -> {glog}")
    print(f"web   pid {http_proc.pid}  http://{ns.bind}:{ns.port}/  log {HTTP_LOG}")
    print("stop: tools/oyabaunctl.py stop")


def _launch_docker(ns: argparse.Namespace) -> None:
    cf = ROOT / "infra" / "docker-compose.yml"
    if ns.build:
        subprocess.run(
            ["docker", "compose", "-f", str(cf), "build"],
            cwd=ROOT,
            check=True,
        )
    subprocess.run(
        ["docker", "compose", "-f", str(cf), "up", "-d"],
        cwd=ROOT,
        check=True,
    )
    _save_state(
        {
            "mode": "docker",
            "compose_file": "infra/docker-compose.yml",
        }
    )
    print("relay (docker) up — ws://127.0.0.1:8765/ws")
    print("stop: tools/oyabaunctl.py stop")
    if not ns.relay_only:
        sys.stderr.write("note: --docker starts relay only; run web with launch --web-only or another server\n")


def main() -> None:
    p = argparse.ArgumentParser(prog="oyabaunctl", description="Control Oyabaun dev processes")
    sub = p.add_subparsers(dest="cmd", required=True)

    sp = sub.add_parser("status", help="show tracked processes")
    sp.set_defaults(func=cmd_status)

    sp = sub.add_parser("stop", help="stop relay + web (or docker compose)")
    sp.set_defaults(func=cmd_stop)

    sp = sub.add_parser("rebuild", help="wasm-pack client and/or go build relay binary")
    sp.add_argument("--wasm-only", action="store_true", dest="wasm", help="only wasm-pack")
    sp.add_argument("--relay-only", action="store_true", dest="relay", help="only go build")
    sp.set_defaults(func=cmd_rebuild, wasm=False, relay=False)

    sp = sub.add_parser("launch", help="start relay binary + static client (http.server)")
    sp.add_argument("--docker", action="store_true", help="docker compose relay instead of local binary")
    sp.add_argument("--build", action="store_true", help="rebuild before start (wasm + go, or docker build)")
    sp.add_argument("--force", action="store_true", help="stop tracked processes then start")
    sp.add_argument("--relay-only", action="store_true", help="only start Go relay")
    sp.add_argument("--web-only", action="store_true", help="only start HTTP server for client/")
    sp.add_argument("--port", type=int, default=8080, help="HTTP port (default 8080)")
    sp.add_argument("--bind", default="127.0.0.1", help="HTTP bind address")
    sp.add_argument("--relay-port", type=int, default=8765, help="relay listen port (host, local mode)")
    sp.set_defaults(func=cmd_launch)
    args = p.parse_args()

    if args.cmd == "rebuild" and not args.wasm and not args.relay:
        args.wasm = True
        args.relay = True

    args.func(args)


if __name__ == "__main__":
    main()
