use bevy::prelude::*;
use factory::AudioSettings;
use factory::PointerKind;
use factory::PointerTracker;
use factory::SafeAreaInsets;
use factory::SaveData;
use factory::SaveStore;

#[test]
fn save_round_trip() {
    let mut save = SaveData::default();
    save.settings.master = 0.4;
    save.settings.muted = true;
    save.progress.runs = 7;
    save.progress.best_score = 10;
    assert_eq!(SaveData::decode(&save.encode().unwrap()).unwrap(), save);
}
#[test]
fn migration_preserves_progress_and_mute() {
    let save = SaveData::decode(r#"{"version":1,"muted":true,"best_score":9}"#).unwrap();
    assert_eq!(save.version, 2);
    assert!(save.settings.muted);
    assert_eq!(save.settings.master, 0.7);
    assert_eq!(save.progress.runs, 0);
    assert_eq!(save.progress.best_score, 9);
}
#[test]
fn rejects_corrupt_unknown_and_incomplete_saves() {
    for json in [
        "broken",
        "{}",
        r#"{"version":99}"#,
        r#"{"version":2}"#,
        r#"{"version":1,"muted":true}"#,
        r#"{"version":1,"muted":"yes","best_score":0}"#,
        r#"{"version":1,"muted":false,"best_score":-1}"#,
    ] {
        assert!(SaveData::decode(json).is_err(), "accepted {json}");
    }
    for volume in [-0.1, 1.1, f32::NAN, f32::INFINITY] {
        let mut save = SaveData::default();
        save.settings.master = volume;
        assert!(save.encode().is_err());
    }
    let save = SaveData {
        version: 3,
        ..default()
    };
    assert!(save.encode().is_err());
    assert!(SaveData::decode(r#"{"version":2,"settings":{"master":2,"muted":false},"progress":{"runs":0,"best_score":0}}"#).is_err());
}
#[test]
fn rejects_invalid_application_identifiers() {
    for name in ["", "../escape", "with space", "é", "path/name"] {
        assert!(SaveStore::new(name).is_err());
    }
    assert!(SaveStore::new("software.sagan.proto-1").is_ok());
    assert_eq!(factory::SaveError("failure".into()).to_string(), "failure");
}
#[test]
fn native_store_missing_round_trip_and_failures() {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-saves");
    let id = std::process::id();
    let path = base.join(format!("round-trip-{id}"));
    let store = SaveStore::new("test").unwrap().with_directory(&path);
    assert_eq!(store.load().unwrap(), SaveData::default());
    let mut save = SaveData::default();
    save.progress.best_score = 8;
    store.store(&save).unwrap();
    assert_eq!(store.load().unwrap(), save);
    save.progress.best_score = 9;
    store.store(&save).unwrap();
    assert_eq!(store.load().unwrap(), save);
    std::fs::write(path.join("save.json"), "broken").unwrap();
    assert!(store.load().is_err());
    let blocker = base.join(format!("file-{id}"));
    std::fs::write(&blocker, "file").unwrap();
    let bad = SaveStore::new("test").unwrap().with_directory(blocker);
    assert!(bad.load().is_err());
    assert!(bad.store(&save).is_err());
    let write_blocker = base.join(format!("write-{id}"));
    std::fs::create_dir_all(write_blocker.join("save.tmp")).unwrap();
    assert!(
        SaveStore::new("test")
            .unwrap()
            .with_directory(write_blocker)
            .store(&save)
            .is_err()
    );
    let rename_blocker = base.join(format!("rename-{id}"));
    std::fs::create_dir_all(rename_blocker.join("save.json")).unwrap();
    assert!(
        SaveStore::new("test")
            .unwrap()
            .with_directory(rename_blocker)
            .store(&save)
            .is_err()
    );
}
#[test]
fn gesture_thresholds_and_cancellation() {
    let mut p = PointerTracker::default();
    assert!(p.end(1, Vec2::ZERO, 0.1).is_none());
    assert!(p.movement(1, Vec2::ZERO).is_none());
    p.begin(1, Vec2::ZERO, 0.0);
    assert!(p.movement(1, Vec2::new(11.0, 0.0)).is_none());
    assert_eq!(p.end(1, Vec2::ZERO, 0.499).unwrap().kind, PointerKind::Tap);
    p.begin(1, Vec2::ZERO, 0.0);
    assert!(p.holds(0.499).is_empty());
    assert_eq!(p.holds(0.5)[0].kind, PointerKind::LongPress);
    assert!(p.holds(1.0).is_empty());
    assert!(p.end(1, Vec2::ZERO, 1.0).is_none());
    p.begin(1, Vec2::ZERO, 0.0);
    assert_eq!(
        p.end(1, Vec2::ZERO, 0.5).unwrap().kind,
        PointerKind::LongPress
    );
    p.begin(1, Vec2::ZERO, 0.0);
    let drag = p.movement(1, Vec2::new(12.0, 0.0)).unwrap();
    assert_eq!(drag.kind, PointerKind::Drag);
    assert_eq!(drag.delta, Vec2::new(12.0, 0.0));
    assert!(p.holds(2.0).is_empty());
    assert_eq!(p.movement(1, Vec2::ZERO).unwrap().kind, PointerKind::Drag);
    assert!(p.end(1, Vec2::ZERO, 2.0).is_none());
    p.begin(1, Vec2::ZERO, 0.0);
    assert!(p.end(1, Vec2::new(12.0, 0.0), 0.1).is_none());
    p.begin(1, Vec2::ZERO, 0.0);
    p.begin(2, Vec2::ONE, 0.0);
    p.cancel(1);
    assert!(p.end(1, Vec2::ZERO, 0.1).is_none());
    assert_eq!(p.end(2, Vec2::ONE, 0.1).unwrap().kind, PointerKind::Tap);
    p.begin(1, Vec2::ZERO, 0.0);
    p.begin(1, Vec2::ONE, 1.0);
    assert_eq!(p.end(1, Vec2::ONE, 1.1).unwrap().kind, PointerKind::Tap);
}
#[test]
fn master_settings_and_safe_area_normalization() {
    assert_eq!(AudioSettings::default().volume(), 0.7);
    for master in [f32::NAN, f32::INFINITY, -1.0] {
        assert_eq!(
            AudioSettings {
                master,
                muted: false
            }
            .volume(),
            0.0
        );
    }
    assert_eq!(
        AudioSettings {
            master: 2.0,
            muted: false
        }
        .volume(),
        1.0
    );
    assert_eq!(
        AudioSettings {
            master: 0.9,
            muted: true
        }
        .volume(),
        0.0
    );
    assert_eq!(
        SafeAreaInsets {
            top: -1.0,
            right: f32::NAN,
            bottom: f32::INFINITY,
            left: 5.0
        }
        .sanitized(),
        SafeAreaInsets {
            left: 5.0,
            ..default()
        }
    );
}
