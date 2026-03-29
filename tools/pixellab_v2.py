#!/usr/bin/env python3
"""
PixelLab HTTP API v2 — same Bearer token as MCP (.cursor/mcp.json).

Use when the IDE MCP client sends broken JSON for string tool args (e.g. animate_character).

Examples:
  python3 tools/pixellab_v2.py balance
  python3 tools/pixellab_v2.py list --limit 20
  python3 tools/pixellab_v2.py get d5ceb30a-0a4b-49c4-8ccb-988898cb8135
  python3 tools/pixellab_v2.py animate dabe33dd-b9d5-481c-9413-402cd0002747 walking
  python3 tools/pixellab_v2.py create8 "yakuza with pistol" --size 112
  python3 tools/pixellab_v2.py zip dabe33dd-b9d5-481c-9413-402cd0002747 ./rival.zip

Env: PIXELLAB_API_TOKEN (or PIXELLAB_MCP_TOKEN). If unset, reads Bearer from repo .cursor/mcp.json.

PixelLab allows **8 concurrent background jobs** (Tier 1). A full 8-direction walk uses all 8 — wait for
them to finish before `create8` or another animate.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

BASE = "https://api.pixellab.ai/v2"


def _repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def load_token() -> str:
    t = os.environ.get("PIXELLAB_API_TOKEN") or os.environ.get("PIXELLAB_MCP_TOKEN")
    if t:
        return t.strip()
    mcp = _repo_root() / ".cursor" / "mcp.json"
    if mcp.is_file():
        cfg = json.loads(mcp.read_text(encoding="utf-8"))
        auth = (
            cfg.get("mcpServers", {})
            .get("pixellab", {})
            .get("headers", {})
            .get("Authorization", "")
        )
        if isinstance(auth, str) and auth.startswith("Bearer "):
            return auth[7:].strip()
    print(
        "Missing token: set PIXELLAB_API_TOKEN or add Authorization Bearer to .cursor/mcp.json",
        file=sys.stderr,
    )
    sys.exit(1)


def request_json(method: str, path: str, body: dict | None = None) -> dict:
    url = BASE + path
    data = json.dumps(body).encode("utf-8") if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Authorization", f"Bearer {load_token()}")
    if body is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        err_body = e.read().decode("utf-8", errors="replace")
        try:
            parsed = json.loads(err_body)
        except json.JSONDecodeError:
            print(f"HTTP {e.code}: {err_body[:2000]}", file=sys.stderr)
        else:
            print(json.dumps(parsed, indent=2), file=sys.stderr)
            if e.code == 429 or (
                isinstance(parsed, dict)
                and "concurrent" in json.dumps(parsed).lower()
            ):
                print(
                    "\nTip: Tier 1 allows 8 concurrent jobs. Wait for running animations to finish.",
                    file=sys.stderr,
                )
        sys.exit(1)


def cmd_balance(_: argparse.Namespace) -> None:
    r = request_json("GET", "/balance")
    print(json.dumps(r, indent=2))


def cmd_list(args: argparse.Namespace) -> None:
    r = request_json("GET", f"/characters?limit={args.limit}&offset={args.offset}")
    print(json.dumps(r, indent=2))


def cmd_get(args: argparse.Namespace) -> None:
    r = request_json("GET", f"/characters/{args.character_id}")
    print(json.dumps(r, indent=2))


def cmd_animate(args: argparse.Namespace) -> None:
    body = {
        "character_id": args.character_id,
        "template_animation_id": args.template,
    }
    if args.name:
        body["animation_name"] = args.name
    r = request_json("POST", "/animate-character", body)
    print(json.dumps(r, indent=2))


def cmd_zip(args: argparse.Namespace) -> None:
    out = Path(args.out).resolve()
    out.parent.mkdir(parents=True, exist_ok=True)
    url = f"{BASE}/characters/{args.character_id}/zip"
    req = urllib.request.Request(url, method="GET")
    req.add_header("Authorization", f"Bearer {load_token()}")
    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            data = resp.read()
    except urllib.error.HTTPError as e:
        print(e.read().decode("utf-8", errors="replace")[:2000], file=sys.stderr)
        sys.exit(1)
    out.write_bytes(data)
    print(f"Wrote {len(data)} bytes -> {out}")


def cmd_create8(args: argparse.Namespace) -> None:
    body = {
        "description": args.description,
        "image_size": {"width": args.size, "height": args.size},
        "view": args.view,
    }
    r = request_json("POST", "/create-character-with-8-directions", body)
    print(json.dumps(r, indent=2))


def cmd_create4(args: argparse.Namespace) -> None:
    """When create8 fails with bone_scaling, 4-dir creation often still works."""
    body = {
        "description": args.description,
        "image_size": {"width": args.size, "height": args.size},
        "view": args.view,
    }
    r = request_json("POST", "/create-character-with-4-directions", body)
    print(json.dumps(r, indent=2))


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest="cmd", required=True)

    sub.add_parser("balance", help="GET /balance").set_defaults(func=cmd_balance)

    lp = sub.add_parser("list", help="GET /characters")
    lp.add_argument("--limit", type=int, default=20)
    lp.add_argument("--offset", type=int, default=0)
    lp.set_defaults(func=cmd_list)

    gp = sub.add_parser("get", help="GET /characters/{id}")
    gp.add_argument("character_id")
    gp.set_defaults(func=cmd_get)

    ap = sub.add_parser("animate", help="POST /animate-character (template walk, etc.)")
    ap.add_argument("character_id")
    ap.add_argument(
        "template",
        help="e.g. walking, walk, running-6-frames, breathing-idle",
    )
    ap.add_argument("--name", help="optional animation_name")
    ap.set_defaults(func=cmd_animate)

    cp = sub.add_parser("create8", help="POST /create-character-with-8-directions")
    cp.add_argument("description")
    cp.add_argument("--size", type=int, default=112)
    cp.add_argument("--view", default="low top-down")
    cp.set_defaults(func=cmd_create8)

    c4 = sub.add_parser(
        "create4",
        help="POST /create-character-with-4-directions (workaround if create8 fails)",
    )
    c4.add_argument("description")
    c4.add_argument("--size", type=int, default=112)
    c4.add_argument("--view", default="low top-down")
    c4.set_defaults(func=cmd_create4)

    zp = sub.add_parser("zip", help="GET /characters/{id}/zip")
    zp.add_argument("character_id")
    zp.add_argument("out", type=Path, help="Output .zip path")
    zp.set_defaults(func=cmd_zip)

    args = p.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
