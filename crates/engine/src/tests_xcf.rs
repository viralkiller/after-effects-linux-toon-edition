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
