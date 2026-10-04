use super::*;

pub(crate) fn create_array_pair_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    pair: Option<(&ModelTexture, &ModelTexture)>,
    first_semantic: RgbaMipSemantic,
    second_semantic: RgbaMipSemantic,
    fallback_first: [u8; 4],
    fallback_second: [u8; 4],
) -> wgpu::Texture {
    let Some((first, second)) = pair else {
        let rgba = [
            fallback_first[0],
            fallback_first[1],
            fallback_first[2],
            fallback_first[3],
            fallback_second[0],
            fallback_second[1],
            fallback_second[2],
            fallback_second[3],
        ];
        return create_rgba_texture(
            device,
            queue,
            label,
            2,
            1,
            &rgba,
            wgpu::TextureFormat::Rgba8Unorm,
        );
    };

    let levels = array_pair_mip_chain(first, second, first_semantic, second_semantic);
    let base = levels
        .first()
        .expect("array pair mip chain has a base level");
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: base.width,
            height: base.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (mip_level, level) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip_level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &level.rgba,
            if level.height == 1 {
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: None,
                    rows_per_image: None,
                }
            } else {
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(level.width * 4),
                    rows_per_image: Some(level.height),
                }
            },
            wgpu::Extent3d {
                width: level.width,
                height: level.height,
                depth_or_array_layers: 1,
            },
        );
    }
    texture
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RgbaMipSemantic {
    SrgbColor,
    LinearData,
    /// Normal XY are packed in RG; B/A remain independent scalar payloads such as alpha masks.
    PackedNormalRg,
}

/// Maps a prepared sampling color space to the mip/upload semantic of the
/// bound texture. `Srgb` uploads decode RGB on the GPU (`Rgba8UnormSrgb`,
/// alpha stays linear); `NonColor` stays raw linear data. This is the single
/// place where the prepared policy becomes a GPU format decision; special
/// textures (packed normals, float ramps, pair atlases, neutral fallbacks)
/// stay explicit at their call sites instead of being inferred here.
pub(crate) fn mip_semantic_for_color_space(
    color_space: PreparedTextureColorSpace,
) -> RgbaMipSemantic {
    match color_space {
        PreparedTextureColorSpace::Srgb => RgbaMipSemantic::SrgbColor,
        PreparedTextureColorSpace::NonColor => RgbaMipSemantic::LinearData,
    }
}

pub(crate) fn texture_format_for_color_space(
    color_space: PreparedTextureColorSpace,
) -> wgpu::TextureFormat {
    match color_space {
        PreparedTextureColorSpace::Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        PreparedTextureColorSpace::NonColor => wgpu::TextureFormat::Rgba8Unorm,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RgbaMipLevel {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

pub(crate) fn array_pair_mip_chain(
    first: &ModelTexture,
    second: &ModelTexture,
    first_semantic: RgbaMipSemantic,
    second_semantic: RgbaMipSemantic,
) -> Vec<RgbaMipLevel> {
    debug_assert!(texture_array_pair_is_compatible(first, second));
    let layer_count = u32::from(first.array_size).max(1);
    let mut layer_width = u32::from(first.width).max(1);
    let mut layer_height = u32::from(first.array_layer_height).max(1);
    let width = layer_width * 2;
    let height = layer_height * layer_count;
    let mut base = vec![0; width as usize * height as usize * 4];
    for y in 0..height as usize {
        let source_offset = y * layer_width as usize * 4;
        let target_offset = y * width as usize * 4;
        let row_bytes = layer_width as usize * 4;
        base[target_offset..target_offset + row_bytes]
            .copy_from_slice(&first.rgba[source_offset..source_offset + row_bytes]);
        base[target_offset + row_bytes..target_offset + row_bytes * 2]
            .copy_from_slice(&second.rgba[source_offset..source_offset + row_bytes]);
    }

    let mut levels = vec![RgbaMipLevel {
        width,
        height,
        rgba: base,
    }];
    while layer_width > 1 && layer_height > 1 && layer_width % 2 == 0 && layer_height % 2 == 0 {
        let source = levels
            .last()
            .expect("array pair mip chain has a base level");
        let target_layer_width = layer_width / 2;
        let target_layer_height = layer_height / 2;
        let target_width = target_layer_width * 2;
        let target_height = target_layer_height * layer_count;
        let mut target = vec![0; target_width as usize * target_height as usize * 4];

        for (side, semantic) in [first_semantic, second_semantic].into_iter().enumerate() {
            let source_side_x = side as u32 * layer_width;
            let target_side_x = side as u32 * target_layer_width;
            for layer in 0..layer_count {
                let source_layer_y = layer * layer_height;
                let target_layer_y = layer * target_layer_height;
                for target_y in 0..target_layer_height {
                    let source_y_start =
                        source_layer_y + target_y * layer_height / target_layer_height;
                    let source_y_end = source_layer_y
                        + ((target_y + 1) * layer_height / target_layer_height)
                            .max(target_y * layer_height / target_layer_height + 1);
                    for target_x in 0..target_layer_width {
                        let source_x_start =
                            source_side_x + target_x * layer_width / target_layer_width;
                        let source_x_end = source_side_x
                            + ((target_x + 1) * layer_width / target_layer_width)
                                .max(target_x * layer_width / target_layer_width + 1);
                        let output_x = target_side_x + target_x;
                        let output_y = target_layer_y + target_y;
                        let target_offset = ((output_y * target_width + output_x) * 4) as usize;
                        downsample_rgba_texel(
                            source,
                            source_x_start,
                            source_x_end,
                            source_y_start,
                            source_y_end,
                            semantic,
                            &mut target[target_offset..target_offset + 4],
                        );
                    }
                }
            }
        }

        levels.push(RgbaMipLevel {
            width: target_width,
            height: target_height,
            rgba: target,
        });
        layer_width = target_layer_width;
        layer_height = target_layer_height;
    }
    levels
}

pub(crate) fn create_mipped_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
    semantic: RgbaMipSemantic,
) -> wgpu::Texture {
    let levels = rgba_mip_chain(width, height, rgba, semantic);
    create_rgba_texture_from_mips(device, queue, label, &levels, semantic)
}

pub(crate) fn create_rgba_texture_from_mips(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    levels: &[RgbaMipLevel],
    semantic: RgbaMipSemantic,
) -> wgpu::Texture {
    let base = levels.first().expect("RGBA mip chain has a base level");
    let format = match semantic {
        RgbaMipSemantic::SrgbColor => wgpu::TextureFormat::Rgba8UnormSrgb,
        RgbaMipSemantic::LinearData | RgbaMipSemantic::PackedNormalRg => {
            wgpu::TextureFormat::Rgba8Unorm
        }
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: base.width,
            height: base.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (mip_level, level) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip_level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &level.rgba,
            if level.height == 1 {
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: None,
                    rows_per_image: None,
                }
            } else {
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(level.width * 4),
                    rows_per_image: Some(level.height),
                }
            },
            wgpu::Extent3d {
                width: level.width,
                height: level.height,
                depth_or_array_layers: 1,
            },
        );
    }
    texture
}

pub(crate) fn rgba_mip_chain(
    width: u32,
    height: u32,
    rgba: &[u8],
    semantic: RgbaMipSemantic,
) -> Vec<RgbaMipLevel> {
    let width = width.max(1);
    let height = height.max(1);
    let expected_len = width as usize * height as usize * 4;
    let mut base = vec![0; expected_len];
    let copy_len = expected_len.min(rgba.len());
    base[..copy_len].copy_from_slice(&rgba[..copy_len]);
    let mut levels = vec![RgbaMipLevel {
        width,
        height,
        rgba: base,
    }];

    while levels
        .last()
        .is_some_and(|level| level.width > 1 || level.height > 1)
    {
        let source = levels.last().expect("mip chain has a base level");
        let target_width = (source.width / 2).max(1);
        let target_height = (source.height / 2).max(1);
        let mut target = vec![0; target_width as usize * target_height as usize * 4];
        for target_y in 0..target_height {
            let source_y_start = target_y * source.height / target_height;
            let source_y_end =
                ((target_y + 1) * source.height / target_height).max(source_y_start + 1);
            for target_x in 0..target_width {
                let source_x_start = target_x * source.width / target_width;
                let source_x_end =
                    ((target_x + 1) * source.width / target_width).max(source_x_start + 1);
                let target_offset = ((target_y * target_width + target_x) * 4) as usize;
                downsample_rgba_texel(
                    source,
                    source_x_start,
                    source_x_end,
                    source_y_start,
                    source_y_end,
                    semantic,
                    &mut target[target_offset..target_offset + 4],
                );
            }
        }
        levels.push(RgbaMipLevel {
            width: target_width,
            height: target_height,
            rgba: target,
        });
    }
    levels
}

pub(crate) fn downsample_rgba_texel(
    source: &RgbaMipLevel,
    x_start: u32,
    x_end: u32,
    y_start: u32,
    y_end: u32,
    semantic: RgbaMipSemantic,
    target: &mut [u8],
) {
    let sample_count = ((x_end - x_start) * (y_end - y_start)) as f32;
    let mut sums = [0.0; 5];
    for y in y_start..y_end {
        for x in x_start..x_end {
            let offset = ((y * source.width + x) * 4) as usize;
            let pixel = &source.rgba[offset..offset + 4];
            match semantic {
                RgbaMipSemantic::SrgbColor => {
                    for channel in 0..3 {
                        sums[channel] += srgb_to_linear(pixel[channel]);
                    }
                    sums[3] += f32::from(pixel[3]) / 255.0;
                }
                RgbaMipSemantic::LinearData => {
                    for channel in 0..4 {
                        sums[channel] += f32::from(pixel[channel]) / 255.0;
                    }
                }
                RgbaMipSemantic::PackedNormalRg => {
                    let x = f32::from(pixel[0]) / 127.5 - 1.0;
                    let y = f32::from(pixel[1]) / 127.5 - 1.0;
                    sums[0] += x;
                    sums[1] += y;
                    sums[2] += (1.0 - x * x - y * y).max(0.0).sqrt();
                    sums[3] += f32::from(pixel[2]) / 255.0;
                    sums[4] += f32::from(pixel[3]) / 255.0;
                }
            }
        }
    }

    match semantic {
        RgbaMipSemantic::SrgbColor => {
            for channel in 0..3 {
                target[channel] = linear_to_srgb_byte(sums[channel] / sample_count);
            }
        }
        RgbaMipSemantic::LinearData => {
            for channel in 0..3 {
                target[channel] = unorm_byte(sums[channel] / sample_count);
            }
        }
        RgbaMipSemantic::PackedNormalRg => {
            let averaged = [
                sums[0] / sample_count,
                sums[1] / sample_count,
                sums[2] / sample_count,
            ];
            let length =
                (averaged[0] * averaged[0] + averaged[1] * averaged[1] + averaged[2] * averaged[2])
                    .sqrt();
            let xy = if length > 1.0e-6 {
                [averaged[0] / length, averaged[1] / length]
            } else {
                [0.0, 0.0]
            };
            target[0] = unorm_byte(xy[0] * 0.5 + 0.5);
            target[1] = unorm_byte(xy[1] * 0.5 + 0.5);
            target[2] = unorm_byte(sums[3] / sample_count);
            target[3] = unorm_byte(sums[4] / sample_count);
        }
    }
    if semantic != RgbaMipSemantic::PackedNormalRg {
        target[3] = unorm_byte(sums[3] / sample_count);
    }
}

pub(crate) fn srgb_to_linear(value: u8) -> f32 {
    let value = f32::from(value) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub(crate) fn linear_to_srgb_byte(value: f32) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let encoded = if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    unorm_byte(encoded)
}

pub(crate) fn create_rgba_texture(
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

pub(crate) fn create_tile_matrix_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    source: Option<&ModelTexture>,
) -> wgpu::Texture {
    let (width, height, pixels) = tile_matrix_texture_pixels(source);
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
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytemuck::cast_slice(&pixels),
        if height == 1 {
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: None,
                rows_per_image: None,
            }
        } else {
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 16),
                rows_per_image: Some(height),
            }
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    texture
}

pub(crate) fn create_float_ramp_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    source: Option<&ModelTexture>,
    neutral: [f32; 4],
) -> wgpu::Texture {
    let (width, height, pixels) = float_ramp_texture_pixels(source, neutral);
    let mut bytes = Vec::with_capacity(pixels.len() * 8);
    for pixel in pixels {
        for channel in pixel {
            bytes.extend_from_slice(&f16::from_f32(channel).to_bits().to_le_bytes());
        }
    }
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
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &bytes,
        if height == 1 {
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: None,
                rows_per_image: None,
            }
        } else {
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 8),
                rows_per_image: Some(height),
            }
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    texture
}

pub(crate) fn float_ramp_texture_pixels(
    source: Option<&ModelTexture>,
    neutral: [f32; 4],
) -> (u32, u32, Vec<[f32; 4]>) {
    let Some(source) = source.filter(|texture| texture.width != 0 && texture.height != 0) else {
        return (1, 1, vec![neutral]);
    };
    let width = u32::from(source.width);
    let height = u32::from(source.height);
    let pixel_count = usize::from(source.width) * usize::from(source.height);
    let float_pixels = source
        .rgba_f32
        .as_deref()
        .filter(|pixels| pixels.len() == pixel_count);
    let unorm_pixels = (source.rgba.len() >= pixel_count * 4).then_some(source.rgba.as_slice());
    let pixels = (0..pixel_count)
        .map(|pixel_index| {
            let mut fallback = neutral;
            if let Some(unorm_pixels) = unorm_pixels {
                for channel in 0..4 {
                    fallback[channel] = f32::from(unorm_pixels[pixel_index * 4 + channel]) / 255.0;
                }
            }
            let Some(float_pixel) = float_pixels.map(|pixels| pixels[pixel_index]) else {
                return fallback;
            };
            for channel in 0..4 {
                if float_pixel[channel].is_finite() {
                    fallback[channel] = float_pixel[channel];
                }
            }
            fallback
        })
        .collect();
    (width, height, pixels)
}

pub(crate) fn tile_matrix_texture_pixels(
    source: Option<&ModelTexture>,
) -> (u32, u32, Vec<[f32; 4]>) {
    let Some(source) = source.filter(|texture| texture.width != 0 && texture.height != 0) else {
        return (1, 1, vec![[1.0, 0.0, 0.0, 1.0]]);
    };
    let width = u32::from(source.width);
    let height = u32::from(source.height);
    let pixel_count = usize::from(source.width) * usize::from(source.height);
    let float_pixels = source
        .rgba_f32
        .as_deref()
        .filter(|pixels| pixels.len() == pixel_count);
    let unorm_pixels = (source.rgba.len() >= pixel_count * 4).then_some(source.rgba.as_slice());
    let pixels = (0..pixel_count)
        .map(|pixel_index| {
            let mut fallback = [1.0, 0.0, 0.0, 1.0];
            if let Some(unorm_pixels) = unorm_pixels {
                for channel in 0..4 {
                    fallback[channel] = f32::from(unorm_pixels[pixel_index * 4 + channel]) / 255.0;
                }
            }
            let Some(float_pixel) = float_pixels.map(|pixels| pixels[pixel_index]) else {
                return fallback;
            };
            for channel in 0..4 {
                if float_pixel[channel].is_finite() {
                    fallback[channel] = float_pixel[channel];
                }
            }
            fallback
        })
        .collect();
    (width, height, pixels)
}

pub(crate) fn unorm_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
