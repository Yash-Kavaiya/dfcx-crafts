//! DEFLATE compression (RFC 1951) and gzip framing (RFC 1952).
//!
//! LZ77 with hash chains plus fixed Huffman codes: simple, fast, and
//! typically 3-5x smaller for JSON / JavaScript / HTML, which is what the
//! server sends.

const WINDOW: usize = 32 * 1024;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const HASH_BITS: usize = 15;
const MAX_CHAIN: usize = 48;

struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, bits: u32, count: u32) {
        self.acc |= (bits as u64) << self.n;
        self.n += count;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
    /// Huffman codes are sent most-significant bit first.
    fn put_rev(&mut self, code: u32, len: u32) {
        let mut r = 0;
        for i in 0..len {
            r |= ((code >> i) & 1) << (len - 1 - i);
        }
        self.put(r, len);
    }
    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

fn lit(w: &mut BitWriter, v: u32) {
    match v {
        0..=143 => w.put_rev(0x30 + v, 8),
        144..=255 => w.put_rev(0x190 + v - 144, 9),
        256..=279 => w.put_rev(v - 256, 7),
        _ => w.put_rev(0xC0 + v - 280, 8),
    }
}

const LBASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEXT: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DBASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DEXT: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn emit_match(w: &mut BitWriter, len: usize, dist: usize) {
    let li = LBASE.iter().rposition(|&b| b as usize <= len).unwrap();
    lit(w, 257 + li as u32);
    if LEXT[li] > 0 {
        w.put((len - LBASE[li] as usize) as u32, LEXT[li] as u32);
    }
    let di = DBASE.iter().rposition(|&b| b as usize <= dist).unwrap();
    w.put_rev(di as u32, 5);
    if DEXT[di] > 0 {
        w.put((dist - DBASE[di] as usize) as u32, DEXT[di] as u32);
    }
}

fn hash(d: &[u8], i: usize) -> usize {
    let v = (d[i] as u32) << 16 | (d[i + 1] as u32) << 8 | d[i + 2] as u32;
    (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
}

/// Compresses `data` into a raw DEFLATE stream (one fixed-Huffman block).
pub fn deflate(data: &[u8]) -> Vec<u8> {
    let mut w = BitWriter { out: Vec::with_capacity(data.len() / 3 + 16), acc: 0, n: 0 };
    w.put(1, 1); // BFINAL
    w.put(1, 2); // BTYPE = fixed Huffman
    let mut head = vec![usize::MAX; 1 << HASH_BITS];
    let mut prev = vec![usize::MAX; WINDOW];
    let n = data.len();
    let mut i = 0;
    let insert = |head: &mut Vec<usize>, prev: &mut Vec<usize>, i: usize| {
        if i + MIN_MATCH <= n {
            let h = hash(data, i);
            prev[i % WINDOW] = head[h];
            head[h] = i;
        }
    };
    while i < n {
        let mut best_len = 0;
        let mut best_dist = 0;
        if i + MIN_MATCH <= n {
            let mut cand = head[hash(data, i)];
            let mut chain = 0;
            let max = (n - i).min(MAX_MATCH);
            while cand != usize::MAX && i - cand <= WINDOW && chain < MAX_CHAIN {
                if data[cand + best_len.min(max - 1)] == data[i + best_len.min(max - 1)] {
                    let mut l = 0;
                    while l < max && data[cand + l] == data[i + l] {
                        l += 1;
                    }
                    if l > best_len {
                        best_len = l;
                        best_dist = i - cand;
                        if l == max {
                            break;
                        }
                    }
                }
                let p = prev[cand % WINDOW];
                if p == usize::MAX || p >= cand {
                    break;
                }
                cand = p;
                chain += 1;
            }
        }
        if best_len >= MIN_MATCH {
            emit_match(&mut w, best_len, best_dist);
            for k in i..i + best_len {
                insert(&mut head, &mut prev, k);
            }
            i += best_len;
        } else {
            lit(&mut w, data[i] as u32);
            insert(&mut head, &mut prev, i);
            i += 1;
        }
    }
    lit(&mut w, 256); // end of block
    w.finish()
}

/// gzip container around [`deflate`].
pub fn gzip(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255];
    out.extend(deflate(data));
    out.extend_from_slice(&crate::zip::crc32(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_through_inflate() {
        let samples: Vec<Vec<u8>> = vec![
            b"".to_vec(),
            b"a".to_vec(),
            b"hello hello hello hello\n".to_vec(),
            br#"{"intents":[{"name":"projects/p/locations/global/agents/a/intents/1","displayName":"order"}]}"#.repeat(200),
            (0..70_000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect(),
            vec![7u8; 100_000],
        ];
        for s in samples {
            let z = deflate(&s);
            assert_eq!(crate::zip::inflate(&z, s.len() + 16).unwrap(), s);
        }
    }

    #[test]
    fn compresses_text_well_and_gzip_frames() {
        let js = br#"function h(tag, attrs, ...kids) { const el = document.createElement(tag); return el; } "#.repeat(300);
        let z = gzip(&js);
        assert!(z.len() * 5 < js.len(), "{} vs {}", z.len(), js.len());
        assert_eq!(&z[..3], &[0x1f, 0x8b, 8]);
        assert_eq!(crate::zip::inflate(&z[10..z.len() - 8], js.len()).unwrap(), js);
    }
}
