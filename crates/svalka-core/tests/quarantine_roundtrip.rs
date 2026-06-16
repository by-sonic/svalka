use std::fs;
use svalka_core::quarantine::QuarantineStore;
use tempfile::tempdir;

#[test]
fn file_roundtrip_restores_bytes_to_original_path() {
    let vol = tempdir().unwrap(); // pretend "volume" root
    let meta = tempdir().unwrap(); // %LocalAppData%\Svalka

    // a junk file we will quarantine then restore
    let junk = vol.path().join("project").join("cache.bin");
    fs::create_dir_all(junk.parent().unwrap()).unwrap();
    fs::write(&junk, b"precious-cache").unwrap();

    let mut store = QuarantineStore::new(meta.path().to_path_buf());
    let entry = store
        .quarantine_paths(
            "test",
            "Очистка",
            1000,
            vol.path(),
            std::slice::from_ref(&junk),
            false,
        )
        .unwrap()
        .entry;

    assert!(!junk.exists(), "source should be moved out");
    assert_eq!(entry.total(), 14);

    store.restore("test").unwrap();

    assert!(junk.exists(), "file should be back");
    assert_eq!(fs::read(&junk).unwrap(), b"precious-cache");
    assert!(store.list().is_empty(), "entry removed after restore");
}
