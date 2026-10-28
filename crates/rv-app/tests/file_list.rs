//! UI tests for the file list, driving the real app through egui_kittest.

use std::path::{Path, PathBuf};
use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use rv_app::RvApp;
use rv_app::app::MapSource;
use rv_core::testutil::{GpsFix, write_rvmi_movie};

/// A fresh folder with `n` short RVMI-only movies named `clip_0.mov`, `clip_1.mov`, …
fn fixture_folder(test: &str, n: usize) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rvplayer-test-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..n {
        let fixes: Vec<GpsFix> = (0..10)
            .map(|s| GpsFix {
                lat: 60.0 + f64::from(s) * 1e-4,
                lon: 30.0,
                speed_kmh: 40.0 + f64::from(s),
                course: 0.0,
            })
            .collect();
        // 2016-02-13 18:39 plus one hour per file
        write_rvmi_movie(
            &dir.join(format!("clip_{i}.mov")),
            42413.777 + i as f64 / 24.0,
            &fixes,
        )
        .unwrap();
    }
    dir
}

fn harness_for(folder: &Path) -> Harness<'static, RvApp> {
    let folder = folder.to_path_buf();
    Harness::builder()
        .with_size([1280.0, 800.0])
        .build_eframe(move |cc| {
            let mut app = RvApp::new(cc, Some(folder.clone()));
            app.view.map = MapSource::None; // no tile downloads in tests
            app
        })
}

/// Steps until the background folder scan has finished.
fn wait_loaded(h: &mut Harness<'_, RvApp>) {
    for _ in 0..500 {
        h.step();
        if h.state().loading.is_none() {
            h.step();
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("folder did not finish loading");
}

#[test]
fn lists_all_files_and_selects_the_first() {
    let dir = fixture_folder("list", 3);
    let mut h = harness_for(&dir);
    wait_loaded(&mut h);
    let app = h.state();
    let names: Vec<&str> = app.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["clip_0.mov", "clip_1.mov", "clip_2.mov"]);
    assert_eq!(app.selected, Some(0));
    assert!(
        app.files
            .iter()
            .all(|f| f.error.is_none() && f.track.gps.len() == 10)
    );
}

#[test]
fn clicking_a_row_selects_it() {
    let dir = fixture_folder("click", 3);
    let mut h = harness_for(&dir);
    wait_loaded(&mut h);
    h.get_by_label("clip_1.mov").click();
    h.run();
    assert_eq!(h.state().selected, Some(1));
    assert_eq!(h.state().playback.position, 0.0);
}
