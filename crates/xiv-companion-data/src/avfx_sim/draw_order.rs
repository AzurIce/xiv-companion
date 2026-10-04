/// Root DwLy's runtime group, before any external scene override.
/// Client 0x1403acff1..3ad000 packs the low five bits into root +0x5c;
/// registration 0x1403b58a6..58b2 extracts them. This is not a Context pass.
/// Values outside the known dispatched groups remain representable here.
pub fn draw_layer_group(authored: i32) -> u8 {
    (authored as u8) & 0x1f
}

/// Extract the depth row of the column-major view matrix supplied at camera
/// registration. Client 0x1403b7ec7..7f21 packs the first three components of
/// each column; depth registration reads packed offsets 8, 20, 32, and 44.
/// Pass the view matrix, not a projection or view-projection matrix.
pub fn depth_sort_camera_row(view_columns: [[f32; 4]; 4]) -> [f32; 4] {
    view_columns.map(|column| column[2])
}

/// Build the client's float key for ordering Documents within a draw group.
///
/// `position` is the Document transform's position at group preparation, not
/// a sampled particle position. `registration_serial` is the manager's
/// monotonically incremented Document registration number. The caller must
/// supply the camera's packed view-depth row for that Document's camera slot.
/// Client 0x1403b918e..3b923e uses this key before allocating each Document's
/// ordinary and independent draw-command ranges.
pub fn document_sort_key(
    position: [f32; 3],
    camera_depth_row: [f32; 4],
    soft_key_offset: f32,
    registration_serial: u32,
) -> f32 {
    // Keep scalar SSE operation order. In particular, the first sum begins
    // with Y and the registration remainder is added after scaling by 10.
    let y = camera_depth_row[1] * position[1];
    let x = camera_depth_row[0] * position[0];
    let z = camera_depth_row[2] * position[2];
    let depth = (((y + x) + z) + camera_depth_row[3]) + soft_key_offset;
    depth * 10.0 + (registration_serial % 1000) as f32 * 0.01
}

/// Build the client's unsigned Depth ordering key from the matrix translation
/// at registration, the camera's depth row, and the shared registration count.
///
/// The caller owns registration timing and the counter shared across documents
/// (including the independent particle list). This does not sort a playback or
/// infer a camera from the final rendered position.
pub fn depth_sort_key(position: [f32; 3], camera_depth_row: [f32; 4], count: u32) -> u64 {
    // 0x1403e5d3b..7a: preserve scalar SSE operation order and truncation.
    let y = camera_depth_row[1] * position[1];
    let x = camera_depth_row[0] * position[0];
    let z = camera_depth_row[2] * position[2];
    let depth = ((y + x) + z) + camera_depth_row[3];
    let scaled = depth * 1000.0;
    // CVTTSS2SI returns integer-indefinite for NaN/overflow, unlike Rust's
    // saturating float cast. The upper i64 bound is exclusive in f32.
    let quantized = if scaled.is_finite()
        && scaled >= -9_223_372_036_854_775_808.0_f32
        && scaled < 9_223_372_036_854_775_808.0_f32
    {
        scaled as i64
    } else {
        i64::MIN
    };
    (quantized as u64)
        .wrapping_add(0x7fff_ffff_ffff)
        .wrapping_shl(16)
        | u64::from(count as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_layer_group_preserves_five_bit_aliases_and_unknown_groups() {
        for (authored, expected) in [
            (0, 0),
            (2, 2),
            (10, 10),
            (11, 11),
            (31, 31),
            (32, 0),
            (257, 1),
            (-1, 31),
            (-22, 10),
        ] {
            assert_eq!(draw_layer_group(authored), expected);
        }
    }

    #[test]
    fn depth_camera_row_keeps_view_translation_and_rotated_axis() {
        // A view looking along world X; off-axis Y/Z must not become depth.
        let row = depth_sort_camera_row([
            [0.0, 0.0, -1.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [7.0, 8.0, 3.0, 1.0],
        ]);
        assert_eq!(row, [-1.0, 0.0, 0.0, 3.0]);
        assert_eq!(
            depth_sort_key([4.0, 99.0, 88.0], row, 0),
            0x7fff_ffff_fc17_0000
        );
        assert!(
            depth_sort_key([5.0, 0.0, 0.0], row, 0) < depth_sort_key([4.0, 99.0, 88.0], row, 0)
        );
    }

    #[test]
    fn document_key_uses_document_position_sko_and_registration_serial() {
        let row = depth_sort_camera_row([
            [-100_000_000.0, 0.0, -100_000_000.0, 0.0],
            [100_000_000.0, 0.0, 100_000_000.0, 0.0],
            [1.0, 0.0, 1.0, 0.0],
            [2.0, 0.0, 2.0, 1.0],
        ]);
        // Y + X cancels first; adding Z earlier would lose it to rounding.
        let key = document_sort_key([1.0; 3], row, 0.25, 1123);
        assert_eq!(key.to_bits(), (32.5_f32 + 123.0_f32 * 0.01).to_bits());
        assert_eq!(key, document_sort_key([1.0; 3], row, 0.25, 123));
        assert!(document_sort_key([1.0; 3], row, -0.25, 123) < key);
        assert!(document_sort_key([1.0; 3], row, 0.25, 122) < key);
        assert!(document_sort_key([1.0, 1.0, 2.0], row, 0.25, 123) > key);
    }

    #[test]
    fn depth_quantization_truncates_both_signs_and_uses_all_camera_terms() {
        let row = [0.0, 0.0, 1.0, 0.0];
        assert_eq!(
            depth_sort_key([0.0, 0.0, 0.0019], row, 7),
            0x8000_0000_0000_0007
        );
        assert_eq!(
            depth_sort_key([0.0, 0.0, -0.0019], row, 7),
            0x7fff_ffff_fffe_0007
        );
        assert_eq!(
            depth_sort_key([2.0, 3.0, 4.0], [1.0, 2.0, -3.0, 4.0], 7),
            0x7fff_ffff_ffff_0007
        );
        // Finite depths in the same thousandth are ordered by registration.
        assert!(
            depth_sort_key([0.0, 0.0, 0.0009], row, 1)
                < depth_sort_key([0.0, 0.0, -0.0009], row, 2)
        );
    }

    #[test]
    fn depth_key_counter_wraps_without_carrying_into_depth() {
        assert_eq!(
            depth_sort_key([0.0; 3], [0.0; 4], 0x1_0001),
            0x7fff_ffff_ffff_0001
        );
        assert!(
            depth_sort_key([0.0; 3], [0.0; 4], 65536) < depth_sort_key([0.0; 3], [0.0; 4], 65535)
        );
    }

    #[test]
    fn depth_key_preserves_sse_invalid_conversion() {
        for depth in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
            assert_eq!(
                depth_sort_key([0.0; 3], [0.0, 0.0, 0.0, depth], 9),
                0x7fff_ffff_ffff_0009
            );
        }
    }
}
