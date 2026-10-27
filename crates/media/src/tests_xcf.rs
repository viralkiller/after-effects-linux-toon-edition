#[path = "../../xcf/tests/support/mod.rs"]
mod fixture;
use crate::convert::AlphaOp;
use crate::layered::{decode, probe};
use effectcraft_project::{AlphaMode, SourceLayer};
use effectcraft_time::Tick;
#[test]
fn xcf_probe_layer_and_flatten() {
    let b = fixture::sample();
    let mut f = probe("plain.xcf", &b).unwrap().unwrap();
    assert_eq!((f.width, f.height), (4, 3));
    assert_eq!(f.codec, "XCF");
    let op = AlphaOp::new(AlphaMode::Straight, [0.; 3]);
    assert_eq!(decode("plain.xcf", &b, &f, op).unwrap().unwrap().data[0], [0., 0., 1., 1.]);
    f.layer = Some(SourceLayer { index: 0, name: "Ink".into(), layer_size: true, xcf_tattoo: Some(11), ..Default::default() });
    let p = decode("plain.xcf", &b, &f, op).unwrap().unwrap();
    assert_eq!((p.width, p.height), (2, 2));
    assert!((p.data[0][0] - 128. / 255.).abs() < 1e-6);
    assert_eq!(p.data[0][0], p.data[0][3]);
}
#[test]
fn xcf_cache_refresh_uses_identity_not_index() {
    let b = fixture::sample();
    let mut f = probe("reload.xcf", &b).unwrap().unwrap();
    f.layer = Some(SourceLayer { index: 0, name: "Ink".into(), layer_size: true, xcf_tattoo: Some(11), ..Default::default() });
    f.width = 2;
    f.height = 2;
    let pool = crate::MediaPool::with_budget(1024 * 1024);
    pool.add_bytes("reload.xcf", b.into());
    assert!(pool.frame_at(&f, Tick::ZERO).unwrap().data[0][0] > 0.4);
    let b = fixture::fixture(
        4,
        3,
        12,
        150,
        0,
        &[fixture::Layer::solid("Other", 1, 1, [255, 0, 0, 255], 44), fixture::Layer::solid("Renamed", 2, 2, [0, 255, 0, 255], 11)],
        None,
    );
    pool.add_bytes("reload.xcf", b.into());
    assert_eq!(pool.frame_at(&f, Tick::ZERO).unwrap().data[0], [0., 1., 0., 1.]);
    let b = fixture::fixture(4, 3, 12, 150, 0, &[fixture::Layer::solid("Ink", 2, 2, [255; 4], 99)], None);
    assert!(decode("reload.xcf", &b, &f, AlphaOp::new(AlphaMode::Straight, [0.; 3])).is_err());
}
