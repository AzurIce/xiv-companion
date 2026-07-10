const TEXTURE_TYPE_2D_ARRAY: u32 = 0x1000_0000;

pub struct DecodedTextureRgba {
    pub width: u16,
    pub height: u16,
    pub array_size: u16,
    pub array_layer_height: u16,
    pub rgba: Vec<u8>,
}

pub fn decode_texture_rgba(texture: &physis::tex::Texture) -> Option<Vec<u8>> {
    match texture.format {
        physis::tex::TextureFormat::BC2_UNORM => decode_bc2_rgba(
            &texture.data,
            texture.width as usize,
            texture.height as usize,
            texture.depth as usize,
        ),
        _ => texture.to_rgba(),
    }
}

pub fn decode_texture_rgba_with_layout(
    texture: &mut physis::tex::Texture,
    bytes: &[u8],
) -> Option<DecodedTextureRgba> {
    let array_size = texture_array_size(bytes);
    let array_layer_height = texture.height;
    let original_depth = texture.depth;
    if array_size > 1 {
        texture.depth = array_size;
    }
    let rgba = decode_texture_rgba(texture);
    texture.depth = original_depth;
    let rgba = rgba?;
    let height = array_layer_height.checked_mul(array_size)?;
    let expected_len = usize::from(texture.width)
        .checked_mul(usize::from(height))?
        .checked_mul(4)?;
    if rgba.len() != expected_len {
        return None;
    }

    Some(DecodedTextureRgba {
        width: texture.width,
        height,
        array_size,
        array_layer_height,
        rgba,
    })
}

fn texture_array_size(bytes: &[u8]) -> u16 {
    let Some(attribute) = bytes
        .get(0..4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
    else {
        return 1;
    };
    if attribute & TEXTURE_TYPE_2D_ARRAY == 0 {
        return 1;
    }
    bytes.get(15).copied().unwrap_or(0).max(1).into()
}

fn decode_bc2_rgba(data: &[u8], width: usize, height: usize, depth: usize) -> Option<Vec<u8>> {
    let height = height.checked_mul(depth.max(1))?;
    let pixel_count = width.checked_mul(height)?;
    let mut rgba = vec![0_u8; pixel_count.checked_mul(4)?];
    if width == 0 || height == 0 {
        return Some(rgba);
    }

    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let required = blocks_x.checked_mul(blocks_y)?.checked_mul(16)?;
    if data.len() < required {
        return None;
    }

    let mut block_offset = 0;
    for block_y in 0..blocks_y {
        for block_x in 0..blocks_x {
            let block = &data[block_offset..block_offset + 16];
            let pixels = decode_bc2_block(block);
            copy_decoded_block(&pixels, block_x, block_y, width, height, &mut rgba);
            block_offset += 16;
        }
    }

    Some(rgba)
}

fn decode_bc2_block(block: &[u8]) -> [[u8; 4]; 16] {
    let palette = bc2_color_palette(&block[8..16]);
    let mut color_indices = u32::from_le_bytes(block[12..16].try_into().expect("color indices"));
    let mut pixels = [[0_u8; 4]; 16];

    for index in 0..16 {
        let alpha_byte = block[index / 2];
        let alpha = if index % 2 == 0 {
            alpha_byte & 0x0f
        } else {
            alpha_byte >> 4
        } * 17;
        let color = palette[(color_indices & 0x03) as usize];
        pixels[index] = [color[0], color[1], color[2], alpha];
        color_indices >>= 2;
    }

    pixels
}

fn bc2_color_palette(color_block: &[u8]) -> [[u8; 3]; 4] {
    let color_0 = u16::from_le_bytes([color_block[0], color_block[1]]);
    let color_1 = u16::from_le_bytes([color_block[2], color_block[3]]);
    let [r0, g0, b0] = rgb565(color_0);
    let [r1, g1, b1] = rgb565(color_1);
    [
        [r0, g0, b0],
        [r1, g1, b1],
        [
            lerp_u8_thirds(r0, r1, 2, 1),
            lerp_u8_thirds(g0, g1, 2, 1),
            lerp_u8_thirds(b0, b1, 2, 1),
        ],
        [
            lerp_u8_thirds(r0, r1, 1, 2),
            lerp_u8_thirds(g0, g1, 1, 2),
            lerp_u8_thirds(b0, b1, 1, 2),
        ],
    ]
}

fn rgb565(value: u16) -> [u8; 3] {
    [
        ((value >> 8) & 0xf8) as u8 | (value >> 13) as u8,
        ((value >> 3) & 0xfc) as u8 | ((value >> 9) & 0x03) as u8,
        (value << 3) as u8 | ((value >> 2) & 0x07) as u8,
    ]
}

fn lerp_u8_thirds(a: u8, b: u8, a_weight: u16, b_weight: u16) -> u8 {
    ((u16::from(a) * a_weight + u16::from(b) * b_weight) / 3) as u8
}

fn copy_decoded_block(
    block: &[[u8; 4]; 16],
    block_x: usize,
    block_y: usize,
    width: usize,
    height: usize,
    rgba: &mut [u8],
) {
    let x0 = block_x * 4;
    let y0 = block_y * 4;
    let copy_width = (width - x0).min(4);
    let copy_height = (height - y0).min(4);

    for y in 0..copy_height {
        for x in 0..copy_width {
            let src = (y * 4 + x) * 4;
            let dst = ((y0 + y) * width + x0 + x) * 4;
            rgba[dst..dst + 4].copy_from_slice(&block[y * 4 + x]);
            debug_assert_eq!(src, (y * 4 + x) * 4);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_array_size_reads_ffxiv_header_array_byte() {
        let mut bytes = [0_u8; 16];
        bytes[0..4].copy_from_slice(&TEXTURE_TYPE_2D_ARRAY.to_le_bytes());
        bytes[15] = 64;
        assert_eq!(texture_array_size(&bytes), 64);

        bytes[0..4].copy_from_slice(&0x0080_0000_u32.to_le_bytes());
        assert_eq!(texture_array_size(&bytes), 1);
    }

    #[test]
    fn bc2_decodes_explicit_alpha_and_rgb565_color() {
        let mut block = [0_u8; 16];
        for index in 0..16 {
            let alpha = index as u8;
            if index % 2 == 0 {
                block[index / 2] |= alpha;
            } else {
                block[index / 2] |= alpha << 4;
            }
        }
        block[8..10].copy_from_slice(&0xf800_u16.to_le_bytes());
        block[10..12].copy_from_slice(&0x07e0_u16.to_le_bytes());

        let rgba = decode_bc2_rgba(&block, 4, 4, 1).expect("decoded bc2");
        assert_eq!(&rgba[0..4], &[255, 0, 0, 0]);
        assert_eq!(&rgba[4..8], &[255, 0, 0, 17]);
        assert_eq!(&rgba[60..64], &[255, 0, 0, 255]);
    }

    #[test]
    fn bc2_crops_partial_edge_blocks() {
        let mut block = [0_u8; 16];
        block[0..8].fill(0xff);
        block[8..10].copy_from_slice(&0x001f_u16.to_le_bytes());

        let rgba = decode_bc2_rgba(&block, 2, 2, 1).expect("decoded bc2");
        assert_eq!(rgba.len(), 16);
        assert_eq!(&rgba[0..4], &[0, 0, 255, 255]);
        assert_eq!(&rgba[12..16], &[0, 0, 255, 255]);
    }

    #[test]
    fn bc2_uses_color_selectors() {
        let mut block = [0_u8; 16];
        block[0..8].fill(0xff);
        block[8..10].copy_from_slice(&0xf800_u16.to_le_bytes());
        block[10..12].copy_from_slice(&0x07e0_u16.to_le_bytes());
        block[12..16].copy_from_slice(&0b11_10_01_00_u32.to_le_bytes());

        let rgba = decode_bc2_rgba(&block, 4, 4, 1).expect("decoded bc2");
        assert_eq!(&rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&rgba[4..8], &[0, 255, 0, 255]);
        assert_eq!(&rgba[8..12], &[170, 85, 0, 255]);
        assert_eq!(&rgba[12..16], &[85, 170, 0, 255]);
    }

    #[test]
    fn bc2_rejects_short_data() {
        assert!(decode_bc2_rgba(&[0; 15], 4, 4, 1).is_none());
    }
}
