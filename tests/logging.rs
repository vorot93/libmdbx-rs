//! Own test binary: installs a global `log` logger.

use libmdbx::{Database, NoWriteMap};
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::sync::Mutex;
use tempfile::tempdir;

struct Capture(Mutex<Vec<(Level, String, String)>>);

impl Log for Capture {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        self.0.lock().unwrap().push((
            record.level(),
            record.target().to_string(),
            record.args().to_string(),
        ));
    }

    fn flush(&self) {}
}

static CAPTURE: Capture = Capture(Mutex::new(Vec::new()));

/// libmdbx's own log messages go to the `log` crate (target `libmdbx`)
/// rather than straight to the process's stderr.
#[test]
fn test_libmdbx_logs_route_to_log_crate() {
    log::set_logger(&CAPTURE).unwrap();
    log::set_max_level(LevelFilter::Trace);

    let dir = tempdir().unwrap();
    // Opening a fresh environment makes libmdbx log at NOTICE level.
    drop(Database::<NoWriteMap>::open(&dir).unwrap());

    let records = CAPTURE.0.lock().unwrap();
    let from_mdbx: Vec<_> = records
        .iter()
        .filter(|(_, target, _)| target == "libmdbx")
        .collect();
    assert!(!from_mdbx.is_empty(), "no libmdbx records in {records:?}");
    assert!(
        from_mdbx
            .iter()
            .all(|(_, _, msg)| !msg.is_empty() && !msg.ends_with('\n')),
        "{from_mdbx:?}"
    );
}
