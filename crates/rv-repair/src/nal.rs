//! Length-prefixed H.264 / H.265 NAL units: header classification and the per-camera "style" of
//! video frames learned from the reference file.

/// Longest NAL unit accepted while walking a frame (the original's `$500000`).
pub(crate) const MAX_NAL: u32 = 0x50_0000;
/// Bytes before a NAL header: a 4-byte big-endian length (the original assumes 4).
pub(crate) const LENGTH_SIZE: usize = 4;

/// How the start of a video frame is recognised in raw media data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartRule {
    /// The original's `IsVideoFrameStart`: an H.264 access unit delimiter `00 00 00 02 09`, or a
    /// 16-byte proprietary frame header whose size field at +8 matches the NAL length at +16.
    Original,
    /// H.265 access unit delimiter (length 3, type 35, layer 0). Extension: the original has no
    /// H.265 rule.
    HevcAud,
    /// Extension for streams without delimiters: a NAL unit whose header is one the reference's
    /// frames begin with (and, for a slice, the first slice of a picture). Candidates are only
    /// accepted after their whole NAL chain has been walked.
    Learned,
}

#[derive(Debug, Clone)]
pub(crate) struct VideoStyle {
    pub hevc: bool,
    pub rule: StartRule,
    /// Header bytes (byte 1 is used for H.265 only) of the first NAL unit of the reference frames.
    pub starts: Vec<[u8; 2]>,
    /// Some reference frame has more than one slice: the following slices of a picture belong to
    /// its frame. The original ends every frame at its first slice.
    pub multi_slice: bool,
}

impl VideoStyle {
    /// The original rule, used for H.264 when the reference teaches nothing better.
    pub fn original() -> Self {
        Self {
            hevc: false,
            rule: StartRule::Original,
            starts: Vec::new(),
            multi_slice: false,
        }
    }

    pub fn header_len(&self) -> usize {
        if self.hevc { 2 } else { 1 }
    }

    fn nal_type(&self, h0: u8) -> u8 {
        if self.hevc {
            (h0 >> 1) & 0x3f
        } else {
            h0 & 0x1f
        }
    }

    /// Coded slice. H.264: types 1–5 (the original's `(type - 1) <= 4`); H.265: types 0–31.
    pub fn is_vcl(&self, h0: u8) -> bool {
        let t = self.nal_type(h0);
        if self.hevc {
            t <= 31
        } else {
            t.wrapping_sub(1) <= 4
        }
    }

    /// Key frame slice. H.264: IDR (5, as the original); H.265: IRAP (16–23).
    pub fn is_key(&self, h0: u8) -> bool {
        let t = self.nal_type(h0);
        if self.hevc {
            (16..=23).contains(&t)
        } else {
            t == 5
        }
    }

    /// Access unit delimiter in the exact form the rules look for.
    pub fn is_aud(&self, len: u32, h: [u8; 2]) -> bool {
        if self.hevc {
            len == 3 && self.nal_type(h[0]) == 35 && h[0] & 0x81 == 0 && h[1] & 0xf8 == 0
        } else {
            len == 2 && h[0] == 0x09
        }
    }

    /// `p` starts at a NAL length prefix: is that NAL a slice that begins a picture?
    /// (`first_mb_in_slice == 0` / `first_slice_segment_in_pic_flag`, the top bit after the header.)
    pub fn first_slice(&self, p: &[u8]) -> bool {
        p.get(LENGTH_SIZE + self.header_len())
            .is_some_and(|b| b & 0x80 != 0)
    }

    /// Two slice headers belong to the same picture's slice sequence (same NAL header bytes).
    pub fn same_slice_kind(&self, a: [u8; 2], b: [u8; 2]) -> bool {
        a[0] == b[0] && (!self.hevc || a[1] == b[1])
    }

    pub fn header(&self, p: &[u8]) -> [u8; 2] {
        let h0 = p.get(LENGTH_SIZE).copied().unwrap_or(0);
        let h1 = if self.hevc {
            p.get(LENGTH_SIZE + 1).copied().unwrap_or(0)
        } else {
            0
        };
        [h0, h1]
    }

    /// `p` (at a length prefix) looks like the first NAL unit of a frame under the learned rule.
    pub fn learned_start(&self, p: &[u8]) -> bool {
        let Some(len) = crate::boxes::be32(p, 0) else {
            return false;
        };
        if !(2..=MAX_NAL).contains(&len) {
            return false;
        }
        let h = self.header(p);
        self.starts.iter().any(|s| self.same_slice_kind(*s, h))
            && (!self.is_vcl(h[0]) || self.first_slice(p))
    }
}

/// What the reference's first frames look like, gathered by [`learn`].
#[derive(Default)]
pub(crate) struct Observed {
    /// First NAL (length, header) of each examined frame.
    pub firsts: Vec<(u32, [u8; 2])>,
    pub multi_slice: bool,
}

impl Observed {
    /// Records one frame given the NAL units it consists of (length, header, byte after header).
    pub fn frame(&mut self, style: &VideoStyle, nals: &[(u32, [u8; 2])]) {
        let Some(&first) = nals.first() else {
            return;
        };
        self.firsts.push(first);
        if nals.iter().filter(|(_, h)| style.is_vcl(h[0])).count() > 1 {
            self.multi_slice = true;
        }
    }
}

/// Chooses the frame-start rule for a codec from what the reference's frames look like.
pub(crate) fn learn(hevc: bool, seen: &Observed) -> VideoStyle {
    let mut style = VideoStyle {
        hevc,
        rule: if hevc {
            StartRule::HevcAud
        } else {
            StartRule::Original
        },
        starts: Vec::new(),
        multi_slice: seen.multi_slice,
    };
    if seen.firsts.is_empty() {
        return style; // nothing learned: the original rule (or its H.265 counterpart)
    }
    if seen.firsts.iter().all(|&(len, h)| style.is_aud(len, h)) {
        return style;
    }
    let mut starts: Vec<[u8; 2]> = seen.firsts.iter().map(|&(_, h)| h).collect();
    if !hevc {
        for s in &mut starts {
            s[1] = 0;
        }
    }
    starts.sort_unstable();
    starts.dedup();
    style.starts = starts;
    style.rule = StartRule::Learned;
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_h264_and_hevc_headers() {
        let avc = VideoStyle::original();
        assert!(avc.is_vcl(0x65) && avc.is_key(0x65));
        assert!(avc.is_vcl(0x41) && !avc.is_key(0x41));
        assert!(!avc.is_vcl(0x67) && !avc.is_vcl(0x09) && !avc.is_vcl(0x00));
        assert!(avc.is_aud(2, [0x09, 0]) && !avc.is_aud(2, [0x29, 0]));
        let hevc = VideoStyle {
            hevc: true,
            ..VideoStyle::original()
        };
        assert!(hevc.is_vcl(0x26) && hevc.is_key(0x26)); // IDR_W_RADL
        assert!(hevc.is_vcl(0x02) && !hevc.is_key(0x02)); // TRAIL_R
        assert!(!hevc.is_vcl(0x40) && !hevc.is_vcl(0x42)); // VPS, SPS
        assert!(hevc.is_aud(3, [0x46, 0x01]));
    }

    #[test]
    fn learns_rule_from_reference_frames() {
        let avc = VideoStyle::original();
        let mut seen = Observed::default();
        seen.frame(&avc, &[(2, [9, 0]), (20, [0x67, 0]), (5000, [0x65, 0])]);
        seen.frame(&avc, &[(2, [9, 0]), (900, [0x41, 0])]);
        let style = learn(false, &seen);
        assert_eq!(style.rule, StartRule::Original);
        assert!(!style.multi_slice);

        let mut seen = Observed::default();
        seen.frame(
            &avc,
            &[
                (20, [0x67, 0]),
                (4, [0x68, 0]),
                (500, [0x65, 0]),
                (500, [0x65, 0]),
            ],
        );
        seen.frame(&avc, &[(900, [0x41, 0])]);
        let style = learn(false, &seen);
        assert_eq!(style.rule, StartRule::Learned);
        assert_eq!(style.starts, [[0x41, 0], [0x67, 0]]);
        assert!(style.multi_slice);
        // a P slice that is not the first slice of its picture does not start a frame
        assert!(style.learned_start(&[0, 0, 3, 0x84, 0x41, 0x9a, 0, 0]));
        assert!(!style.learned_start(&[0, 0, 3, 0x84, 0x41, 0x1a, 0, 0]));
        assert!(!style.learned_start(&[0, 0x60, 0, 0, 0x41, 0x9a, 0, 0])); // too long
    }
}
