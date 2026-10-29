//! Box (atom) helpers over in-memory byte slices: big-endian readers, a child iterator and a
//! writer for nested boxes.

use std::io;

pub(crate) type FourCC = [u8; 4];

pub(crate) fn be16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

pub(crate) fn be32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

pub(crate) fn be64(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(d.get(at..at + 8)?.try_into().ok()?))
}

/// One box inside a parent body.
#[derive(Clone, Copy)]
pub(crate) struct Atom<'a> {
    pub kind: FourCC,
    pub body: &'a [u8],
}

/// Iterator over the boxes of a container body. Stops at the first header that is malformed or
/// overruns the parent (the original's `CopyHeaderFrom` does the same). Size 0 extends to the end
/// of the parent; size 1 has a 64-bit size after the type.
pub(crate) struct Atoms<'a> {
    d: &'a [u8],
    pos: usize,
}

pub(crate) fn atoms(d: &[u8]) -> Atoms<'_> {
    Atoms { d, pos: 0 }
}

impl<'a> Iterator for Atoms<'a> {
    type Item = Atom<'a>;

    fn next(&mut self) -> Option<Atom<'a>> {
        let rest = self.d.get(self.pos..)?;
        let size32 = be32(rest, 0)?;
        let kind: FourCC = rest.get(4..8)?.try_into().ok()?;
        let (size, header) = match size32 {
            0 => (rest.len() as u64, 8),
            1 => (be64(rest, 8)?, 16),
            n => (u64::from(n), 8),
        };
        if size < header as u64 || size > rest.len() as u64 {
            self.pos = self.d.len();
            return None;
        }
        let size = size as usize;
        self.pos += size;
        Some(Atom {
            kind,
            body: &rest[header..size],
        })
    }
}

/// First child of `kind`.
pub(crate) fn child<'a>(d: &'a [u8], kind: &FourCC) -> Option<&'a [u8]> {
    atoms(d).find(|a| &a.kind == kind).map(|a| a.body)
}

/// Descends through `path`, taking the first match at each level.
pub(crate) fn path<'a>(d: &'a [u8], path: &[&FourCC]) -> Option<&'a [u8]> {
    path.iter().try_fold(d, |d, kind| child(d, kind))
}

/// Builds nested boxes into a byte buffer; sizes are patched when a box is closed.
#[derive(Default)]
pub(crate) struct BoxWriter {
    pub buf: Vec<u8>,
    open: Vec<usize>,
}

impl BoxWriter {
    pub fn open(&mut self, kind: &FourCC) {
        self.open.push(self.buf.len());
        self.buf.extend_from_slice(&[0; 4]);
        self.buf.extend_from_slice(kind);
    }

    /// Opens a full box (version + 24-bit flags).
    pub fn open_full(&mut self, kind: &FourCC, version: u8, flags: u32) {
        self.open(kind);
        self.u32((u32::from(version) << 24) | (flags & 0x00ff_ffff));
    }

    pub fn close(&mut self) -> io::Result<()> {
        let start = self.open.pop().expect("close without open");
        let size = u32::try_from(self.buf.len() - start)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "box larger than 4 GiB"))?;
        self.buf[start..start + 4].copy_from_slice(&size.to_be_bytes());
        Ok(())
    }

    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }

    /// Whole box: header + `body`.
    pub fn atom(&mut self, kind: &FourCC, body: &[u8]) -> io::Result<()> {
        self.open(kind);
        self.bytes(body);
        self.close()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterates_and_stops_at_malformed_boxes() {
        let mut w = BoxWriter::default();
        w.atom(b"aaaa", &[1, 2]).unwrap();
        w.open(b"bbbb");
        w.atom(b"cccc", &[3]).unwrap();
        w.close().unwrap();
        let mut d = w.buf.clone();
        d.extend_from_slice(&[0, 0, 0, 99, b'x', b'x', b'x', b'x']); // overruns
        let kinds: Vec<FourCC> = atoms(&d).map(|a| a.kind).collect();
        assert_eq!(kinds, [*b"aaaa", *b"bbbb"]);
        assert_eq!(path(&d, &[b"bbbb", b"cccc"]), Some(&[3u8][..]));
        // 64-bit size and size 0 (to the end)
        let mut big = vec![0, 0, 0, 1];
        big.extend_from_slice(b"dddd");
        big.extend_from_slice(&17u64.to_be_bytes());
        big.push(7);
        big.extend_from_slice(&[0, 0, 0, 0, b'e', b'e', b'e', b'e', 8, 9]);
        let all: Vec<(FourCC, Vec<u8>)> = atoms(&big).map(|a| (a.kind, a.body.to_vec())).collect();
        assert_eq!(all, [(*b"dddd", vec![7]), (*b"eeee", vec![8, 9])]);
    }
}
