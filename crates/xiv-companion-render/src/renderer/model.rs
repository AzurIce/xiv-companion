use wgpu::util::DeviceExt;

use crate::{MaterialAlphaMode, MaterialRenderMode, ModelMaterial, ModelRenderData};

const POST_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEFAULT_BLOOM_STRENGTH: f32 = 0.68;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelRenderOptions {
    pub normal_mapping: bool,
    pub normal_y_sign: f32,
    pub bloom: bool,
    pub bloom_strength: f32,
}

impl Default for ModelRenderOptions {
    fn default() -> Self {
        Self {
            normal_mapping: true,
            normal_y_sign: -1.0,
            bloom: true,
            bloom_strength: DEFAULT_BLOOM_STRENGTH,
        }
    }
}

impl ModelRenderOptions {
    fn normalized(self) -> Self {
        Self {
            normal_mapping: self.normal_mapping,
            normal_y_sign: if self.normal_y_sign < 0.0 { -1.0 } else { 1.0 },
            bloom: self.bloom,
            bloom_strength: self.bloom_strength.clamp(0.0, 2.0),
        }
    }

    fn bloom_strength(self) -> f32 {
        let normalized = self.normalized();
        if normalized.bloom {
            normalized.bloom_strength
        } else {
            0.0
        }
    }
}

pub struct ModelRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    culled_pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,
    transparent_culled_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    compose_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    draw_batches: Vec<DrawBatch>,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    material_bind_groups: Vec<wgpu::BindGroup>,
    post_sampler: wgpu::Sampler,
    compose_uniform_buffer: wgpu::Buffer,
    blur_bind_group_layout: wgpu::BindGroupLayout,
    compose_bind_group_layout: wgpu::BindGroupLayout,
    post_process: Option<PostProcessState>,
    format: wgpu::TextureFormat,
    bounds_center: [f32; 3],
    bounds_radius: f32,
}

impl ModelRenderer {
    pub fn new<M: ModelRenderData + ?Sized>(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        model: &M,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("model shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("model.wgsl").into()),
        });
        let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("model postprocess shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("postprocess.wgsl").into()),
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("weapon camera uniform"),
            size: std::mem::size_of::<CameraUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("weapon camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("weapon camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("weapon material bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("weapon pipeline layout"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&material_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let blur_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("weapon-bloom-pass bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let compose_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("weapon compose bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let blur_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("weapon-bloom-pass pipeline layout"),
            bind_group_layouts: &[Some(&blur_bind_group_layout)],
            immediate_size: 0,
        });
        let compose_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("weapon compose pipeline layout"),
                bind_group_layouts: &[Some(&compose_bind_group_layout)],
                immediate_size: 0,
            });

        let pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon model pipeline",
            false,
            false,
        );
        let culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon culled model pipeline",
            false,
            true,
        );
        let transparent_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon transparent model pipeline",
            true,
            false,
        );
        let transparent_culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon transparent culled model pipeline",
            true,
            true,
        );

        let blur_pipeline = create_post_pipeline(
            &device,
            &post_shader,
            &blur_pipeline_layout,
            "weapon-bloom-pass pipeline",
            "blur_fs",
            POST_FORMAT,
        );
        let compose_pipeline = create_post_pipeline(
            &device,
            &post_shader,
            &compose_pipeline_layout,
            "weapon compose pipeline",
            "compose_fs",
            format,
        );

        let (vertices, indices, draw_batches) = flatten_model(model);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("weapon vertex buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("weapon index buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let material_bind_groups =
            create_material_bind_groups(&device, &queue, &material_bind_group_layout, model);
        let post_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("weapon postprocess sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let compose_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("weapon compose uniform"),
            contents: bytemuck::bytes_of(&PostUniform {
                params: [DEFAULT_BLOOM_STRENGTH, 0.0, 0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            device,
            queue,
            pipeline,
            culled_pipeline,
            transparent_pipeline,
            transparent_culled_pipeline,
            blur_pipeline,
            compose_pipeline,
            vertex_buffer,
            index_buffer,
            draw_batches,
            camera_buffer,
            camera_bind_group,
            material_bind_groups,
            post_sampler,
            compose_uniform_buffer,
            blur_bind_group_layout,
            compose_bind_group_layout,
            post_process: None,
            format,
            bounds_center: model.bounds().center,
            bounds_radius: model.bounds().radius,
        }
    }

    pub fn render_to(
        &mut self,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        viewport: [u32; 2],
        yaw: f32,
        pitch: f32,
        zoom: f32,
        pan: [f32; 2],
        options: ModelRenderOptions,
    ) {
        let uniform = camera_uniform(
            self.bounds_center,
            self.bounds_radius,
            viewport,
            yaw,
            pitch,
            zoom,
            pan,
            options,
        );
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        self.queue.write_buffer(
            &self.compose_uniform_buffer,
            0,
            bytemuck::bytes_of(&PostUniform {
                params: [options.bloom_strength(), 0.0, 0.0, 0.0],
            }),
        );
        let viewport = [viewport[0].max(1), viewport[1].max(1)];
        self.ensure_post_process_targets(viewport);
        let post = self
            .post_process
            .as_ref()
            .expect("post process targets are initialized");

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("weapon render encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("weapon scene render pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &post.scene_view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.055,
                                g: 0.061,
                                b: 0.067,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &post.bright_view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            for batch in self.draw_batches.iter().filter(|batch| !batch.transparent) {
                render_pass.set_pipeline(if batch.render_backfaces {
                    &self.pipeline
                } else {
                    &self.culled_pipeline
                });
                draw_model_batch(&mut render_pass, &self.material_bind_groups, batch);
            }

            let sorted_transparent_batches =
                sorted_transparent_batches(&self.draw_batches, yaw, pitch);
            for batch in sorted_transparent_batches {
                render_pass.set_pipeline(if batch.render_backfaces {
                    &self.transparent_pipeline
                } else {
                    &self.transparent_culled_pipeline
                });
                draw_model_batch(&mut render_pass, &self.material_bind_groups, batch);
            }
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("weapon bloom horizontal pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &post.blur_a_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            render_pass.set_pipeline(&self.blur_pipeline);
            render_pass.set_bind_group(0, &post.blur_bright_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("weapon bloom vertical pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &post.blur_b_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            render_pass.set_pipeline(&self.blur_pipeline);
            render_pass.set_bind_group(0, &post.blur_a_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("weapon compose pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            render_pass.set_pipeline(&self.compose_pipeline);
            render_pass.set_bind_group(0, &post.compose_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    fn ensure_post_process_targets(&mut self, viewport: [u32; 2]) {
        let width = viewport[0].max(1);
        let height = viewport[1].max(1);
        let needs_recreate = self
            .post_process
            .as_ref()
            .map(|targets| targets.width != width || targets.height != height)
            .unwrap_or(true);

        if needs_recreate {
            self.post_process = Some(PostProcessState::new(
                &self.device,
                &self.blur_bind_group_layout,
                &self.compose_bind_group_layout,
                &self.post_sampler,
                &self.compose_uniform_buffer,
                width,
                height,
            ));
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}

struct PostProcessState {
    width: u32,
    height: u32,
    scene_view: wgpu::TextureView,
    bright_view: wgpu::TextureView,
    blur_a_view: wgpu::TextureView,
    blur_b_view: wgpu::TextureView,
    blur_bright_bind_group: wgpu::BindGroup,
    blur_a_bind_group: wgpu::BindGroup,
    compose_bind_group: wgpu::BindGroup,
}

impl PostProcessState {
    fn new(
        device: &wgpu::Device,
        blur_layout: &wgpu::BindGroupLayout,
        compose_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        compose_uniform_buffer: &wgpu::Buffer,
        width: u32,
        height: u32,
    ) -> Self {
        let scene_view = create_post_texture_view(device, "weapon scene texture", width, height);
        let bright_view = create_post_texture_view(device, "weapon bright texture", width, height);
        let blur_a_view = create_post_texture_view(device, "weapon-bloom-pass a", width, height);
        let blur_b_view = create_post_texture_view(device, "weapon-bloom-pass b", width, height);

        let blur_horizontal_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("weapon-bloom-pass horizontal uniform"),
            contents: bytemuck::bytes_of(&PostUniform {
                params: [1.0 / width as f32, 0.0, 0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let blur_vertical_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("weapon-bloom-pass vertical uniform"),
            contents: bytemuck::bytes_of(&PostUniform {
                params: [0.0, 1.0 / height as f32, 0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let blur_bright_bind_group = create_post_bind_group(
            device,
            blur_layout,
            &bright_view,
            sampler,
            &blur_horizontal_buffer,
            &bright_view,
            "weapon-bloom-pass bright bind group",
        );
        let blur_a_bind_group = create_post_bind_group(
            device,
            blur_layout,
            &blur_a_view,
            sampler,
            &blur_vertical_buffer,
            &blur_a_view,
            "weapon-bloom-pass a bind group",
        );
        let compose_bind_group = create_post_bind_group(
            device,
            compose_layout,
            &scene_view,
            sampler,
            compose_uniform_buffer,
            &blur_b_view,
            "weapon compose bind group",
        );

        Self {
            width,
            height,
            scene_view,
            bright_view,
            blur_a_view,
            blur_b_view,
            blur_bright_bind_group,
            blur_a_bind_group,
            compose_bind_group,
        }
    }
}

fn flatten_model<M: ModelRenderData + ?Sized>(
    model: &M,
) -> (Vec<GpuVertex>, Vec<u32>, Vec<DrawBatch>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut draw_batches = Vec::new();

    for mesh in model.meshes() {
        let base = vertices.len() as u32;
        vertices.extend(mesh.vertices.iter().map(|vertex| GpuVertex {
            position: vertex.position,
            normal: vertex.normal,
            uv0: vertex.uv0,
            bitangent: vertex.bitangent,
            color: vertex.color,
        }));
        let index_start = indices.len() as u32;
        indices.extend(mesh.indices.iter().map(|index| base + *index));
        draw_batches.push(DrawBatch {
            material_slot: mesh.material_slot,
            index_start,
            index_count: mesh.indices.len() as u32,
            transparent: material_is_transparent(model, mesh.material_slot),
            render_backfaces: material_renders_backfaces(model, mesh.material_slot),
            center: mesh_bounds_center(mesh),
        });
    }

    (vertices, indices, draw_batches)
}

fn mesh_bounds_center(mesh: &crate::ModelMesh) -> [f32; 3] {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in &mesh.vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex.position[axis]);
            max[axis] = max[axis].max(vertex.position[axis]);
        }
    }

    if min.iter().any(|value| !value.is_finite()) || max.iter().any(|value| !value.is_finite()) {
        return [0.0; 3];
    }

    [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ]
}

fn material_is_transparent<M: ModelRenderData + ?Sized>(model: &M, material_slot: usize) -> bool {
    let Some(material) = model.materials().get(material_slot) else {
        return false;
    };

    match material.alpha_mode {
        MaterialAlphaMode::Blend | MaterialAlphaMode::Glass => true,
        MaterialAlphaMode::Mask => false,
        MaterialAlphaMode::Opaque => material.render_mode != MaterialRenderMode::Opaque,
    }
}

fn material_renders_backfaces<M: ModelRenderData + ?Sized>(
    model: &M,
    material_slot: usize,
) -> bool {
    model
        .materials()
        .get(material_slot)
        .map(|material| material.render_backfaces)
        .unwrap_or(true)
}

fn sorted_transparent_batches(draw_batches: &[DrawBatch], yaw: f32, pitch: f32) -> Vec<&DrawBatch> {
    let sort_dir = transparent_sort_direction(yaw, pitch);
    let mut batches = draw_batches
        .iter()
        .filter(|batch| batch.transparent)
        .collect::<Vec<_>>();
    batches.sort_by(|left, right| {
        let left_depth = glam::Vec3::from(left.center).dot(sort_dir);
        let right_depth = glam::Vec3::from(right.center).dot(sort_dir);
        right_depth
            .partial_cmp(&left_depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    batches
}

fn transparent_sort_direction(yaw: f32, pitch: f32) -> glam::Vec3 {
    let pitch = pitch.clamp(-1.35, 1.35);
    glam::Vec3::new(
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        -yaw.cos() * pitch.cos(),
    )
    .normalize_or_zero()
}

fn draw_model_batch<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    material_bind_groups: &'a [wgpu::BindGroup],
    batch: &DrawBatch,
) {
    if let Some(bind_group) = material_bind_groups
        .get(batch.material_slot)
        .or_else(|| material_bind_groups.first())
    {
        render_pass.set_bind_group(1, bind_group, &[]);
        render_pass.draw_indexed(
            batch.index_start..batch.index_start + batch.index_count,
            0,
            0..1,
        );
    }
}

fn create_model_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    label: &str,
    transparent: bool,
    cull_backfaces: bool,
) -> wgpu::RenderPipeline {
    let blend = transparent.then_some(wgpu::BlendState::ALPHA_BLENDING);
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[GpuVertex::layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[
                Some(wgpu::ColorTargetState {
                    format: POST_FORMAT,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
                Some(wgpu::ColorTargetState {
                    format: POST_FORMAT,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
            ],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: cull_backfaces.then_some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24Plus,
            depth_write_enabled: Some(!transparent),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_post_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    label: &str,
    fragment_entry: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment_entry),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_post_texture_view(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: POST_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn create_post_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform_buffer: &wgpu::Buffer,
    bloom_view: &wgpu::TextureView,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(bloom_view),
            },
        ],
    })
}

fn create_material_bind_groups<M: ModelRenderData + ?Sized>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    model: &M,
) -> Vec<wgpu::BindGroup> {
    if model.materials().is_empty() {
        return vec![create_material_bind_group(
            device,
            queue,
            layout,
            &fallback_material(),
            model,
        )];
    }

    model
        .materials()
        .iter()
        .map(|material| create_material_bind_group(device, queue, layout, material, model))
        .collect()
}

fn create_material_bind_group<M: ModelRenderData + ?Sized>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    material: &ModelMaterial,
    model: &M,
) -> wgpu::BindGroup {
    let uniform = MaterialUniform {
        diffuse_color: [
            material.diffuse_color[0],
            material.diffuse_color[1],
            material.diffuse_color[2],
            material.opacity.clamp(0.0, 1.0),
        ],
        emissive_color: [
            material.emissive_color[0],
            material.emissive_color[1],
            material.emissive_color[2],
            material
                .emissive_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
        ],
        specular_color: [
            material.specular_color[0],
            material.specular_color[1],
            material.specular_color[2],
            material.roughness,
        ],
        params: [
            material
                .base_color_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
            material.metalness,
            material
                .normal_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
            material
                .mask_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
        ],
        properties: [
            material
                .material_properties_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
            0.0,
            0.0,
            0.0,
        ],
        render: [
            render_mode_value(material.render_mode),
            material.opacity,
            alpha_mode_value(material.alpha_mode),
            material.alpha_threshold.clamp(0.0, 1.0),
        ],
    };
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("weapon material uniform"),
        contents: bytemuck::bytes_of(&uniform),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let texture_view = material
        .base_color_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8UnormSrgb,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon white texture",
                1,
                1,
                &[255, 255, 255, 255],
                wgpu::TextureFormat::Rgba8UnormSrgb,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mask_texture_view = material
        .mask_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon mask texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon neutral mask texture",
                1,
                1,
                &[255, 128, 0, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let emissive_texture_view = material
        .emissive_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon emissive texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8UnormSrgb,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon black emissive texture",
                1,
                1,
                &[0, 0, 0, 255],
                wgpu::TextureFormat::Rgba8UnormSrgb,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let normal_texture_view = material
        .normal_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon normal texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon flat normal texture",
                1,
                1,
                &[128, 128, 255, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let material_properties_texture_view = material
        .material_properties_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon material properties texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon neutral material properties texture",
                1,
                1,
                &[
                    unorm_byte(material.metalness),
                    unorm_byte(material.roughness),
                    255,
                    255,
                ],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("weapon material sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("weapon material bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&normal_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&mask_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(&emissive_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(&material_properties_texture_view),
            },
        ],
    })
}

fn create_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    let expected_len = width as usize * height as usize * 4;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    if rgba.len() >= expected_len {
        let copy_layout = if height == 1 {
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: None,
                rows_per_image: None,
            }
        } else {
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            }
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba[..expected_len],
            copy_layout,
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
    texture
}

fn unorm_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn fallback_material() -> ModelMaterial {
    ModelMaterial {
        slot: 0,
        material_index: 0,
        name: "fallback".to_string(),
        path: None,
        shader_package_name: None,
        render_mode: MaterialRenderMode::Opaque,
        alpha_mode: MaterialAlphaMode::Opaque,
        alpha_threshold: 0.0,
        opacity: 1.0,
        render_backfaces: true,
        fallback_color: [0.78, 0.72, 0.64],
        diffuse_color: [0.78, 0.72, 0.64],
        specular_color: [0.35, 0.35, 0.35],
        emissive_color: [0.0, 0.0, 0.0],
        roughness: 0.55,
        metalness: 0.0,
        texture_indices: Vec::new(),
        base_color_texture: None,
        normal_texture: None,
        mask_texture: None,
        specular_texture: None,
        emissive_texture: None,
        material_properties_texture: None,
    }
}

fn render_mode_value(mode: MaterialRenderMode) -> f32 {
    match mode {
        MaterialRenderMode::Opaque => 0.0,
        MaterialRenderMode::Transparent => 1.0,
        MaterialRenderMode::Glass => 2.0,
    }
}

fn alpha_mode_value(mode: MaterialAlphaMode) -> f32 {
    match mode {
        MaterialAlphaMode::Opaque => 0.0,
        MaterialAlphaMode::Mask => 1.0,
        MaterialAlphaMode::Blend => 2.0,
        MaterialAlphaMode::Glass => 3.0,
    }
}

fn camera_uniform(
    center: [f32; 3],
    radius: f32,
    viewport: [u32; 2],
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan: [f32; 2],
    options: ModelRenderOptions,
) -> CameraUniform {
    let options = options.normalized();
    let aspect = if viewport[1] == 0 {
        1.0
    } else {
        viewport[0] as f32 / viewport[1] as f32
    };
    let radius = radius.max(0.1);
    let distance = radius * zoom.max(1.15);
    let pitch = pitch.clamp(-1.35, 1.35);
    let eye_offset = glam::Vec3::new(
        yaw.sin() * pitch.cos() * distance,
        pitch.sin() * distance,
        yaw.cos() * pitch.cos() * distance,
    );
    let view_dir = eye_offset.normalize_or_zero();
    let right = glam::Vec3::Y
        .cross(view_dir)
        .try_normalize()
        .unwrap_or(glam::Vec3::X);
    let up = view_dir
        .cross(right)
        .try_normalize()
        .unwrap_or(glam::Vec3::Y);
    let target = glam::Vec3::from(center) + (right * pan[0] + up * pan[1]) * radius;
    let eye = target + eye_offset;
    let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y);
    let projection = glam::Mat4::perspective_rh(
        45_f32.to_radians(),
        aspect.max(0.1),
        0.01,
        distance + radius * 6.0,
    );
    let light = glam::Vec3::new(-0.4, 0.7, 0.55).normalize();

    CameraUniform {
        view_proj: (projection * view).to_cols_array_2d(),
        light_dir: [light.x, light.y, light.z, 0.0],
        options: [
            if options.normal_mapping { 1.0 } else { 0.0 },
            options.normal_y_sign,
            0.0,
            0.0,
        ],
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 4],
    options: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialUniform {
    diffuse_color: [f32; 4],
    emissive_color: [f32; 4],
    specular_color: [f32; 4],
    params: [f32; 4],
    properties: [f32; 4],
    render: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PostUniform {
    params: [f32; 4],
}

#[derive(Copy, Clone, Debug)]
struct DrawBatch {
    material_slot: usize,
    index_start: u32,
    index_count: u32,
    transparent: bool,
    render_backfaces: bool,
    center: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv0: [f32; 2],
    bitangent: [f32; 4],
    color: [f32; 4],
}

impl GpuVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2 + std::mem::size_of::<[f32; 2]>())
                        as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2
                        + std::mem::size_of::<[f32; 2]>()
                        + std::mem::size_of::<[f32; 4]>())
                        as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

pub type WeaponRenderOptions = ModelRenderOptions;
pub type WeaponRenderer = ModelRenderer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_batches_sort_back_to_front_without_moving_opaque() {
        let batches = vec![
            test_batch(0, false, [0.0, 0.0, 100.0]),
            test_batch(1, true, [0.0, 0.0, -2.0]),
            test_batch(2, true, [0.0, 0.0, 3.0]),
        ];

        let sorted = sorted_transparent_batches(&batches, 0.0, 0.0);

        assert_eq!(
            sorted
                .iter()
                .map(|batch| batch.material_slot)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn material_alpha_mode_controls_transparent_pass() {
        assert!(!test_material_is_transparent(
            MaterialAlphaMode::Opaque,
            MaterialRenderMode::Opaque
        ));
        assert!(!test_material_is_transparent(
            MaterialAlphaMode::Mask,
            MaterialRenderMode::Opaque
        ));
        assert!(test_material_is_transparent(
            MaterialAlphaMode::Blend,
            MaterialRenderMode::Transparent
        ));
        assert!(test_material_is_transparent(
            MaterialAlphaMode::Glass,
            MaterialRenderMode::Glass
        ));
        assert!(test_material_is_transparent(
            MaterialAlphaMode::Opaque,
            MaterialRenderMode::Transparent
        ));
    }

    fn test_material_is_transparent(
        alpha_mode: MaterialAlphaMode,
        render_mode: MaterialRenderMode,
    ) -> bool {
        let mut material = fallback_material();
        material.alpha_mode = alpha_mode;
        material.render_mode = render_mode;
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![material],
            textures: Vec::new(),
            meshes: Vec::new(),
        };
        material_is_transparent(&model, 0)
    }

    fn test_batch(material_slot: usize, transparent: bool, center: [f32; 3]) -> DrawBatch {
        DrawBatch {
            material_slot,
            index_start: 0,
            index_count: 3,
            transparent,
            render_backfaces: true,
            center,
        }
    }
}
