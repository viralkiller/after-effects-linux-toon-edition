#[path = "../../xcf/tests/support/mod.rs"]
mod fixture;
use crate::Session;
use effectcraft_project::{ItemId, ItemKind, LayerSource, Project};
use serde_json::json;
fn import(retain: bool, suffix: &str) -> (Session, ItemId, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("toon-xcf-{}-{suffix}.xcf", std::process::id()));
    std::fs::write(&path, fixture::sample()).unwrap();
    let mut s = Session::default();
    let r = s.execute_checked("file.import", json!({"paths":[path],"importAs":if retain{"compositionLayerSizes"}else{"composition"}})).unwrap();
    assert_eq!(r["errors"], json!([]), "{r}");
    let cid = ItemId(r["comps"][0].as_u64().unwrap());
    (s, cid, path)
}
#[test]
fn xcf_composition_preserves_stack_and_geometry() {
    for retain in [true, false] {
        let (s, cid, path) = import(retain, if retain { "retain" } else { "canvas" });
        let c = s.project.comp(cid).unwrap();
        assert_eq!((c.width, c.height), (4, 3));
        assert_eq!(c.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Ink", "Hidden", "Paper"]);
        assert!(!c.layers[1].switches.video);
        let l = &c.layers[0];
        let LayerSource::Footage { item } = l.source else { panic!("footage") };
        let ItemKind::Footage(f) = &s.project.item(item).unwrap().kind else { panic!("footage") };
        assert_eq!((f.width, f.height), if retain { (2, 2) } else { (4, 3) });
        assert_eq!(f.layer.as_ref().unwrap().xcf_tattoo, Some(11));
        assert_eq!(l.props.prop("transform/position").unwrap().value.as_vec2(), if retain { [2., 2.] } else { [2., 1.5] });
        std::fs::remove_file(path).unwrap();
    }
}
#[test]
fn xcf_save_undo_redo() {
    let (mut s, cid, path) = import(true, "undo");
    let saved = serde_json::to_string(s.project.as_ref()).unwrap();
    let restored: Project = serde_json::from_str(&saved).unwrap();
    assert!(restored.items.values().any(|i| matches!(&i.kind,ItemKind::Footage(f)if f.layer.as_ref().is_some_and(|l|l.xcf_tattoo==Some(11)))));
    s.execute_checked("edit.undo", json!({})).unwrap();
    assert!(s.project.comp(cid).is_none());
    s.execute_checked("edit.redo", json!({})).unwrap();
    assert!(s.project.comp(cid).is_some());
    std::fs::remove_file(path).unwrap();
}
#[test]
fn xcf_failed_import_is_atomic() {
    let mut l = fixture::Layer::solid("masked", 1, 1, [0; 4], 1);
    l.mask = true;
    for (label, b) in [("mask", fixture::fixture(1, 1, 12, 150, 0, &[l], None)), ("short", fixture::sample()[..80].to_vec())] {
        let path = std::env::temp_dir().join(format!("xcf-bad-{}-{label}.xcf", std::process::id()));
        std::fs::write(&path, b).unwrap();
        let mut s = Session::default();
        let before = serde_json::to_value(s.project.as_ref()).unwrap();
        let r = s.execute_checked("file.import", json!({"paths":[path],"importAs":"composition"})).unwrap();
        assert!(!r["errors"].as_array().unwrap().is_empty());
        assert!(r["toast"].as_str().is_some_and(|t| t.contains("XCF")));
        assert_eq!(serde_json::to_value(s.project.as_ref()).unwrap(), before);
        std::fs::remove_file(path).unwrap();
    }
}
#[test]
fn xcf_colour_warning_does_not_change_project() {
    let mut l = fixture::Layer::solid("linear", 1, 1, [255; 4], 1);
    l.props.push((7, 28u32.to_be_bytes().to_vec()));
    let b = fixture::fixture(1, 1, 12, 150, 0, &[l], None);
    let d = effectcraft_xcf::Document::parse(&b).unwrap();
    let mut p = Project::default();
    let before = p.settings.clone();
    let r = crate::xcf_import::import(&mut p, &d, "test.xcf", "test", false, effectcraft_time::FrameRate::FPS_30, effectcraft_time::Tick::from_seconds_f64(1.));
    assert_eq!(p.settings, before);
    assert_eq!(r.warnings.len(), 1);
}

#[test]
fn xcf_auto_blend_creates_project_items() {
    let mut layers = vec![fixture::Layer::solid("front", 2, 2, [255; 4], 2), fixture::Layer::solid("back", 2, 2, [255; 4], 3)];
    for layer in &mut layers {
        layer.props.extend([
            (7, 28u32.to_be_bytes().to_vec()),
            (37, 0u32.to_be_bytes().to_vec()),
            (36, (-1i32).to_be_bytes().to_vec()),
            (35, (-1i32).to_be_bytes().to_vec()),
        ]);
    }
    let path = std::env::temp_dir().join(format!("xcf-auto-{}.xcf", std::process::id()));
    std::fs::write(&path, fixture::fixture(2, 2, 11, 150, 1, &layers, None)).unwrap();
    let mut s = Session::default();
    let r = s.execute_checked("file.import", json!({"paths":[path], "importAs":"composition", "background":true})).unwrap();
    assert_eq!(r["comps"].as_array().unwrap().len(), 1);
    assert_eq!(r["items"].as_array().unwrap().len(), 4); // Folder, two footage items, composition.
    let cid = ItemId(r["comps"][0].as_u64().unwrap());
    let comp = s.project.comp(cid).unwrap();
    assert_eq!(comp.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["front", "back"]);
    let footage_count =
        r["items"].as_array().unwrap().iter().filter(|id| matches!(s.project.item(ItemId(id.as_u64().unwrap())).unwrap().kind, ItemKind::Footage(_))).count();
    assert_eq!(footage_count, 2);
    std::fs::remove_file(path).unwrap();
}

fn updated_xcf() -> Vec<u8> {
    fixture::fixture(4, 3, 12, 150, 0, &[
        fixture::Layer::solid("layer4", 1, 1, [255; 4], 14),
        fixture::Layer::solid("Renamed Ink", 2, 2, [0, 255, 0, 255], 11),
        fixture::Layer::solid("Paper", 4, 3, [0, 0, 255, 255], 13),
        fixture::Layer::solid("Hidden", 4, 3, [0, 255, 0, 255], 12),
        fixture::Layer::solid("layer5", 1, 1, [255; 4], 15),
    ], None)
}

fn xcf_items(s: &Session, path: &std::path::Path) -> Vec<(ItemId, effectcraft_project::Footage)> {
    s.project.items.values().filter_map(|i| match &i.kind {
        ItemKind::Footage(f) if f.path == path.to_string_lossy() => Some((i.id, f.clone())),
        _ => None,
    }).collect()
}

#[test]
fn xcf_reload_reopen_discovers_layers_and_preserves_all_timeline_edits() {
    let (mut s, cid, path) = import(true, "reopen-sync");
    s.open_comp(cid);
    let layer = s.project.comp(cid).unwrap().layers[0].id;
    s.execute("prop.toggleAnimation", json!({"layer": layer.0, "path": "transform/position"})).unwrap();
    s.execute("time.set", json!({"time": 1})).unwrap();
    s.execute("prop.set", json!({"layer": layer.0, "path": "transform/position", "value": [100, 200]})).unwrap();
    s.execute("layer.timing", json!({"layers": [layer.0], "in": 0.5, "out": 2})).unwrap();
    let timeline = serde_json::to_value(s.project.comp(cid).unwrap()).unwrap();
    let originals = xcf_items(&s, &path);
    let project_path = path.with_extension("ecproj");
    std::fs::write(&project_path, s.project.to_json()).unwrap();
    std::fs::write(&path, updated_xcf()).unwrap();
    // Reopen an older project with no watcher history, as after quitting the app.
    let mut reopened = Session::default();
    reopened.execute("file.open", json!({"path": project_path})).unwrap();
    let result = reopened.execute("footage.reloadChanged", json!({})).unwrap();
    assert_eq!(result["reloaded"], 3);
    assert_eq!(serde_json::to_value(reopened.project.comp(cid).unwrap()).unwrap(), timeline);
    let fresh = xcf_items(&reopened, &path);
    assert_eq!(fresh.len(), 5);
    for (id, old) in originals {
        let f = &fresh.iter().find(|(i, _)| *i == id).unwrap().1;
        assert_eq!(f.layer.as_ref().unwrap().xcf_tattoo, old.layer.as_ref().unwrap().xcf_tattoo);
        assert!(!f.missing);
    }
    assert!(reopened.is_dirty(), "new project items must be saved");
    let before = reopened.project.to_json();
    reopened.execute("footage.reloadChanged", json!({})).unwrap();
    assert_eq!(reopened.project.to_json(), before, "no duplicate discovery");
    // Explicit reload is equally idempotent and refreshes the whole document.
    crate::footage_reload::reload_items(&mut reopened, &[fresh[0].0]).unwrap();
    assert_eq!(xcf_items(&reopened, &path).len(), 5);
    std::fs::remove_file(project_path).unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn xcf_reload_deleted_identity_is_not_replaced_by_same_name() {
    let (mut s, cid, path) = import(true, "identity-sync");
    let originals = xcf_items(&s, &path);
    let ink = originals.iter().find(|(_, f)| f.layer.as_ref().unwrap().xcf_tattoo == Some(11)).unwrap().0;
    let timeline = serde_json::to_value(s.project.comp(cid).unwrap()).unwrap();
    let bytes = fixture::fixture(4, 3, 12, 150, 0, &[fixture::Layer::solid("Ink", 2, 2, [255; 4], 99)], None);
    std::fs::write(&path, bytes).unwrap();
    crate::footage_reload::reload_items(&mut s, &[ink]).unwrap();
    let items = xcf_items(&s, &path);
    assert!(items.iter().find(|(i, _)| *i == ink).unwrap().1.missing);
    assert!(items.iter().any(|(id, f)| *id != ink && f.layer.as_ref().unwrap().xcf_tattoo == Some(99)));
    s.execute("footage.check", json!({"wait": true})).unwrap();
    assert!(xcf_items(&s, &path).iter().find(|(i, _)| *i == ink).unwrap().1.missing);
    assert_eq!(serde_json::to_value(s.project.comp(cid).unwrap()).unwrap(), timeline);
    // Restore the original source: its existing item is reconnected, never recreated.
    std::fs::write(&path, fixture::sample()).unwrap();
    crate::footage_reload::reload_items(&mut s, &[ink]).unwrap();
    assert!(!xcf_items(&s, &path).iter().find(|(i, _)| *i == ink).unwrap().1.missing);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn xcf_reload_retries_incomplete_save_without_acknowledging_bad_stamp() {
    let (mut s, _, path) = import(false, "retry-sync");
    let key = path.to_string_lossy().into_owned();
    s.mark_saved();
    let before = s.project.to_json();
    std::fs::write(&path, b"gimp xcf v012\0").unwrap();
    let stamp = Some((123, 456));
    crate::footage_reload::apply_stamps(&mut s, vec![(key.clone(), stamp)]).unwrap();
    assert_eq!(s.project.to_json(), before);
    assert!(!s.is_dirty());
    assert!(!s.footage_stamps.contains_key(&key));
    let ids = xcf_items(&s, &path);
    assert!(crate::footage_reload::reload_items(&mut s, &[ids[0].0]).is_err());
    assert_eq!(s.project.to_json(), before);
    std::fs::write(&path, updated_xcf()).unwrap();
    crate::footage_reload::apply_stamps(&mut s, vec![(key, stamp)]).unwrap();
    assert_eq!(xcf_items(&s, &path).len(), 5, "retry even with the same observed stamp");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn xcf_reload_pixel_only_refresh_does_not_dirty_saved_project() {
    let (mut s, _, path) = import(false, "pixel-sync");
    s.mark_saved();
    let revision = s.revision;
    s.execute("footage.reloadChanged", json!({})).unwrap();
    assert!(!s.is_dirty());
    assert!(s.revision > revision);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn xcf_reload_rejects_unsupported_save_atomically() {
    let (mut s, _, path) = import(true, "unsupported-sync");
    let before = s.project.to_json();
    let ids = xcf_items(&s, &path);
    let mut unsupported = fixture::Layer::solid("Ink", 2, 2, [255; 4], 11);
    unsupported.mask = true;
    std::fs::write(&path, fixture::fixture(4, 3, 12, 150, 0, &[unsupported], None)).unwrap();
    assert!(crate::footage_reload::reload_items(&mut s, &[ids[0].0]).is_err());
    assert_eq!(s.project.to_json(), before);
    std::fs::remove_file(path).unwrap();
}
