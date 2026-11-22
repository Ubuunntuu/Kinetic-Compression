pub const MIN_CDC_CHUNK: usize = 64 * 1024;
pub const AVG_CDC_CHUNK: usize = 256 * 1024;
pub const MAX_CDC_CHUNK: usize = 1024 * 1024;

const fn build_gear_table() -> [u64; 256] {
    let mut table = [0u64; 256];
    let mut i = 0;
    let mut state = 0x853C_49E6_748F_EA9Bu64;
    while i < 256 {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        table[i] = state.wrapping_mul(0x2545_F491_4F6C_DD1Du64);
        i += 1;
    }
    table
}

const GEAR_TABLE: [u64; 256] = build_gear_table();
const FASTCDC_MASK: u64 = 0x0003_FFFF_0000_0000;

pub fn chunk_slice_fastcdc(data: &[u8], enable_dedup: bool) -> Vec<(usize, usize)> {
    if data.is_empty() {
        return Vec::new();
    }
    if !enable_dedup || data.len() <= MIN_CDC_CHUNK {
        return vec![(0, data.len())];
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;
    let n = data.len();

    while start < n {
        let remaining = n - start;
        if remaining <= MIN_CDC_CHUNK {
            chunks.push((start, remaining));
            break;
        }

        let max_end = (start + MAX_CDC_CHUNK).min(n);
        let mut pos = start + MIN_CDC_CHUNK;
        let mut hash = 0u64;
        let mut cut = max_end;

        while pos < max_end {
            hash = (hash << 1).wrapping_add(GEAR_TABLE[data[pos] as usize]);
            if (hash & FASTCDC_MASK) == 0 {
                cut = pos;
                break;
            }
            pos += 1;
        }

        chunks.push((start, cut - start));
        start = cut;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fastcdc_chunking_covers_exact_stream() {
        let data: Vec<u8> = (0..1_500_000).map(|i| ((i * 73 + 19) & 0xFF) as u8).collect();
        let chunks = chunk_slice_fastcdc(&data, true);
        assert!(chunks.len() >= 2);

        let total_len: usize = chunks.iter().map(|(_, len)| *len).sum();
        assert_eq!(total_len, data.len());
    }
}