//! The original's `MOVCheckEncrypted` (UnitSourceMOV).

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// Whether a MOV/MP4 file is encrypted (SF-REPAIR-011): an `octs` box inside
/// `moov/trak/mdia/minf/stbl`, found before any `mdat`.
///
/// Walks the box tree from the start of the file, descending into `moov`, `trak`, `mdia`, `minf`
/// and `stbl` and skipping everything else. The walk stops (not encrypted) at `mdat`, at `udat`,
/// at a box shorter than 8 bytes (which includes 64-bit sizes) and at a box that runs past the end
/// of the file — so a camera-layout file (`mdat` before `moov`) never reports encryption, exactly
/// as in the original.
///
/// The original treated any error as "not encrypted"; here opening and reading errors are returned
/// so the caller can decide (mapping them to `false` reproduces the original).
pub fn is_encrypted(path: &Path) -> io::Result<bool> {
    let mut f = File::open(path)?;
    let size = f.metadata()?.len();
    let mut pos = 0u64;
    while pos < size {
        f.seek(SeekFrom::Start(pos))?;
        let mut h = [0u8; 8];
        match f.read_exact(&mut h) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
        let len = u64::from(u32::from_be_bytes(h[..4].try_into().unwrap()));
        let kind = &h[4..];
        if len < 8 || pos + len > size || kind == b"mdat" || kind == b"udat" {
            break;
        }
        match kind {
            b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl" => pos += 8,
            b"octs" => return Ok(true),
            _ => pos += len,
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rv_core::testutil::mk;

    fn check(name: &str, data: &[u8]) -> bool {
        let path =
            std::env::temp_dir().join(format!("rv-repair-enc-{}-{name}", std::process::id()));
        std::fs::write(&path, data).unwrap();
        let r = is_encrypted(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        r
    }

    #[test]
    fn finds_octs_in_sample_table() {
        let stbl = mk(
            b"stbl",
            &[mk(b"stsd", &[0; 8]), mk(b"octs", &[1, 2, 3])].concat(),
        );
        let trak = mk(
            b"trak",
            &[mk(b"tkhd", &[0; 84]), mk(b"mdia", &mk(b"minf", &stbl))].concat(),
        );
        let moov = mk(b"moov", &[mk(b"mvhd", &[0; 100]), trak].concat());
        let ftyp = mk(b"ftyp", b"qt  \0\0\0\0");
        assert!(check(
            "enc",
            &[ftyp.clone(), moov.clone(), mk(b"mdat", &[0; 16])].concat()
        ));
        // camera layout: the walk stops at mdat
        assert!(!check(
            "cam",
            &[ftyp.clone(), mk(b"mdat", &[0; 16]), moov.clone()].concat()
        ));
        // a truncated moov ends the walk
        let mut cut = [ftyp.clone(), moov].concat();
        cut.truncate(cut.len() - 4);
        assert!(!check("cut", &cut));
        let plain = mk(b"moov", &mk(b"trak", &mk(b"mdia", &[0; 8])));
        assert!(!check("plain", &[ftyp, plain].concat()));
        assert!(is_encrypted(Path::new("/definitely/not/here.mov")).is_err());
    }
}
