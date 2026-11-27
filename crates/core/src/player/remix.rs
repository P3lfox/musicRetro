//! Channel-count adaptation between the decoded stream and the device.
//!
//! Only used when the device cannot open the track's native channel count.

/// Appends `src` (interleaved, `src_ch` channels) to `dst` as interleaved
/// `dst_ch` channels.
///
/// - Same layout: copied as-is (bit-perfect).
/// - Mono source: duplicated to every output channel.
/// - Fewer source channels: extra output channels are silent.
/// - More source channels: extra source channels are dropped.
pub fn remix_into(src: &[f32], src_ch: usize, dst_ch: usize, dst: &mut Vec<f32>) {
    if src_ch == 0 || dst_ch == 0 {
        return;
    }
    if src_ch == dst_ch {
        dst.extend_from_slice(src);
        return;
    }
    dst.reserve(src.len() / src_ch * dst_ch);
    for frame in src.chunks_exact(src_ch) {
        for ch in 0..dst_ch {
            let sample = if src_ch == 1 {
                frame[0]
            } else {
                frame.get(ch).copied().unwrap_or(0.0)
            };
            dst.push(sample);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::remix_into;

    fn remix(src: &[f32], src_ch: usize, dst_ch: usize) -> Vec<f32> {
        let mut out = Vec::new();
        remix_into(src, src_ch, dst_ch, &mut out);
        out
    }

    #[test]
    fn same_layout_is_copied_unchanged() {
        assert_eq!(remix(&[0.1, 0.2, 0.3, 0.4], 2, 2), vec![0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn mono_is_duplicated_to_stereo() {
        assert_eq!(remix(&[0.5, -0.5], 1, 2), vec![0.5, 0.5, -0.5, -0.5]);
    }

    #[test]
    fn stereo_to_quad_pads_with_silence() {
        assert_eq!(remix(&[0.1, 0.2], 2, 4), vec![0.1, 0.2, 0.0, 0.0]);
    }

    #[test]
    fn surround_to_stereo_keeps_front_pair() {
        assert_eq!(remix(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6], 6, 2), vec![0.1, 0.2]);
    }

    #[test]
    fn appends_to_existing_buffer() {
        let mut out = vec![9.0];
        remix_into(&[0.1, 0.2], 2, 2, &mut out);
        assert_eq!(out, vec![9.0, 0.1, 0.2]);
    }

    #[test]
    fn zero_channels_produce_nothing() {
        assert!(remix(&[0.1, 0.2], 0, 2).is_empty());
        assert!(remix(&[0.1, 0.2], 2, 0).is_empty());
    }
}
