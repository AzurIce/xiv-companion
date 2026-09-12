import bpy, json, sys
from mathutils import Vector

src, out = sys.argv[-2:]
with open(src) as f: data=json.load(f)
bpy.ops.wm.read_factory_settings(use_empty=True)
for mi, m in enumerate(data['meshes']):
    vs=[v['position'] for v in m['vertices']]
    fs=[m['indices'][i:i+3] for i in range(0,len(m['indices']),3)]
    me=bpy.data.meshes.new('FFXIV_'+str(mi)); me.from_pydata(vs,[],fs); me.update()
    ob=bpy.data.objects.new(m['path'],me); bpy.context.collection.objects.link(ob)
    mat=bpy.data.materials.get('FFXIV_'+str(m['materialIndex'])) or bpy.data.materials.new('FFXIV_'+str(m['materialIndex']))
    mat.diffuse_color=(*m.get('color',[0.7,0.7,0.7]),1.0); ob.data.materials.append(mat)
    me.normals_split_custom_set_from_vertices([v['normal'] for v in m['vertices']])
    uv=me.uv_layers.new(name='UVMap')
    for poly in me.polygons:
        for li in poly.loop_indices: uv.data[li].uv=m['vertices'][me.loops[li].vertex_index]['uv0']
bpy.ops.object.select_all(action='SELECT')
bpy.context.view_layer.objects.active=None
for ob in bpy.context.scene.objects:
    if ob.type=='MESH': ob.rotation_euler[0]=1.5708
bpy.ops.object.camera_add(location=(2.8,3.2,1.3)); cam=bpy.context.object; bpy.context.scene.camera=cam
bpy.ops.object.empty_add(location=(0,0.03,0.85)); target=bpy.context.object
c=cam.constraints.new(type='TRACK_TO'); c.target=target; c.track_axis='TRACK_NEGATIVE_Z'; c.up_axis='UP_Y'
bpy.ops.object.light_add(type='AREA', location=(1,2,3)); bpy.context.object.data.energy=1200; bpy.context.object.data.shape='DISK'; bpy.context.object.data.size=4
bpy.context.scene.render.engine='BLENDER_EEVEE'; bpy.context.scene.render.resolution_x=700; bpy.context.scene.render.resolution_y=700; bpy.context.scene.render.resolution_percentage=100
bpy.context.scene.render.filepath=out.replace('.blend','.png'); bpy.ops.render.render(write_still=True)
bpy.ops.wm.save_as_mainfile(filepath=out)
bpy.ops.object.select_all(action='SELECT')
bpy.ops.export_scene.gltf(filepath=out.replace('.blend','.glb'), export_format='GLB', use_selection=True, export_apply=True)
