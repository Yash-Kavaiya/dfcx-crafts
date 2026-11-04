//! ZIP archives: a DEFLATE (RFC 1951) decoder, CRC-32, a ZIP reader and a
//! writer (stored entries). Enough to read Conversational Agents agent
//! exports (`.zip` / `.blob`) and to write JSON-package exports.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct ZipError(pub String);

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "zip: {}", self.0)
    }
}
impl std::error::Error for ZipError {}

fn err<T>(m: &str) -> Result<T, ZipError> {
    Err(ZipError(m.to_string()))
}

// ---- CRC-32 -------------------------------------------------------------------

fn crc_table() -> &'static [u32; 256] {
    static T: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, slot) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *slot = c;
        }
        t
    })
}

pub fn crc32(data: &[u8]) -> u32 {
    let t = crc_table();
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = t[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

// ---- inflate ------------------------------------------------------------------

const MAXBITS: usize = 15;

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u32,
    cnt: u32,
}

impl<'a> Bits<'a> {
    fn bits(&mut self, need: u32) -> Result<u32, ZipError> {
        let mut val = self.buf as u64;
        let mut cnt = self.cnt;
        while cnt < need {
            if self.pos >= self.data.len() {
                return err("unexpected end of compressed data");
            }
            val |= (self.data[self.pos] as u64) << cnt;
            self.pos += 1;
            cnt += 8;
        }
        self.buf = (val >> need) as u32;
        self.cnt = cnt - need;
        Ok((val & ((1u64 << need) - 1)) as u32)
    }
    fn align(&mut self) {
        self.buf = 0;
        self.cnt = 0;
    }
}

struct Huffman {
    count: [i32; MAXBITS + 1],
    symbol: Vec<i32>,
}

impl Huffman {
    /// Builds a canonical Huffman decoder from code lengths. Returns the
    /// table and whether the code is incomplete (allowed for distances).
    fn new(lengths: &[u8]) -> Result<Huffman, ZipError> {
        let mut count = [0i32; MAXBITS + 1];
        for &l in lengths {
            count[l as usize] += 1;
        }
        if count[0] as usize == lengths.len() {
            return Ok(Huffman { count, symbol: vec![] });
        }
        let mut left = 1i32;
        for c in count.iter().skip(1) {
            left <<= 1;
            left -= c;
            if left < 0 {
                return err("over-subscribed Huffman code");
            }
        }
        let mut offs = [0i32; MAXBITS + 1];
        for len in 1..MAXBITS {
            offs[len + 1] = offs[len] + count[len];
        }
        let mut symbol = vec![0i32; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbol[offs[l as usize] as usize] = sym as i32;
                offs[l as usize] += 1;
            }
        }
        Ok(Huffman { count, symbol })
    }

    fn decode(&self, s: &mut Bits) -> Result<i32, ZipError> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..=MAXBITS {
            code |= s.bits(1)? as i32;
            let count = self.count[len];
            if code - count < first {
                return Ok(self.symbol[(index + (code - first)) as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        err("invalid Huffman code")
    }
}

const LBASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEXT: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DBASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DEXT: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn codes(s: &mut Bits, out: &mut Vec<u8>, lencode: &Huffman, distcode: &Huffman, limit: usize) -> Result<(), ZipError> {
    loop {
        let sym = lencode.decode(s)?;
        if sym < 256 {
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let sym = (sym - 257) as usize;
            if sym >= 29 {
                return err("invalid length symbol");
            }
            let len = LBASE[sym] as usize + s.bits(LEXT[sym] as u32)? as usize;
            let dsym = distcode.decode(s)? as usize;
            if dsym >= 30 {
                return err("invalid distance symbol");
            }
            let dist = DBASE[dsym] as usize + s.bits(DEXT[dsym] as u32)? as usize;
            if dist > out.len() {
                return err("distance too far back");
            }
            let start = out.len() - dist;
            for i in 0..len {
                let b = out[start + i];
                out.push(b);
            }
        }
        if out.len() > limit {
            return err("decompressed data exceeds limit");
        }
    }
}

/// Decompresses a raw DEFLATE stream. `limit` bounds the output size.
pub fn inflate(data: &[u8], limit: usize) -> Result<Vec<u8>, ZipError> {
    let mut s = Bits { data, pos: 0, buf: 0, cnt: 0 };
    let mut out = Vec::new();
    loop {
        let last = s.bits(1)?;
        match s.bits(2)? {
            0 => {
                s.align();
                if s.pos + 4 > data.len() {
                    return err("truncated stored block");
                }
                let len = u16::from_le_bytes([data[s.pos], data[s.pos + 1]]) as usize;
                let nlen = u16::from_le_bytes([data[s.pos + 2], data[s.pos + 3]]) as usize;
                if len != (!nlen & 0xFFFF) {
                    return err("stored block length mismatch");
                }
                s.pos += 4;
                if s.pos + len > data.len() {
                    return err("truncated stored block");
                }
                out.extend_from_slice(&data[s.pos..s.pos + len]);
                s.pos += len;
            }
            1 => {
                let mut lengths = [0u8; 288];
                for (i, l) in lengths.iter_mut().enumerate() {
                    *l = match i {
                        0..=143 => 8,
                        144..=255 => 9,
                        256..=279 => 7,
                        _ => 8,
                    };
                }
                let lencode = Huffman::new(&lengths)?;
                let distcode = Huffman::new(&[5u8; 30])?;
                codes(&mut s, &mut out, &lencode, &distcode, limit)?;
            }
            2 => {
                let nlen = s.bits(5)? as usize + 257;
                let ndist = s.bits(5)? as usize + 1;
                let ncode = s.bits(4)? as usize + 4;
                if nlen > 286 || ndist > 30 {
                    return err("bad dynamic block counts");
                }
                const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
                let mut cl = [0u8; 19];
                for &o in ORDER.iter().take(ncode) {
                    cl[o] = s.bits(3)? as u8;
                }
                let clcode = Huffman::new(&cl)?;
                let mut lengths = vec![0u8; nlen + ndist];
                let mut i = 0;
                while i < nlen + ndist {
                    let sym = clcode.decode(&mut s)?;
                    if sym < 16 {
                        lengths[i] = sym as u8;
                        i += 1;
                    } else {
                        let (val, rep) = match sym {
                            16 => {
                                if i == 0 {
                                    return err("repeat with no previous length");
                                }
                                (lengths[i - 1], 3 + s.bits(2)? as usize)
                            }
                            17 => (0, 3 + s.bits(3)? as usize),
                            _ => (0, 11 + s.bits(7)? as usize),
                        };
                        if i + rep > nlen + ndist {
                            return err("too many code lengths");
                        }
                        for _ in 0..rep {
                            lengths[i] = val;
                            i += 1;
                        }
                    }
                }
                if lengths[256] == 0 {
                    return err("missing end-of-block code");
                }
                let lencode = Huffman::new(&lengths[..nlen])?;
                let distcode = Huffman::new(&lengths[nlen..])?;
                codes(&mut s, &mut out, &lencode, &distcode, limit)?;
            }
            _ => return err("invalid block type"),
        }
        if out.len() > limit {
            return err("decompressed data exceeds limit");
        }
        if last == 1 {
            return Ok(out);
        }
    }
}

// ---- ZIP container ------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
}

fn u16_at(b: &[u8], i: usize) -> Result<u16, ZipError> {
    b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| ZipError("truncated archive".into()))
}
fn u32_at(b: &[u8], i: usize) -> Result<u32, ZipError> {
    b.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| ZipError("truncated archive".into()))
}

pub fn is_zip(b: &[u8]) -> bool {
    b.len() >= 4 && b[..4] == [0x50, 0x4b, 0x03, 0x04]
}

/// Reads every file entry of a ZIP archive (directories are skipped).
/// `limit` bounds the total uncompressed size.
pub fn read(bytes: &[u8], limit: usize) -> Result<Vec<Entry>, ZipError> {
    // End of central directory: search backwards (comment up to 64 KiB).
    let min = bytes.len().saturating_sub(22 + 65_535);
    let mut eocd = None;
    let mut i = bytes.len().saturating_sub(22);
    loop {
        if u32_at(bytes, i).ok() == Some(0x0605_4b50) {
            eocd = Some(i);
            break;
        }
        if i == min || i == 0 {
            break;
        }
        i -= 1;
    }
    let eocd = eocd.ok_or_else(|| ZipError("not a zip archive (no end of central directory)".into()))?;
    let total = u16_at(bytes, eocd + 10)? as usize;
    let mut p = u32_at(bytes, eocd + 16)? as usize;
    let mut out = Vec::new();
    let mut used = 0usize;
    for _ in 0..total {
        if u32_at(bytes, p)? != 0x0201_4b50 {
            return err("bad central directory entry");
        }
        let flags = u16_at(bytes, p + 8)?;
        let method = u16_at(bytes, p + 10)?;
        let crc = u32_at(bytes, p + 16)?;
        let csize = u32_at(bytes, p + 20)? as usize;
        let usize_ = u32_at(bytes, p + 24)? as usize;
        let nlen = u16_at(bytes, p + 28)? as usize;
        let xlen = u16_at(bytes, p + 30)? as usize;
        let clen = u16_at(bytes, p + 32)? as usize;
        let local = u32_at(bytes, p + 42)? as usize;
        let name_bytes = bytes.get(p + 46..p + 46 + nlen).ok_or_else(|| ZipError("truncated name".into()))?;
        let name = String::from_utf8_lossy(name_bytes).replace('\\', "/");
        p += 46 + nlen + xlen + clen;
        if flags & 1 != 0 {
            return err("encrypted entries are not supported");
        }
        if name.ends_with('/') {
            continue;
        }
        if u32_at(bytes, local)? != 0x0403_4b50 {
            return err("bad local header");
        }
        let lnlen = u16_at(bytes, local + 26)? as usize;
        let lxlen = u16_at(bytes, local + 28)? as usize;
        let start = local + 30 + lnlen + lxlen;
        let raw = bytes.get(start..start + csize).ok_or_else(|| ZipError("truncated entry data".into()))?;
        used += usize_;
        if used > limit {
            return err("archive exceeds size limit");
        }
        let data = match method {
            0 => raw.to_vec(),
            8 => inflate(raw, usize_.max(1) + 1024)?,
            m => return Err(ZipError(format!("unsupported compression method {} for {}", m, name))),
        };
        if crc32(&data) != crc {
            return Err(ZipError(format!("CRC mismatch for {}", name)));
        }
        out.push(Entry { name, data });
    }
    Ok(out)
}

/// Writes a ZIP archive with stored (uncompressed) entries.
pub fn write(entries: &[Entry]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for e in entries {
        let crc = crc32(&e.data);
        let offset = out.len() as u32;
        let name = e.name.as_bytes();
        let header = |sig: u32, central: bool| {
            let mut h = Vec::new();
            h.extend_from_slice(&sig.to_le_bytes());
            if central {
                h.extend_from_slice(&20u16.to_le_bytes()); // version made by
            }
            h.extend_from_slice(&20u16.to_le_bytes()); // version needed
            h.extend_from_slice(&0x0800u16.to_le_bytes()); // flags: UTF-8 names
            h.extend_from_slice(&0u16.to_le_bytes()); // method: stored
            h.extend_from_slice(&0u16.to_le_bytes()); // time
            h.extend_from_slice(&0x21u16.to_le_bytes()); // date (1980-01-01)
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&(e.data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(e.data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes()); // extra
            if central {
                h.extend_from_slice(&0u16.to_le_bytes()); // comment
                h.extend_from_slice(&0u16.to_le_bytes()); // disk
                h.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
                h.extend_from_slice(&0u32.to_le_bytes()); // external attrs
                h.extend_from_slice(&offset.to_le_bytes());
            }
            h.extend_from_slice(name);
            h
        };
        out.extend(header(0x0403_4b50, false));
        out.extend_from_slice(&e.data);
        central.extend(header(0x0201_4b50, true));
    }
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn stored_roundtrip() {
        let e = vec![Entry { name: "a/agent.json".into(), data: b"{\"x\":1}".to_vec() }, Entry { name: "b.txt".into(), data: vec![] }];
        let z = write(&e);
        assert!(is_zip(&z));
        assert_eq!(read(&z, 1 << 20).unwrap(), e);
    }

    #[test]
    fn inflate_fixed_dynamic_and_stored() {
        // vectors produced by zlib (raw deflate, level 9)
        let fixed = [203, 72, 205, 201, 201, 87, 200, 64, 39, 185, 0];
        assert_eq!(inflate(&fixed, 1000).unwrap(), b"hello hello hello hello\n");
        let stored = [0x01, 0x03, 0x00, 0xfc, 0xff, b'a', b'b', b'c'];
        assert_eq!(inflate(&stored, 1000).unwrap(), b"abc");
        // dynamic Huffman block: 3000 pseudo-random skewed letters
        let alpha = b"eeeeeeetttaaoinshrdlu  \n";
        let mut x: u64 = 12345;
        let mut expected = Vec::new();
        for _ in 0..3000 {
            x = (x * 1103515245 + 12345) & 0x7fff_ffff;
            expected.push(alpha[((x >> 16) as usize) % alpha.len()]);
        }
        let dynamic = include_bytes!("testdata_dyn.bin");
        assert_eq!((dynamic[0] >> 1) & 3, 2, "fixture uses a dynamic block");
        assert_eq!(inflate(dynamic, 10_000).unwrap(), expected);
        assert!(inflate(&[0x07], 10).is_err());
        assert!(inflate(dynamic, 100).is_err(), "limit enforced");
    }

    #[test]
    fn reads_python_zipfile_archive() {
        let z = include_bytes!("testdata_agent.zip");
        let entries = read(z, 1 << 20).unwrap();
        assert_eq!(entries.len(), 4);
        let tp = entries.iter().find(|e| e.name.ends_with("trainingPhrases/en.json")).unwrap();
        assert!(String::from_utf8_lossy(&tp.data).contains("hello there friend"));
    }
}
