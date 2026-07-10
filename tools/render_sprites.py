# Render animation clips from a .glb to transparent sprite strips.
#
#   blender -b -P tools/render_sprites.py -- <model.glb> <out_dir> [Clip=frames ...]
#
# Example:
#   blender -b -P tools/render_sprites.py -- assets/models/runner.glb \
#       crates/game/assets/sprites Run=12 Idle=8 Jump=10
#
# Each clip becomes <out_dir>/<clip>.png, a horizontal strip of square frames,
# plus <out_dir>/<clip>.json with frame metadata.

import bpy
import json
import math
import os
import sys

import numpy as np

FRAME_PX = 256

argv = sys.argv[sys.argv.index("--") + 1:]
glb_path, out_dir = argv[0], argv[1]
clips = []
for spec in argv[2:]:
    name, _, frames = spec.partition("=")
    clips.append((name, int(frames or "12")))

os.makedirs(out_dir, exist_ok=True)

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=glb_path)
scene = bpy.context.scene

from mathutils import Matrix, Vector

armature = next(o for o in scene.objects if o.type == "ARMATURE")
# Face the character to the right of a side-view camera placed on +X.
# Pre-multiply so the importer's Y-up -> Z-up rotation is preserved.
FACING_DEG = float(os.environ.get("FACING_DEG", "180"))
armature.matrix_world = (
    Matrix.Rotation(math.radians(FACING_DEG), 4, "Z") @ armature.matrix_world
)

# Fit an orthographic camera to the rest-pose bounding box of all meshes.
lo = [1e9] * 3
hi = [-1e9] * 3
for obj in scene.objects:
    if obj.type != "MESH":
        continue
    for corner in obj.bound_box:
        world = obj.matrix_world @ Vector(corner)
        for i in range(3):
            lo[i] = min(lo[i], world[i])
            hi[i] = max(hi[i], world[i])
height = hi[2] - lo[2]

cam_data = bpy.data.cameras.new("cam")
cam_data.type = "ORTHO"
# Leave headroom for jump/limb extremes beyond the rest pose.
view_h = height * 1.6
cam_data.ortho_scale = view_h
# Pin the ground plane (z=0) at GROUND_FRAC of the frame from the bottom, so
# the game knows exactly where the feet touch the track.
GROUND_FRAC = 0.1
cam = bpy.data.objects.new("cam", cam_data)
scene.collection.objects.link(cam)
cam.location = (8, (lo[1] + hi[1]) / 2, view_h / 2 - GROUND_FRAC * view_h)
cam.rotation_euler = (math.radians(90), 0, math.radians(90))
scene.camera = cam

sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
sun.data.energy = 3.0
scene.collection.objects.link(sun)
sun.rotation_euler = (math.radians(50), math.radians(-20), math.radians(40))
fill = bpy.data.objects.new("fill", bpy.data.lights.new("fill", "SUN"))
fill.data.energy = 1.2
scene.collection.objects.link(fill)
fill.rotation_euler = (math.radians(60), math.radians(20), math.radians(220))

scene.render.engine = (
    "BLENDER_EEVEE_NEXT"
    if "BLENDER_EEVEE_NEXT" in
    bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items
    else "BLENDER_EEVEE"
)
scene.render.film_transparent = True
scene.render.resolution_x = scene.render.resolution_y = FRAME_PX
scene.render.image_settings.file_format = "PNG"
scene.render.image_settings.color_mode = "RGBA"

armature.animation_data_create()

for clip_name, frame_count in clips:
    action = bpy.data.actions[f"CharacterArmature|{clip_name}"]
    armature.animation_data.action = action
    f0, f1 = action.frame_range
    span = max(f1 - f0, 1)

    strip = np.zeros((FRAME_PX, FRAME_PX * frame_count, 4), dtype=np.float32)
    tmp = os.path.join(out_dir, "_frame.png")
    for i in range(frame_count):
        # Sample the clip evenly; for loops, stop short of the last (=first) frame.
        frame = f0 + span * i / frame_count
        scene.frame_set(int(frame), subframe=frame - int(frame))
        scene.render.filepath = tmp
        bpy.ops.render.render(write_still=True)
        img = bpy.data.images.load(tmp)
        px = np.array(img.pixels[:], dtype=np.float32).reshape(FRAME_PX, FRAME_PX, 4)
        strip[:, i * FRAME_PX:(i + 1) * FRAME_PX, :] = px
        bpy.data.images.remove(img)
    os.remove(tmp)

    out_img = bpy.data.images.new(clip_name, FRAME_PX * frame_count, FRAME_PX, alpha=True)
    out_img.pixels[:] = strip.ravel()
    out_path = os.path.join(out_dir, f"{clip_name.lower()}.png")
    out_img.filepath_raw = out_path
    out_img.file_format = "PNG"
    out_img.save()
    with open(os.path.join(out_dir, f"{clip_name.lower()}.json"), "w") as fh:
        json.dump(
            {"frame_px": FRAME_PX, "frames": frame_count, "ground_frac": GROUND_FRAC},
            fh,
        )
    print(f"wrote {out_path} ({frame_count} frames)")
