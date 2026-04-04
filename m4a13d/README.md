# M4A1 source mesh

Place your exported **`base.obj`** here (Blender **File → Export → Wavefront .obj**). The file is **gitignored** because it is typically very large (~500k+ faces).

Regenerate game assets:

```bash
python3 tools/oyabaunctl.py export-m4a1-assets
```

This writes **`client/fpsweapons/m4a1.png`** and **`client/props/m4a1_prop.glb`** (decimated).
