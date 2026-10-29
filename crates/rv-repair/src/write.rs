//! Writing the new header into the broken file (the original's `WriteHeader`), as a plan of byte
//! edits applied with a backup so that a failure leaves the file as it was.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};

use crate::Error;
use crate::moov::{build_moov, create_track};
use crate::reference::Reference;
use crate::scan::{Found, Mdat, read_at};

/// The 16-byte `ftyp` written in front of raw data (the original's constant).
const RAW_FTYP: [u8; 16] = *b"\0\0\0\x10ftypqt  \0\0\0\0";
/// `ftyp` + `mdat` header written over the start of a file without an `mdat`.
const RAW_HEADER: u64 = 24;

/// Byte edits that turn the broken file into a playable one.
#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub edits: Vec<(u64, Vec<u8>)>,
    pub new_len: u64,
    pub moov_pos: u64,
}

/// `WriteHeader` up to the point of writing: the new `moov` and where it goes.
///
/// Fewer than two video frames: nothing is written (the original reset its counters and the
/// dialog reports failure). Otherwise the `moov` goes into the space reserved between `ftyp` and
/// `mdat` when it fits with 8 bytes to spare (the rest becomes a `free` box and the file ends
/// with the media data), else right after the media data.
pub(crate) fn plan<R: Read + Seek>(
    reference: &Reference,
    mdat: &Mdat,
    found: &mut Found,
    file: &mut R,
) -> Result<Option<Plan>, Error> {
    let mut plan = Plan::default();
    if found.video.len() < 2 {
        return Ok(None);
    }
    let unsupported = |what: &str| Error::Unsupported(what.into());

    let data_end = found
        .video
        .iter()
        .chain(&found.audio)
        .map(|f| f.pos + u64::from(f.size))
        .max()
        .unwrap_or(0);
    // Media data must cover every sample (deviation: the original kept a declared size even when
    // frames were found past it, then wrote the header over them).
    let mdat_end = mdat.end.max(data_end);
    match mdat.start {
        None => {
            // No mdat: write ftyp + mdat header over the first 24 bytes. A frame starting there
            // is moved behind the header, cut short if it would overrun the next sample (as the
            // original, which also moved it over any audio that followed).
            let first = found.video[0];
            if first.pos < RAW_HEADER {
                let next = found
                    .video
                    .iter()
                    .skip(1)
                    .chain(&found.audio)
                    .map(|f| f.pos)
                    .min()
                    .unwrap_or(u64::MAX);
                let len = u64::from(first.size).min(next.saturating_sub(RAW_HEADER));
                if len == 0 {
                    found.video.remove(0);
                    if found.video.len() < 2 {
                        return Ok(None);
                    }
                } else {
                    let data = read_at(file, first.pos, len as usize).map_err(Error::io)?;
                    plan.edits.push((RAW_HEADER, data));
                    found.video[0].pos = RAW_HEADER;
                    found.video[0].size = len as u32;
                }
            }
            let size = u32::try_from(mdat_end - 16)
                .map_err(|_| unsupported("media data over 4 GiB without an mdat header"))?;
            let mut header = RAW_FTYP.to_vec();
            header.extend_from_slice(&size.to_be_bytes());
            header.extend_from_slice(b"mdat");
            plan.edits.push((0, header));
        }
        Some(start) if mdat.size_invalid || mdat_end != mdat.end => {
            let size = mdat_end - start;
            if mdat.header == 16 {
                plan.edits.push((start + 8, size.to_be_bytes().to_vec()));
            } else {
                let size = u32::try_from(size)
                    .map_err(|_| unsupported("media data over 4 GiB with a 32-bit mdat header"))?;
                plan.edits.push((start, size.to_be_bytes().to_vec()));
            }
        }
        Some(_) => {}
    }

    let video = create_track(&reference.tracks[reference.video], &found.video);
    let audio = reference
        .audio
        .and_then(|i| create_track(&reference.tracks[i], &found.audio));
    let tracks: Vec<_> = video.into_iter().chain(audio).collect();
    let moov = build_moov(reference, &tracks).map_err(|e| unsupported(&e.to_string()))?;

    let moov_len = moov.len() as u64;
    match mdat.free {
        Some((pos, size)) if moov_len + 8 <= size && size - moov_len <= u64::from(u32::MAX) => {
            let mut header = ((size - moov_len) as u32).to_be_bytes().to_vec();
            header.extend_from_slice(b"free");
            plan.moov_pos = pos;
            plan.edits.insert(0, (pos + moov_len, header));
            plan.edits.insert(0, (pos, moov));
            plan.new_len = mdat_end;
        }
        _ => {
            plan.moov_pos = mdat_end;
            plan.edits.insert(0, (mdat_end, moov));
            plan.new_len = mdat_end + moov_len;
        }
    }
    Ok(Some(plan))
}

/// File operations needed to apply a plan.
pub(crate) trait Storage: Read + Write + Seek {
    fn set_len(&mut self, len: u64) -> io::Result<()>;
    fn sync(&mut self) -> io::Result<()>;
}

impl Storage for File {
    fn set_len(&mut self, len: u64) -> io::Result<()> {
        File::set_len(self, len)
    }

    fn sync(&mut self) -> io::Result<()> {
        self.sync_all()
    }
}

/// Applies `plan` to a file of `old_len` bytes. On failure every overwritten byte and the old
/// length are restored before the error is returned.
pub(crate) fn apply<S: Storage>(s: &mut S, old_len: u64, plan: &Plan) -> io::Result<()> {
    let mut backup = Vec::new();
    for (pos, data) in &plan.edits {
        if *pos < old_len {
            let n = (old_len - pos).min(data.len() as u64) as usize;
            backup.push((*pos, read_at(s, *pos, n)?));
        }
    }
    if plan.new_len < old_len {
        backup.push((
            plan.new_len,
            read_at(s, plan.new_len, (old_len - plan.new_len) as usize)?,
        ));
    }
    let result = (|| {
        for (pos, data) in &plan.edits {
            s.seek(SeekFrom::Start(*pos))?;
            s.write_all(data)?;
        }
        s.set_len(plan.new_len)?;
        s.sync()
    })();
    if result.is_err() {
        // Back out (best effort: the original error is what gets reported).
        for (pos, data) in backup.iter().rev() {
            let _ = s
                .seek(SeekFrom::Start(*pos))
                .and_then(|_| s.write_all(data));
        }
        let _ = s.set_len(old_len);
        let _ = s.sync();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// In-memory file whose writes start failing after a number of successful ones.
    struct Flaky {
        data: Cursor<Vec<u8>>,
        writes_left: usize,
        failing: bool,
        fail_sync: bool,
    }

    impl Read for Flaky {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.data.read(buf)
        }
    }
    impl Seek for Flaky {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.data.seek(pos)
        }
    }
    impl Write for Flaky {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.failing {
                if self.writes_left == 0 {
                    self.failing = false; // only the first failure; the back-out must succeed
                    return Err(io::Error::other("disk full"));
                }
                self.writes_left -= 1;
            }
            self.data.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Storage for Flaky {
        fn set_len(&mut self, len: u64) -> io::Result<()> {
            self.data.get_mut().resize(len as usize, 0);
            Ok(())
        }
        fn sync(&mut self) -> io::Result<()> {
            if std::mem::take(&mut self.fail_sync) {
                return Err(io::Error::other("device gone"));
            }
            Ok(())
        }
    }

    fn flaky(data: &[u8], writes_left: usize, failing: bool, fail_sync: bool) -> Flaky {
        Flaky {
            data: Cursor::new(data.to_vec()),
            writes_left,
            failing,
            fail_sync,
        }
    }

    #[test]
    fn failed_write_is_backed_out() {
        let original: Vec<u8> = (0..200u8).collect();
        let plan = Plan {
            edits: vec![(150, vec![0xee; 100]), (10, vec![0xdd; 4])],
            new_len: 250,
            moov_pos: 150,
        };
        for writes_ok in 0..2 {
            let mut f = flaky(&original, writes_ok, true, false);
            assert!(apply(&mut f, 200, &plan).is_err());
            assert_eq!(f.data.get_ref(), &original);
        }
        let mut f = flaky(&original, 0, false, false);
        apply(&mut f, 200, &plan).unwrap();
        let d = f.data.get_ref();
        assert_eq!(
            (d.len(), d[10], d[149], d[150], d[249]),
            (250, 0xdd, 149, 0xee, 0xee)
        );

        // failing after the file was shortened: the cut tail is restored as well
        let shrink = Plan {
            edits: vec![(20, vec![1; 30])],
            new_len: 120,
            moov_pos: 20,
        };
        let mut f = flaky(&original, 0, false, true);
        assert!(apply(&mut f, 200, &shrink).is_err());
        assert_eq!(f.data.get_ref(), &original);
    }
}
