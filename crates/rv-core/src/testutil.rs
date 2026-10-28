//! Builders for small MP4/MOV files used by tests in this and other crates
//! (enabled by the `testutil` feature).

use std::io;
use std::path::Path;

/// Builds a box: size + type + body.
pub fn mk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    v.extend_from_slice(kind);
    v.extend_from_slice(body);
    v
}

pub fn be(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_be_bytes()).collect()
}

/// A one-track movie whose samples (given as payloads) live in an `mdat` after `moov`.
pub fn movie_with_track(
    handler: &[u8; 4],
    codec: &[u8; 4],
    timescale: u32,
    payloads: &[Vec<u8>],
) -> Vec<u8> {
    let build = |mdat_body_offset: u32| {
        let mut stsd = be(&[0, 1, 16]);
        stsd.extend_from_slice(codec);
        stsd.extend_from_slice(&[0; 8]);
        let stbl = [
            mk(b"stsd", &stsd),
            mk(b"stts", &be(&[0, 1, payloads.len() as u32, timescale])),
            mk(b"stsc", &be(&[0, 1, 1, payloads.len() as u32, 1])),
            mk(
                b"stsz",
                &[
                    be(&[0, 0, payloads.len() as u32]),
                    be(&payloads.iter().map(|p| p.len() as u32).collect::<Vec<_>>()),
                ]
                .concat(),
            ),
            mk(b"stco", &be(&[0, 1, mdat_body_offset])),
        ]
        .concat();
        let mut hdlr = be(&[0, 0]);
        hdlr.extend_from_slice(handler);
        hdlr.extend_from_slice(&[0; 12]);
        let mdhd = be(&[0, 0, 0, timescale, timescale * payloads.len() as u32, 0]);
        let mdia = mk(
            b"mdia",
            &[
                mk(b"mdhd", &mdhd),
                mk(b"hdlr", &hdlr),
                mk(b"minf", &mk(b"stbl", &stbl)),
            ]
            .concat(),
        );
        let mut tkhd = vec![0u8; 84];
        tkhd[12..16].copy_from_slice(&1u32.to_be_bytes());
        tkhd[76..80].copy_from_slice(&(1920u32 << 16).to_be_bytes());
        tkhd[80..84].copy_from_slice(&(1080u32 << 16).to_be_bytes());
        let trak = mk(b"trak", &[mk(b"tkhd", &tkhd), mdia].concat());
        let mvhd = [
            be(&[0, 3_000_000_000, 0, 1000, 1000 * payloads.len() as u32]),
            vec![0; 80],
        ]
        .concat();
        let udta = mk(b"udta", &mk(b"\xa9inf", b"AMBARELLA A7L\0\0"));
        let moov = mk(b"moov", &[mk(b"mvhd", &mvhd), udta, trak].concat());
        [mk(b"ftyp", b"qt  \0\0\0\0qt  "), moov].concat()
    };
    let head_len = build(0).len() as u32;
    let mut file = build(head_len + 8);
    file.extend(mk(b"mdat", &payloads.concat()));
    file
}

/// One RVMI GPS record: (seconds offset is the sample index), lat, lon, speed km/h, course degrees.
pub struct GpsFix {
    pub lat: f64,
    pub lon: f64,
    pub speed_kmh: f64,
    pub course: f64,
}

/// A movie with an RVMI track: a start-time record followed by one GPS record per second.
pub fn rvmi_movie(start_tdatetime: f64, fixes: &[GpsFix]) -> Vec<u8> {
    let mut payloads = Vec::with_capacity(fixes.len() + 1);
    let mut start = b"tReV".to_vec();
    start.extend_from_slice(&start_tdatetime.to_le_bytes());
    payloads.push(start);
    for f in fixes {
        let mut rec = b"gReV".to_vec();
        rec.extend_from_slice(&((f.lon * 1e6).round() as i32).to_le_bytes());
        rec.extend_from_slice(&((f.lat * 1e6).round() as i32).to_le_bytes());
        rec.extend_from_slice(&0i32.to_le_bytes());
        rec.extend_from_slice(&((f.speed_kmh * 10.0).round() as u16).to_le_bytes());
        rec.extend_from_slice(&[(f.course / 2.0).round() as u8, 0]);
        payloads.push(rec);
    }
    movie_with_track(b"data", b"RVMI", 1, &payloads)
}

/// Writes `rvmi_movie` output to `path`.
pub fn write_rvmi_movie(path: &Path, start_tdatetime: f64, fixes: &[GpsFix]) -> io::Result<()> {
    std::fs::write(path, rvmi_movie(start_tdatetime, fixes))
}
