pub const FILTER_NONE: u8 = 0x00;
pub const FILTER_X86_BCJ: u8 = 0x01;
pub const FILTER_BYTE_DELTA: u8 = 0x04;
pub const FILTER_BYTE_SHUFFLE: u8 = 0x08;

pub fn select_optimal_filter(file_path: &str, data: &[u8]) -> u8 {
    let lower = file_path.to_ascii_lowercase();
    if (data.len() >= 2 && &data[0..2] == b"MZ")
        || lower.ends_with(".exe")
        || lower.ends_with(".dll")
    {
        return FILTER_X86_BCJ;
    }
    if lower.ends_with(".wav") || lower.ends_with(".bmp") || lower.ends_with(".raw") {
        return FILTER_BYTE_DELTA;
    }
    if (lower.ends_with(".bin") || lower.ends_with(".dat")) && data.len() >= 64 {
        return FILTER_BYTE_SHUFFLE;
    }
    FILTER_NONE
}

pub fn apply_filter(filter_mask: u8, data: &[u8]) -> Vec<u8> {
    match filter_mask {
        FILTER_X86_BCJ => apply_x86_bcj(data),
        FILTER_BYTE_DELTA => apply_byte_delta(data),
        FILTER_BYTE_SHUFFLE => apply_byte_shuffle_4(data),
        _ => data.to_vec(),
    }
}

pub fn reverse_filter(filter_mask: u8, data: &[u8]) -> Vec<u8> {
    match filter_mask {
        FILTER_X86_BCJ => reverse_x86_bcj(data),
        FILTER_BYTE_DELTA => reverse_byte_delta(data),
        FILTER_BYTE_SHUFFLE => reverse_byte_shuffle_4(data),
        _ => data.to_vec(),
    }
}

fn apply_byte_delta(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut prev = 0u8;
    for &b in data {
        out.push(b.wrapping_sub(prev));
        prev = b;
    }
    out
}

fn reverse_byte_delta(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut prev = 0u8;
    for &d in data {
        let orig = d.wrapping_add(prev);
        out.push(orig);
        prev = orig;
    }
    out
}

fn apply_byte_shuffle_4(data: &[u8]) -> Vec<u8> {
    let n = data.len();
    let groups = n / 4;
    if groups == 0 {
        return data.to_vec();
    }
    let mut out = vec![0u8; n];
    for i in 0..groups {
        out[i] = data[i * 4];
        out[groups + i] = data[i * 4 + 1];
        out[groups * 2 + i] = data[i * 4 + 2];
        out[groups * 3 + i] = data[i * 4 + 3];
    }
    let rem_start = groups * 4;
    out[rem_start..].copy_from_slice(&data[rem_start..]);
    out
}

fn reverse_byte_shuffle_4(data: &[u8]) -> Vec<u8> {
    let n = data.len();
    let groups = n / 4;
    if groups == 0 {
        return data.to_vec();
    }
    let mut out = vec![0u8; n];
    for i in 0..groups {
        out[i * 4] = data[i];
        out[i * 4 + 1] = data[groups + i];
        out[i * 4 + 2] = data[groups * 2 + i];
        out[i * 4 + 3] = data[groups * 3 + i];
    }
    let rem_start = groups * 4;
    out[rem_start..].copy_from_slice(&data[rem_start..]);
    out
}

fn apply_x86_bcj(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    if out.len() < 5 {
        return out;
    }
    let mut i = 0usize;
    let limit = out.len() - 4;
    while i < limit {
        if out[i] == 0xE8 || out[i] == 0xE9 {
            let msb = out[i + 4];
            if msb == 0x00 || msb == 0xFF {
                let rel = i32::from_le_bytes([out[i + 1], out[i + 2], out[i + 3], out[i + 4]]);
                let abs = rel.wrapping_add((i + 5) as i32);
                let bytes = abs.to_le_bytes();
                out[i + 1] = bytes[0];
                out[i + 2] = bytes[1];
                out[i + 3] = bytes[2];
                out[i + 4] = bytes[3];
                i += 5;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn reverse_x86_bcj(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    if out.len() < 5 {
        return out;
    }
    let mut i = 0usize;
    let limit = out.len() - 4;
    while i < limit {
        if out[i] == 0xE8 || out[i] == 0xE9 {
            let abs = i32::from_le_bytes([out[i + 1], out[i + 2], out[i + 3], out[i + 4]]);
            let rel = abs.wrapping_sub((i + 5) as i32);
            let rel_bytes = rel.to_le_bytes();
            if rel_bytes[3] == 0x00 || rel_bytes[3] == 0xFF {
                out[i + 1] = rel_bytes[0];
                out[i + 2] = rel_bytes[1];
                out[i + 3] = rel_bytes[2];
                out[i + 4] = rel_bytes[3];
                i += 5;
                continue;
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filters_lossless_roundtrip() {
        let sample: Vec<u8> = (0..1025).map(|i| ((i * 37) & 0xFF) as u8).collect();

        for filter in [FILTER_BYTE_DELTA, FILTER_BYTE_SHUFFLE, FILTER_X86_BCJ] {
            let filtered = apply_filter(filter, &sample);
            let restored = reverse_filter(filter, &filtered);
            assert_eq!(restored, sample);
        }
    }
}