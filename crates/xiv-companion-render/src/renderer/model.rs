use wgpu::util::DeviceExt;

use crate::{
    MaterialAlphaMode, MaterialRenderMode, ModelMaterial, ModelRenderData, PreparedMaterial,
    PreparedRenderPass, mesh_draw_role_for_category, prepare_material_for_draw_role,
};

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
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 9,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 10,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 11,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 12,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 13,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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

            for batch in self
                .draw_batches
                .iter()
                .filter(|batch| batch.pass().uses_opaque_pipeline())
            {
                render_pass.set_pipeline(if batch.render_backfaces() {
                    &self.pipeline
                } else {
                    &self.culled_pipeline
                });
                draw_model_batch(&mut render_pass, &self.material_bind_groups, batch);
            }

            let sorted_transparent_batches =
                sorted_transparent_batches(&self.draw_batches, yaw, pitch);
            for batch in sorted_transparent_batches {
                render_pass.set_pipeline(if batch.render_backfaces() {
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
        let draw_role = mesh_draw_role_for_category(mesh.mesh_category.as_deref());
        if !draw_role.renders_in_main_pass() {
            continue;
        }

        let base = vertices.len() as u32;
        vertices.extend(mesh.vertices.iter().map(GpuVertex::from_model_vertex));
        let index_start = indices.len() as u32;
        indices.extend(mesh.indices.iter().map(|index| base + *index));
        draw_batches.push(DrawBatch {
            material_slot: mesh.material_slot,
            index_start,
            index_count: mesh.indices.len() as u32,
            prepared_material: prepare_material_for_draw_role(
                model.materials().get(mesh.material_slot),
                draw_role,
            ),
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

fn sorted_transparent_batches(draw_batches: &[DrawBatch], yaw: f32, pitch: f32) -> Vec<&DrawBatch> {
    let sort_dir = transparent_sort_direction(yaw, pitch);
    let mut batches = draw_batches
        .iter()
        .filter(|batch| batch.pass().sorts_back_to_front())
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
    let effective_mask_texture = effective_mask_texture(material);
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
            effective_mask_texture
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
            material
                .specular_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
            if material.apply_vertex_color {
                1.0
            } else {
                0.0
            },
            0.0,
        ],
        render: [
            render_mode_value(material.render_mode),
            material.opacity,
            alpha_mode_value(material.alpha_mode),
            material.alpha_threshold.clamp(0.0, 1.0),
        ],
        extra_properties: material_extra_texture_flags(material, model),
        shader_params: material_shader_params(material),
        tile_params: material_tile_params(material),
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
    let mask_texture_view = effective_mask_texture
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
    let specular_texture_view = material
        .specular_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon specular texture {}", texture.path),
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
                "weapon neutral specular texture",
                1,
                1,
                &[
                    srgb_byte(material.specular_color[0]),
                    srgb_byte(material.specular_color[1]),
                    srgb_byte(material.specular_color[2]),
                    255,
                ],
                wgpu::TextureFormat::Rgba8UnormSrgb,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let tile_properties_texture_view = material
        .tile_properties_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon tile properties texture {}", texture.path),
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
                "weapon neutral tile properties texture",
                1,
                1,
                &[0, 255, 255, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let sheen_properties_texture_view = material
        .sheen_properties_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon sheen properties texture {}", texture.path),
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
                "weapon neutral sheen properties texture",
                1,
                1,
                &[0, 0, 0, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let sphere_properties_texture_view = material
        .sphere_properties_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon sphere properties texture {}", texture.path),
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
                "weapon neutral sphere properties texture",
                1,
                1,
                &[0, 0, 255, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let tile_matrix_texture_view = material
        .tile_matrix_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon tile matrix texture {}", texture.path),
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
                "weapon neutral tile matrix texture",
                1,
                1,
                &[255, 0, 0, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());

    let color_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("weapon material color sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    let data_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("weapon material data sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    let nearest_data_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("weapon material nearest data sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
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
                resource: wgpu::BindingResource::Sampler(&color_sampler),
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
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(&specular_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::Sampler(&data_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: wgpu::BindingResource::TextureView(&tile_properties_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 10,
                resource: wgpu::BindingResource::TextureView(&sheen_properties_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 11,
                resource: wgpu::BindingResource::TextureView(&sphere_properties_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 12,
                resource: wgpu::BindingResource::TextureView(&tile_matrix_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 13,
                resource: wgpu::BindingResource::Sampler(&nearest_data_sampler),
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

fn srgb_byte(linear: f32) -> u8 {
    let value = linear.clamp(0.0, 1.0);
    let srgb = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    unorm_byte(srgb)
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
        normal_scale: 1.0,
        multi_normal_scale: 1.0,
        detail_normal_scale: 1.0,
        multi_detail_normal_scale: 1.0,
        tile_index: 0.0,
        tile_alpha: 1.0,
        tile_scale: [16.0, 16.0],
        opacity: 1.0,
        render_backfaces: true,
        apply_vertex_color: false,
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
        material_map_texture: None,
        multi_map_texture: None,
        specular_texture: None,
        emissive_texture: None,
        material_properties_texture: None,
        tile_properties_texture: None,
        sheen_properties_texture: None,
        sphere_properties_texture: None,
        tile_matrix_texture: None,
    }
}

fn effective_mask_texture(material: &ModelMaterial) -> Option<usize> {
    material.mask_texture
}

fn material_extra_texture_flags<M: ModelRenderData + ?Sized>(
    material: &ModelMaterial,
    model: &M,
) -> [f32; 4] {
    [
        texture_presence_flag(model, material.tile_properties_texture),
        texture_presence_flag(model, material.sheen_properties_texture),
        texture_presence_flag(model, material.sphere_properties_texture),
        texture_presence_flag(model, material.tile_matrix_texture),
    ]
}

fn texture_presence_flag<M: ModelRenderData + ?Sized>(
    model: &M,
    texture_index: Option<usize>,
) -> f32 {
    texture_index
        .and_then(|index| model.textures().get(index))
        .map(|_| 1.0)
        .unwrap_or(0.0)
}

fn material_shader_params(material: &ModelMaterial) -> [f32; 4] {
    [
        material.normal_scale.clamp(0.0, 4.0),
        material.multi_normal_scale.clamp(0.0, 4.0),
        material.detail_normal_scale.clamp(0.0, 4.0),
        material.multi_detail_normal_scale.clamp(0.0, 4.0),
    ]
}

fn material_tile_params(material: &ModelMaterial) -> [f32; 4] {
    [
        finite_or(material.tile_index, 0.0),
        finite_or(material.tile_alpha, 1.0).clamp(0.0, 1.0),
        finite_or(material.tile_scale[0], 16.0),
        finite_or(material.tile_scale[1], 16.0),
    ]
}

fn finite_or(value: f32, default: f32) -> f32 {
    if value.is_finite() { value } else { default }
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
    extra_properties: [f32; 4],
    shader_params: [f32; 4],
    tile_params: [f32; 4],
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
    prepared_material: PreparedMaterial,
    center: [f32; 3],
}

impl DrawBatch {
    fn pass(&self) -> PreparedRenderPass {
        self.prepared_material.render_pass
    }

    fn render_backfaces(&self) -> bool {
        self.prepared_material.render_backfaces
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv0: [f32; 2],
    bitangent: [f32; 4],
    color: [f32; 4],
    uv1: [f32; 2],
    uv2: [f32; 2],
    uv3: [f32; 2],
    color1: [f32; 4],
    normal1: [f32; 3],
    bitangent1: [f32; 4],
    flow0: [f32; 4],
    flow1: [f32; 4],
}

impl GpuVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 13] = [
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, position) as wgpu::BufferAddress,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, normal) as wgpu::BufferAddress,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, uv0) as wgpu::BufferAddress,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, bitangent) as wgpu::BufferAddress,
            shader_location: 3,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, color) as wgpu::BufferAddress,
            shader_location: 4,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, uv1) as wgpu::BufferAddress,
            shader_location: 5,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, uv2) as wgpu::BufferAddress,
            shader_location: 6,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, uv3) as wgpu::BufferAddress,
            shader_location: 7,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, color1) as wgpu::BufferAddress,
            shader_location: 8,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, normal1) as wgpu::BufferAddress,
            shader_location: 9,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, bitangent1) as wgpu::BufferAddress,
            shader_location: 10,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, flow0) as wgpu::BufferAddress,
            shader_location: 11,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, flow1) as wgpu::BufferAddress,
            shader_location: 12,
            format: wgpu::VertexFormat::Float32x4,
        },
    ];

    fn from_model_vertex(vertex: &crate::ModelVertex) -> Self {
        Self {
            position: vertex.position,
            normal: vertex.normal,
            uv0: vertex.uv0,
            bitangent: vertex.bitangent,
            color: vertex.color,
            uv1: vertex.uv1,
            uv2: vertex.uv2,
            uv3: vertex.uv3,
            color1: vertex.color1.unwrap_or([1.0, 1.0, 1.0, 1.0]),
            normal1: vertex.normal1.unwrap_or(vertex.normal),
            bitangent1: vertex.bitangent1.unwrap_or(vertex.bitangent),
            flow0: vertex.flow0.unwrap_or([0.0; 4]),
            flow1: vertex.flow1.unwrap_or([0.0; 4]),
        }
    }

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

pub type WeaponRenderOptions = ModelRenderOptions;
pub type WeaponRenderer = ModelRenderer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MaterialShaderFamily, ModelMeshDrawRole, PreparedTextureBindings,
        PreparedTextureSamplingSet,
    };

    #[test]
    fn transparent_batches_sort_back_to_front_without_moving_opaque() {
        let batches = vec![
            test_batch(0, PreparedRenderPass::Opaque, [0.0, 0.0, 100.0]),
            test_batch(1, PreparedRenderPass::Transparent, [0.0, 0.0, -2.0]),
            test_batch(2, PreparedRenderPass::Glass, [0.0, 0.0, 3.0]),
            test_batch(3, PreparedRenderPass::Cutout, [0.0, 0.0, -100.0]),
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
    fn prepared_material_pass_maps_alpha_modes_and_draw_roles() {
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Opaque
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Mask,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Cutout
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Blend,
                MaterialRenderMode::Transparent,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Transparent
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Glass,
                MaterialRenderMode::Glass,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Glass
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Transparent,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Transparent
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::Glass
            ),
            PreparedRenderPass::Glass
        );
    }

    #[test]
    fn prepared_material_falls_back_for_missing_material_slot() {
        assert_eq!(
            prepare_material_for_draw_role(None, ModelMeshDrawRole::Normal),
            PreparedMaterial {
                render_pass: PreparedRenderPass::Opaque,
                shader_family: MaterialShaderFamily::Unknown,
                texture_bindings: PreparedTextureBindings::default(),
                texture_sampling: PreparedTextureSamplingSet::default(),
                render_backfaces: true,
            }
        );
    }

    #[test]
    fn prepared_render_pass_reports_pipeline_class() {
        assert!(PreparedRenderPass::Opaque.uses_opaque_pipeline());
        assert!(PreparedRenderPass::Cutout.uses_opaque_pipeline());
        assert!(!PreparedRenderPass::Opaque.sorts_back_to_front());
        assert!(!PreparedRenderPass::Cutout.sorts_back_to_front());
        assert!(PreparedRenderPass::Transparent.uses_transparent_pipeline());
        assert!(PreparedRenderPass::Glass.uses_transparent_pipeline());
        assert!(PreparedRenderPass::Transparent.sorts_back_to_front());
        assert!(PreparedRenderPass::Glass.sorts_back_to_front());
    }

    #[test]
    fn prepared_material_preserves_culling_policy() {
        let mut material = fallback_material();
        material.render_backfaces = false;
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![material],
            textures: Vec::new(),
            meshes: Vec::new(),
        };

        assert_eq!(
            prepare_material_for_draw_role(model.materials().first(), ModelMeshDrawRole::Normal),
            PreparedMaterial {
                render_pass: PreparedRenderPass::Opaque,
                shader_family: MaterialShaderFamily::Unknown,
                texture_bindings: PreparedTextureBindings::default(),
                texture_sampling: PreparedTextureSamplingSet::default(),
                render_backfaces: false,
            }
        );
    }

    #[test]
    fn prepared_render_pass_uses_source_order_for_render_mode_fallbacks() {
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Glass,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Glass
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Transparent,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Transparent
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Opaque
        );
    }

    #[test]
    fn effective_mask_texture_uses_only_explicit_mask_sampler() {
        let mut material = fallback_material();
        material.material_map_texture = Some(2);
        material.multi_map_texture = Some(3);

        assert_eq!(effective_mask_texture(&material), None);

        material.mask_texture = Some(1);
        assert_eq!(effective_mask_texture(&material), Some(1));
    }

    #[test]
    fn material_extra_texture_flags_require_loaded_textures() {
        let mut material = fallback_material();
        material.tile_properties_texture = Some(0);
        material.sheen_properties_texture = Some(1);
        material.sphere_properties_texture = Some(99);
        material.tile_matrix_texture = Some(2);
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![material.clone()],
            textures: vec![
                test_texture(crate::ModelTextureKind::TileProperties),
                test_texture(crate::ModelTextureKind::SheenProperties),
                test_texture(crate::ModelTextureKind::TileMatrixProperties),
            ],
            meshes: Vec::new(),
        };

        assert_eq!(
            material_extra_texture_flags(&material, &model),
            [1.0, 1.0, 0.0, 1.0]
        );
    }

    #[test]
    fn material_shader_params_clamp_normal_scales() {
        let mut material = fallback_material();
        assert_eq!(material_shader_params(&material), [1.0, 1.0, 1.0, 1.0]);

        material.normal_scale = 2.25;
        material.multi_normal_scale = 0.5;
        material.detail_normal_scale = 3.5;
        material.multi_detail_normal_scale = f32::INFINITY;
        assert_eq!(material_shader_params(&material), [2.25, 0.5, 3.5, 4.0]);

        material.normal_scale = 8.0;
        material.multi_normal_scale = -1.0;
        material.detail_normal_scale = 8.0;
        material.multi_detail_normal_scale = 0.25;
        assert_eq!(material_shader_params(&material), [4.0, 0.0, 4.0, 0.25]);
    }

    #[test]
    fn material_tile_params_preserve_tile_select_values() {
        let mut material = fallback_material();
        assert_eq!(material_tile_params(&material), [0.0, 1.0, 16.0, 16.0]);

        material.tile_index = 7.0;
        material.tile_alpha = 0.35;
        material.tile_scale = [24.0, 12.0];
        assert_eq!(material_tile_params(&material), [7.0, 0.35, 24.0, 12.0]);

        material.tile_index = f32::INFINITY;
        material.tile_alpha = 8.0;
        material.tile_scale = [f32::NAN, f32::NEG_INFINITY];
        assert_eq!(material_tile_params(&material), [0.0, 1.0, 16.0, 16.0]);
    }

    #[test]
    fn gpu_vertex_layout_exposes_extended_model_channels() {
        let layout = GpuVertex::layout();

        assert_eq!(
            layout.array_stride,
            std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress
        );
        assert_eq!(layout.step_mode, wgpu::VertexStepMode::Vertex);
        assert_eq!(
            layout
                .attributes
                .iter()
                .map(|attribute| (
                    attribute.shader_location,
                    attribute.offset,
                    attribute.format
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    0,
                    std::mem::offset_of!(GpuVertex, position) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x3
                ),
                (
                    1,
                    std::mem::offset_of!(GpuVertex, normal) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x3
                ),
                (
                    2,
                    std::mem::offset_of!(GpuVertex, uv0) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x2
                ),
                (
                    3,
                    std::mem::offset_of!(GpuVertex, bitangent) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x4
                ),
                (
                    4,
                    std::mem::offset_of!(GpuVertex, color) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x4
                ),
                (
                    5,
                    std::mem::offset_of!(GpuVertex, uv1) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x2
                ),
                (
                    6,
                    std::mem::offset_of!(GpuVertex, uv2) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x2
                ),
                (
                    7,
                    std::mem::offset_of!(GpuVertex, uv3) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x2
                ),
                (
                    8,
                    std::mem::offset_of!(GpuVertex, color1) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x4
                ),
                (
                    9,
                    std::mem::offset_of!(GpuVertex, normal1) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x3
                ),
                (
                    10,
                    std::mem::offset_of!(GpuVertex, bitangent1) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x4
                ),
                (
                    11,
                    std::mem::offset_of!(GpuVertex, flow0) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x4
                ),
                (
                    12,
                    std::mem::offset_of!(GpuVertex, flow1) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x4
                ),
            ]
        );
    }

    #[test]
    fn flatten_model_filters_meshes_outside_main_draw_roles() {
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![fallback_material()],
            textures: Vec::new(),
            meshes: vec![
                test_mesh("normal", 0.0),
                test_mesh("shadow", 1.0),
                test_mesh("terrainShadow", 2.0),
                test_mesh("verticalFog", 3.0),
                test_mesh("lightShaft", 4.0),
                test_mesh("materialChange", 5.0),
                test_mesh("glass", 6.0),
            ],
        };

        let (vertices, indices, batches) = flatten_model(&model);

        assert_eq!(vertices.len(), 9);
        assert_eq!(indices.len(), 9);
        assert_eq!(batches.len(), 3);
        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.center[0])
                .collect::<Vec<_>>(),
            vec![0.5, 5.5, 6.5]
        );
        assert_eq!(batches[0].pass(), PreparedRenderPass::Opaque);
        assert_eq!(batches[1].pass(), PreparedRenderPass::Opaque);
        assert_eq!(batches[2].pass(), PreparedRenderPass::Glass);
    }

    #[test]
    fn flatten_model_preserves_extended_vertex_channels() {
        let mut mesh = test_mesh("normal", 0.0);
        mesh.vertices[0].uv1 = [0.1, 0.2];
        mesh.vertices[0].uv2 = [0.3, 0.4];
        mesh.vertices[0].uv3 = [0.5, 0.6];
        mesh.vertices[0].color1 = Some([0.7, 0.8, 0.9, 1.0]);
        mesh.vertices[0].normal1 = Some([1.0, 0.0, 0.0]);
        mesh.vertices[0].bitangent1 = Some([0.0, 1.0, 0.0, -1.0]);
        mesh.vertices[0].flow0 = Some([0.11, 0.22, 0.33, 0.44]);
        mesh.vertices[0].flow1 = Some([0.55, 0.66, 0.77, 0.88]);
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![fallback_material()],
            textures: Vec::new(),
            meshes: vec![mesh],
        };

        let (vertices, _, _) = flatten_model(&model);

        assert_eq!(vertices[0].uv1, [0.1, 0.2]);
        assert_eq!(vertices[0].uv2, [0.3, 0.4]);
        assert_eq!(vertices[0].uv3, [0.5, 0.6]);
        assert_eq!(vertices[0].color1, [0.7, 0.8, 0.9, 1.0]);
        assert_eq!(vertices[0].normal1, [1.0, 0.0, 0.0]);
        assert_eq!(vertices[0].bitangent1, [0.0, 1.0, 0.0, -1.0]);
        assert_eq!(vertices[0].flow0, [0.11, 0.22, 0.33, 0.44]);
        assert_eq!(vertices[0].flow1, [0.55, 0.66, 0.77, 0.88]);
        assert_eq!(vertices[1].color1, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(vertices[1].normal1, [0.0, 1.0, 0.0]);
        assert_eq!(vertices[1].bitangent1, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(vertices[1].flow0, [0.0; 4]);
        assert_eq!(vertices[1].flow1, [0.0; 4]);
    }

    fn test_prepared_render_pass(
        alpha_mode: MaterialAlphaMode,
        render_mode: MaterialRenderMode,
        draw_role: ModelMeshDrawRole,
    ) -> PreparedRenderPass {
        let mut material = fallback_material();
        material.alpha_mode = alpha_mode;
        material.render_mode = render_mode;
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![material],
            textures: Vec::new(),
            meshes: Vec::new(),
        };
        prepare_material_for_draw_role(model.materials().first(), draw_role).render_pass
    }

    fn test_batch(material_slot: usize, pass: PreparedRenderPass, center: [f32; 3]) -> DrawBatch {
        DrawBatch {
            material_slot,
            index_start: 0,
            index_count: 3,
            prepared_material: PreparedMaterial {
                render_pass: pass,
                shader_family: MaterialShaderFamily::Unknown,
                texture_bindings: PreparedTextureBindings::default(),
                texture_sampling: PreparedTextureSamplingSet::default(),
                render_backfaces: true,
            },
            center,
        }
    }

    fn test_mesh(category: &str, x: f32) -> crate::ModelMesh {
        crate::ModelMesh {
            path: format!("test/{category}.mdl"),
            part_index: 0,
            mesh_category: Some(category.to_string()),
            material_index: 0,
            material_slot: 0,
            material_name: "test".to_string(),
            color: [1.0, 1.0, 1.0],
            bone_table: None,
            vertices: vec![
                test_vertex([x, 0.0, 0.0]),
                test_vertex([x + 1.0, 0.0, 0.0]),
                test_vertex([x, 1.0, 0.0]),
            ],
            indices: vec![0, 1, 2],
        }
    }

    fn test_vertex(position: [f32; 3]) -> crate::ModelVertex {
        crate::ModelVertex {
            position,
            blend_weights: None,
            blend_indices: None,
            normal: [0.0, 1.0, 0.0],
            uv0: [0.0, 0.0],
            uv1: [0.0, 0.0],
            uv2: [0.0, 0.0],
            uv3: [0.0, 0.0],
            bitangent: [1.0, 0.0, 0.0, 1.0],
            normal1: None,
            bitangent1: None,
            color: [1.0, 1.0, 1.0, 1.0],
            color1: None,
            flow0: None,
            flow1: None,
        }
    }

    fn test_texture(kind: crate::ModelTextureKind) -> crate::ModelTexture {
        crate::ModelTexture {
            path: "test.tex".to_string(),
            kind,
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
            rgba_f32: None,
        }
    }
}
