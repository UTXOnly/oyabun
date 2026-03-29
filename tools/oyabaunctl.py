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


def _blender_exe(ns: argparse.Namespace | None = None) -> str:
    if ns is not None and getattr(ns, "blender", None):
        return ns.blender
    return os.environ.get("BLENDER", "blender")


def _resolve_blender_executable(ns: argparse.Namespace | None) -> str:
    import shutil

    exe = _blender_exe(ns)
    ep = Path(exe)
    if ep.is_file():
        return str(ep.resolve())
    w = shutil.which(exe)
    if not w and sys.platform == "darwin":
        mac_blender = Path("/Applications/Blender.app/Contents/MacOS/Blender")
        if mac_blender.is_file():
            w = str(mac_blender)
    if not w:
        sys.stderr.write(
            f"oyabaunctl: Blender not found ({exe!r}). "
            "Install Blender, put it on PATH, set BLENDER, or pass --blender /path/to/blender\n"
        )
        sys.exit(1)
    return w


def _default_tokyo_blend() -> Path:
    return (ROOT / "client" / "levels" / "tokyo_alley.blend").resolve()


def cmd_redesign_tokyo_phase1(ns: argparse.Namespace) -> None:
    """Run tools/blender_redesign_tokyo_alley_phase1.py (shop recess + awnings + blade signs)."""
    blend = Path(ns.blend).expanduser().resolve() if ns.blend else _default_tokyo_blend()
    if not blend.is_file():
        sys.stderr.write(f"redesign-tokyo-phase1: blend not found: {blend}\n")
        sys.exit(1)
    script = ROOT / "tools" / "blender_redesign_tokyo_alley_phase1.py"
    if not script.is_file():
        sys.stderr.write(f"redesign-tokyo-phase1: missing {script}\n")
        sys.exit(1)
    exe = _resolve_blender_executable(ns)
    print(f"redesign-tokyo-phase1: {blend}", flush=True)
    subprocess.run(
        [exe, str(blend), "--background", "--python", str(script)],
        cwd=ROOT,
        check=True,
    )
    print("redesign-tokyo-phase1: done (run export-world --force-all to repack + export)", flush=True)
    if ns.export_after:
        cmd_export_world(
            argparse.Namespace(
                blend=str(blend),
                enhance=True,
                repack=True,
                force_all=False,
                fmt="both",
                blender=ns.blender,
                output_glb=None,
                output_json=None,
            )
        )


def cmd_enhance_tokyo_alley(ns: argparse.Namespace) -> None:
    """Run tools/blender_enhance_tokyo_alley.py (packed glTF-ready albedos, strip OyabaunTokyoDetail)."""
    blend = Path(ns.blend).expanduser().resolve() if ns.blend else _default_tokyo_blend()
    if not blend.is_file():
        sys.stderr.write(f"enhance-tokyo-alley: blend not found: {blend}\n")
        sys.exit(1)
    script = ROOT / "tools" / "blender_enhance_tokyo_alley.py"
    if not script.is_file():
        sys.stderr.write(f"enhance-tokyo-alley: missing {script}\n")
        sys.exit(1)
    exe = _resolve_blender_executable(ns)
    env = os.environ.copy()
    if ns.repack:
        env["OYABAUN_REPACK_ALBEDOS"] = "1"
    print(f"enhance-tokyo-alley: {blend}", flush=True)
    subprocess.run(
        [exe, str(blend), "--background", "--python", str(script)],
        cwd=ROOT,
        env=env,
        check=True,
    )
    print("enhance-tokyo-alley: done", flush=True)


def cmd_import_glb(ns: argparse.Namespace) -> None:
    """Copy a hand-exported .glb into client/levels/tokyo_alley.glb (optional wasm-pack)."""
    import shutil

    src = Path(ns.glb).expanduser().resolve()
    if not src.is_file():
        sys.stderr.write(f"import-glb: file not found: {src}\n")
        sys.exit(1)
    if src.suffix.lower() != ".glb":
        sys.stderr.write("import-glb: expected a .glb file\n")
        sys.exit(1)
    dest = ROOT / "client" / "levels" / "tokyo_alley.glb"
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dest)
    print(f"import-glb: {src} -> {dest}")
    if ns.rebuild:
        subprocess.run(
            ["wasm-pack", "build", "--target", "web", "--out-dir", "pkg"],
            cwd=ROOT / "client",
            check=True,
        )
        print("wasm client rebuilt -> client/pkg/")
    else:
        print("import-glb: run: python3 tools/oyabaunctl.py rebuild --wasm-only")


def cmd_export_world(ns: argparse.Namespace) -> None:
    """Run Blender headless to write client/levels/tokyo_alley.glb and/or tokyo_street.json."""
    if ns.force_all:
        ns.enhance = True
        ns.repack = True
        print("export-world: --force-all → enhance + repack + export", flush=True)

    blend = Path(ns.blend).expanduser().resolve() if ns.blend else _default_tokyo_blend()
    if not blend.is_file():
        sys.stderr.write(f"export-world: blend file not found: {blend}\n")
        sys.exit(1)

    if ns.enhance:
        cmd_enhance_tokyo_alley(
            argparse.Namespace(blend=str(blend), repack=ns.repack, blender=ns.blender)
        )

    exe = _resolve_blender_executable(ns)

    levels = ROOT / "client" / "levels"
    levels.mkdir(parents=True, exist_ok=True)
    out_glb = Path(ns.output_glb).expanduser().resolve() if ns.output_glb else levels / "tokyo_alley.glb"
    out_json = Path(ns.output_json).expanduser().resolve() if ns.output_json else levels / "tokyo_street.json"

    glb_script = ROOT / "tools" / "blender_export_gltf_oyabaun.py"
    json_script = ROOT / "tools" / "blender_export_oyabaun.py"
    if ns.fmt in ("glb", "both") and not glb_script.is_file():
        sys.stderr.write(f"export-world: missing {glb_script}\n")
        sys.exit(1)
    if ns.fmt in ("json", "both") and not json_script.is_file():
        sys.stderr.write(f"export-world: missing {json_script}\n")
        sys.exit(1)

    base_cmd = [exe, str(blend), "--background"]

    if ns.fmt in ("glb", "both"):
        env = os.environ.copy()
        env["OYABAUN_GLB_OUT"] = str(out_glb)
        print(f"export-world: glTF -> {out_glb}", flush=True)
        subprocess.run([*base_cmd, "--python", str(glb_script)], cwd=ROOT, env=env, check=True)

    if ns.fmt in ("json", "both"):
        env = os.environ.copy()
        env["OYABAUN_OUT"] = str(out_json)
        print(f"export-world: JSON  -> {out_json}", flush=True)
        subprocess.run([*base_cmd, "--python", str(json_script)], cwd=ROOT, env=env, check=True)

    print("export-world: done (serve from client/; see docs/BLENDER_GLTF.md)", flush=True)


def cmd_rebuild_level(ns: argparse.Namespace) -> None:
    """Full Tokyo level pipeline: repack all albedos, export GLB + JSON, optional wasm-pack."""
    print("rebuild-level: full refresh (repack → glTF → JSON" + (" → wasm" if ns.wasm else "") + ")", flush=True)
    cmd_export_world(
        argparse.Namespace(
            blend=ns.blend,
            enhance=True,
            repack=True,
            force_all=False,
            fmt=ns.fmt,
            blender=ns.blender,
            output_glb=ns.output_glb,
            output_json=ns.output_json,
        )
    )
    if ns.wasm:
        cmd_rebuild(argparse.Namespace(wasm=True, relay=False))
    print("rebuild-level: done", flush=True)


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

    sp = sub.add_parser(
        "export-world",
        help="export a .blend to client/levels (glTF .glb for WASM, optional vertex JSON fallback)",
    )
    sp.add_argument(
        "--blend",
        default=None,
        help="path to Blender .blend (default: client/levels/tokyo_alley.blend)",
    )
    sp.add_argument(
        "--enhance",
        action="store_true",
        help="run enhance-tokyo-alley first (packed albedos for glTF; use with --repack to rebuild all)",
    )
    sp.add_argument(
        "--repack",
        action="store_true",
        help="with --enhance: set OYABAUN_REPACK_ALBEDOS (rebuild every OyabaunPx_ texture)",
    )
    sp.add_argument(
        "--force-all",
        action="store_true",
        dest="force_all",
        help="shorthand for --enhance --repack (full albedo rebuild then GLB/JSON export)",
    )
    sp.add_argument(
        "--format",
        dest="fmt",
        choices=("glb", "json", "both"),
        default="both",
        help="glb= textured level only, json= legacy vertex export, both= run both (default)",
    )
    sp.add_argument(
        "--blender",
        default=None,
        help="Blender executable (default: $BLENDER env or 'blender' on PATH)",
    )
    sp.add_argument(
        "--output-glb",
        default=None,
        help="output .glb path (default: <repo>/client/levels/tokyo_alley.glb)",
    )
    sp.add_argument(
        "--output-json",
        default=None,
        help="output JSON path (default: <repo>/client/levels/tokyo_street.json)",
    )
    sp.set_defaults(func=cmd_export_world, enhance=False, repack=False, force_all=False)

    sp = sub.add_parser(
        "rebuild-level",
        help="Tokyo alley: repack every packed albedo, export .glb + legacy JSON (optional wasm-pack)",
    )
    sp.add_argument(
        "--blend",
        default=None,
        help="path to Blender .blend (default: client/levels/tokyo_alley.blend)",
    )
    sp.add_argument(
        "--wasm",
        action="store_true",
        help="run wasm-pack after export (refresh include_bytes! embedded GLB in the bundle)",
    )
    sp.add_argument(
        "--format",
        dest="fmt",
        choices=("glb", "json", "both"),
        default="both",
        help="same as export-world (default: both)",
    )
    sp.add_argument("--blender", default=None, help="Blender executable (default: $BLENDER or PATH)")
    sp.add_argument("--output-glb", default=None, help="output .glb path (default: client/levels/tokyo_alley.glb)")
    sp.add_argument("--output-json", default=None, help="output JSON path (default: client/levels/tokyo_street.json)")
    sp.set_defaults(func=cmd_rebuild_level, wasm=False)

    sp = sub.add_parser(
        "enhance-tokyo-alley",
        help="pack pixel albedos in Tokyo alley .blend (glTF-safe textures; removes OyabaunTokyoDetail if present)",
    )
    sp.add_argument(
        "--blend",
        default=None,
        help="path to .blend (default: client/levels/tokyo_alley.blend)",
    )
    sp.add_argument(
        "--repack",
        action="store_true",
        help="rebuild all packed materials (OYABAUN_REPACK_ALBEDOS)",
    )
    sp.add_argument(
        "--blender",
        default=None,
        help="Blender executable (default: $BLENDER env or 'blender' on PATH)",
    )
    sp.set_defaults(func=cmd_enhance_tokyo_alley, repack=False)

    sp = sub.add_parser(
        "redesign-tokyo-phase1",
        help="Tokyo alley CURSOR_LEVEL_REDESIGN phase 1: add shop recesses, awnings, blade signs (see tools/blender_redesign_tokyo_alley_phase1.py)",
    )
    sp.add_argument("--blend", default=None, help="path to .blend (default: client/levels/tokyo_alley.blend)")
    sp.add_argument("--blender", default=None, help="Blender executable (default: $BLENDER or PATH)")
    sp.add_argument(
        "--export-after",
        action="store_true",
        dest="export_after",
        help="run export-world with enhance+repack after (same as rebuild-level content-wise)",
    )
    sp.set_defaults(func=cmd_redesign_tokyo_phase1, export_after=False)

    sp = sub.add_parser(
        "import-glb",
        help="copy an existing .glb to client/levels/tokyo_alley.glb (then rebuild wasm to refresh embed)",
    )
    sp.add_argument("glb", help="path to source .glb (e.g. ~/Desktop/oyabaun-av/oyabaun-level-1.glb)")
    sp.add_argument(
        "--rebuild",
        action="store_true",
        help="run wasm-pack after copy (needed for include_bytes! embedded level)",
    )
    sp.set_defaults(func=cmd_import_glb)

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
