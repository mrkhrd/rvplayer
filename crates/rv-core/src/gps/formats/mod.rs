//! The format registry (`UnitGPSManager` initialization): one module per `UnitGps_*` unit.
//! The order is the original's detection order. Additions: the Novatek block reader (tried
//! before the original's DOD reader on the same records) and the CSV reader (the original only
//! wrote CSV), which comes last.

use super::format::{Ctx, DataTest, Format, Spec};
use super::util::CharSet;

pub mod cambox;
pub mod chelong;
pub mod csv;
pub mod datdod;
pub mod datsamoon;
pub mod digma;
pub mod dod;
pub mod eagle;
pub mod gprmc;
pub mod gpx;
pub mod hdw;
pub mod ion;
pub mod kml;
pub mod lb;
pub mod logdod;
pub mod logds10;
pub mod logdvr;
pub mod lukas;
pub mod magiceye;
pub mod map;
pub mod mapstr;
pub mod nmea;
pub mod novatek;
pub mod panorama;
pub mod plt;
pub mod ptekd;
pub mod regv;
pub mod roadhawk;
pub mod roadline;
pub mod samoon;
pub mod srt;
pub mod txt1000a;
pub mod txtviofo;

/// Fresh instances of every format, in detection order.
pub fn all() -> Vec<Box<dyn Format>> {
    vec![
        regv::create(),
        gpx::create(),
        kml::create(),
        novatek::create(),
        dod::create(),
        panorama::create(),
        digma::create(),
        map::create(),
        mapstr::create(),
        samoon::create(),
        eagle::create(),
        ptekd::create(),
        hdw::create(),
        ion::create(),
        magiceye::create(),
        roadline::create(),
        roadhawk::create(),
        cambox::create(),
        lb::create(),
        datdod::create(),
        logds10::create(),
        lukas::create(),
        chelong::create(),
        plt::create(),
        logdod::create(),
        logdvr::create(),
        txt1000a::create(),
        txtviofo::create(),
        datsamoon::create(),
        nmea::create(),
        gprmc::create(),
        srt::create(),
        csv::create(),
    ]
}

/// A spec with every field empty, for `..EMPTY_SPEC` in format specs.
pub const EMPTY_SPEC: Spec = Spec {
    title: "",
    def_ext: "",
    format_id: "",
    must_be_chars_in_line: CharSet::EMPTY,
    must_be_chars_in_data: CharSet::EMPTY,
    bad_chars: CharSet::EMPTY,
    must_be_strings_in_data: &[],
    data_prefix: b"",
    line_prefix: b"",
};

/// A format that recognises nothing (placeholder for a unit not ported yet).
pub struct Stub(pub &'static Spec);

impl Format for Stub {
    fn spec(&self) -> &Spec {
        self.0
    }

    fn test_load_data(&mut self, _ctx: &mut Ctx, _data: &[u8]) -> DataTest {
        DataTest::No
    }

    fn test_load_item(&mut self, _ctx: &mut Ctx, _line: &[u8]) -> bool {
        false
    }
}
