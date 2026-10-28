//! Planar YUV 4:2:0 to packed RGBA conversion (fixed-point, rows in parallel).

use rayon::prelude::*;

/// YCbCr to RGB matrix (ITU-T H.273 `matrix_coefficients`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matrix {
    Bt601,
    Bt709,
    Bt2020,
}

impl Matrix {
    /// Matrix from the stream's VUI code point; unspecified streams use BT.709 for HD and BT.601 for SD.
    pub fn from_h273(code: u8, height: u32) -> Self {
        match code {
            1 => Matrix::Bt709,
            5 | 6 => Matrix::Bt601,
            9 | 10 => Matrix::Bt2020,
            _ if height >= 720 => Matrix::Bt709,
            _ => Matrix::Bt601,
        }
    }

    fn kr_kb(self) -> (f64, f64) {
        match self {
            Matrix::Bt601 => (0.299, 0.114),
            Matrix::Bt709 => (0.2126, 0.0722),
            Matrix::Bt2020 => (0.2627, 0.0593),
        }
    }
}

pub struct Planes<'a> {
    pub y: &'a [u8],
    pub u: &'a [u8],
    pub v: &'a [u8],
    pub y_stride: usize,
    pub uv_stride: usize,
}

/// 16.16 fixed-point coefficients.
struct Coeffs {
    y: i32,
    y_offset: i32,
    r_v: i32,
    g_u: i32,
    g_v: i32,
    b_u: i32,
}

impl Coeffs {
    fn new(m: Matrix, full_range: bool) -> Self {
        let (kr, kb) = m.kr_kb();
        let kg = 1.0 - kr - kb;
        let (ys, cs) = if full_range {
            (1.0, 1.0)
        } else {
            (255.0 / 219.0, 255.0 / 224.0)
        };
        let fx = |v: f64| (v * 65536.0).round() as i32;
        Self {
            y: fx(ys),
            y_offset: if full_range { 0 } else { 16 },
            r_v: fx(cs * 2.0 * (1.0 - kr)),
            g_u: fx(cs * 2.0 * (1.0 - kb) * kb / kg),
            g_v: fx(cs * 2.0 * (1.0 - kr) * kr / kg),
            b_u: fx(cs * 2.0 * (1.0 - kb)),
        }
    }
}

pub fn to_rgba(p: &Planes<'_>, width: u32, height: u32, m: Matrix, full_range: bool) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let c = Coeffs::new(m, full_range);
    let mut out = vec![0u8; w * h * 4];
    out.par_chunks_mut(w * 4)
        .take(h)
        .enumerate()
        .for_each(|(row, dst)| {
            let y_row = &p.y[row * p.y_stride..row * p.y_stride + w];
            let uv = (row / 2) * p.uv_stride;
            let (u_row, v_row) = (&p.u[uv..], &p.v[uv..]);
            for (x, px) in dst.chunks_exact_mut(4).enumerate() {
                let yy = (i32::from(y_row[x]) - c.y_offset) * c.y;
                let u = i32::from(u_row[x / 2]) - 128;
                let v = i32::from(v_row[x / 2]) - 128;
                let clamp = |val: i32| ((val + 32768) >> 16).clamp(0, 255) as u8;
                px[0] = clamp(yy + c.r_v * v);
                px[1] = clamp(yy - c.g_u * u - c.g_v * v);
                px[2] = clamp(yy + c.b_u * u);
                px[3] = 255;
            }
        });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert_one(y: u8, u: u8, v: u8, full: bool) -> [u8; 4] {
        let planes = Planes {
            y: &[y, y, y, y],
            u: &[u],
            v: &[v],
            y_stride: 2,
            uv_stride: 1,
        };
        to_rgba(&planes, 2, 2, Matrix::Bt709, full)[..4]
            .try_into()
            .unwrap()
    }

    #[test]
    fn limited_range_black_white_and_grey() {
        assert_eq!(convert_one(16, 128, 128, false), [0, 0, 0, 255]);
        assert_eq!(convert_one(235, 128, 128, false), [255, 255, 255, 255]);
        assert_eq!(convert_one(128, 128, 128, true), [128, 128, 128, 255]);
    }

    #[test]
    fn bt709_primaries_round_trip() {
        // Limited-range BT.709 red is approximately Y=63, Cb=102, Cr=240.
        let [r, g, b, _] = convert_one(63, 102, 240, false);
        assert!(r >= 250 && g <= 5 && b <= 5, "{r} {g} {b}");
    }
}
