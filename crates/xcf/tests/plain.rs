mod support;
use effectcraft_xcf::Document;
use support::{Layer, fixture};
fn zip(b: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(b, 6)
}
#[test]
fn xcf_raw_rle_zip_and_edge_tiles() {
    for version in [0, 3, 10, 12, 23, 26] {
        for compression in 0..=2 {
            let mut l = Layer::solid("edge", 65, 67, [0, 0, 0, 255], 1);
            for (i, p) in l.rgba.iter_mut().enumerate() {
                *p = [((i * 31) % 65536) as u16, 65535, 0, 65535];
            }
            let b = fixture(65, 67, version, 150, compression, &[l], Some(zip));
            let d = Document::parse(&b).unwrap();
            let p = d.layer_pixels(0, false).unwrap();
            d.validate_pixels().unwrap();
            for (i, p) in p.data.iter().enumerate() {
                assert!((p[0] - (((i * 31) % 65536) / 257) as f32 / 255.).abs() < 1e-6);
                assert_eq!(p[1], 1.);
            }
        }
    }
}
#[test]
fn xcf_stacking_visibility_and_sizes() {
    let b = support::sample();
    let d = Document::parse(&b).unwrap();
    assert_eq!(d.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Ink", "Hidden", "Paper"]);
    let p = d.composite().unwrap();
    assert_eq!(p.data[0], [0., 0., 1., 1.]);
    assert!((p.data[5][0] - 128. / 255.).abs() < 1e-6);
    assert!((p.data[5][2] - 127. / 255.).abs() < 1e-6);
    assert_eq!(d.layer_pixels(0, false).unwrap().data.len(), 4);
    let p = d.layer_pixels(0, true).unwrap();
    assert_eq!(p.data.len(), 12);
    assert_eq!(p.data[0], [0.; 4]);
}
#[test]
fn xcf_negative_offsets_opacity_once() {
    let mut l = Layer::solid("offset", 2, 2, [255, 0, 0, 255], 1);
    l.xy = (-1, -1);
    l.props.push((33, 0.25f32.to_be_bytes().to_vec()));
    let b = fixture(3, 3, 12, 150, 0, &[l], None);
    let d = Document::parse(&b).unwrap();
    assert_eq!(d.layer_pixels(0, false).unwrap().data[0][3], 1.);
    let p = d.composite().unwrap();
    assert_eq!(p.data[0], [1., 0., 0., 0.25]);
    assert_eq!(p.data[1], [0.; 4]);
}
#[test]
fn xcf_sixteen_bit_and_linear_samples() {
    for precision in [200, 250] {
        for compression in 0..=2 {
            let mut l = Layer::solid("16", 1, 1, [0; 4], 1);
            l.rgba[0] = [32768, 12345, 0, 12345];
            let b = fixture(1, 1, 12, precision, compression, &[l], Some(zip));
            let d = Document::parse(&b).unwrap();
            let p = d.layer_pixels(0, false).unwrap().data[0];
            let half = 32768. / 65535.;
            assert!((p[0] - if precision == 200 { effectcraft_color::linear_to_srgb(half) } else { half }).abs() < 1e-6);
            assert!((p[3] - 12345. / 65535.).abs() < 1e-6);
        }
    }
}
#[test]
fn xcf_linear_composite() {
    let mut l = Layer::solid("white", 1, 1, [255, 255, 255, 128], 1);
    l.props.push((7, 28u32.to_be_bytes().to_vec()));
    let b = fixture(1, 1, 12, 150, 0, &[l, Layer::solid("black", 1, 1, [0, 0, 0, 255], 2)], None);
    let d = Document::parse(&b).unwrap();
    assert!((d.composite().unwrap().data[0][0] - effectcraft_color::linear_to_srgb(128. / 255.)).abs() < 1e-6);
}
#[test]
fn xcf_identity_and_ambiguous_names() {
    let b = support::sample();
    let d = Document::parse(&b).unwrap();
    assert_eq!(d.find_layer(Some(13), "Old").unwrap(), 2);
    assert!(d.find_layer(Some(999), "Ink").is_err());
    let b = fixture(1, 1, 12, 150, 0, &[Layer::solid("same", 1, 1, [0; 4], 1), Layer::solid("same", 1, 1, [0; 4], 2)], None);
    let d = Document::parse(&b).unwrap();
    assert!(d.find_layer(None, "same").is_err());
    assert_eq!(d.find_layer(Some(2), "same").unwrap(), 1);
}
#[test]
fn xcf_rejects_unsupported_features() {
    for ty in [5, 29, 30, 43, 44, 45] {
        let mut l = Layer::solid("bad", 1, 1, [0; 4], 1);
        l.props.push((ty, vec![]));
        assert!(Document::parse(&fixture(1, 1, 23, 150, 0, &[l], None)).is_err());
    }
    for (ty, v) in [(7, 30u32), (35, 2), (36, 3)] {
        let mut l = Layer::solid("bad", 1, 1, [0; 4], 1);
        l.props.push((ty, v.to_be_bytes().to_vec()));
        assert!(Document::parse(&fixture(1, 1, 23, 150, 0, &[l], None)).is_err());
    }
    for mask in [true, false] {
        let mut l = Layer::solid("bad", 1, 1, [0; 4], 1);
        l.mask = mask;
        l.effect = !mask;
        assert!(Document::parse(&fixture(1, 1, 23, 150, 0, &[l], None)).is_err());
    }
    for precision in [300, 500, 600] {
        assert!(Document::parse(&fixture(1, 1, 12, precision, 0, &[Layer::solid("bad", 1, 1, [0; 4], 1)], None)).is_err());
    }
    let mut b = support::sample();
    b[9..13].copy_from_slice(b"v027");
    assert!(Document::parse(&b).is_err());
}
#[test]
fn xcf_every_truncated_prefix() {
    let b = support::sample();
    for n in 0..b.len() {
        if let Ok(d) = Document::parse(&b[..n]) {
            assert!(d.validate_pixels().is_err(), "prefix {n}");
        }
    }
}
#[test]
fn xcf_hostile_dimensions_rle_and_mutations() {
    let mut b = support::sample();
    b[14..18].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(Document::parse(&b).is_err());
    let mut l = Layer::solid("nan", 1, 1, [0; 4], 1);
    l.props.push((33, f32::NAN.to_be_bytes().to_vec()));
    assert!(Document::parse(&fixture(1, 1, 12, 150, 0, &[l], None)).is_err());
    let mut b = fixture(1, 1, 12, 150, 1, &[Layer::solid("bad", 1, 1, [0; 4], 1)], None);
    let n = b.len();
    b[n - 8] = 126;
    assert!(Document::parse(&b).unwrap().validate_pixels().is_err());
    let b = fixture(1, 1, 12, 150, 0, &[Layer::solid("fuzz", 1, 1, [0; 4], 1)], None);
    for i in 0..b.len() {
        let mut b = b.clone();
        b[i] ^= 255;
        if let Ok(d) = Document::parse(&b) {
            let _ = d.composite();
        }
    }
}
#[test]
fn xcf_zip_limit() {
    fn oversized(_: &[u8]) -> Vec<u8> {
        zip(&[0; 8192])
    }
    let b = fixture(1, 1, 12, 150, 2, &[Layer::solid("bad", 1, 1, [0; 4], 1)], Some(oversized));
    assert!(Document::parse(&b).unwrap().validate_pixels().is_err());
}
#[test]
fn xcf_native_file_limit() {
    let path = std::env::temp_dir().join(format!("xcf-limit-{}.xcf", std::process::id()));
    let f = std::fs::File::create(&path).unwrap();
    f.set_len(effectcraft_xcf::MAX_FILE_BYTES as u64 + 1).unwrap();
    drop(f);
    assert!(effectcraft_xcf::read_file(&path).is_err());
    std::fs::remove_file(path).unwrap();
}
#[test]
fn xcf_rgb_no_alpha() {
    let mut l = Layer::solid("RGB", 1, 1, [80, 100, 120, 0], 1);
    l.channels = 3;
    let b = fixture(1, 1, 3, 150, 1, &[l], None);
    assert_eq!(Document::parse(&b).unwrap().composite().unwrap().data[0], [80. / 255., 100. / 255., 120. / 255., 1.]);
}
#[test]
fn xcf_oracle_dump() {
    if let Ok(dir) = std::env::var("XCF_ORACLE_DIR") {
        let mut l = Layer::solid("Top", 65, 67, [255, 0, 0, 128], 1);
        l.xy = (-1, 2);
        let b = fixture(80, 80, 3, 150, 1, &[l, Layer::solid("Bottom", 80, 80, [0, 0, 255, 255], 2)], None);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join("plain.xcf"), &b).unwrap();
        let p = Document::parse(&b).unwrap().composite().unwrap();
        let raw: Vec<_> = p.data.iter().flat_map(|p| p.map(|v| (v * 255.).round() as u8)).collect();
        std::fs::write(std::path::Path::new(&dir).join("expected.rgba"), raw).unwrap();
    }
}

#[test]
fn xcf_normal_auto_blend_space() {
    for mode in [0u32, 28] {
        let mut layer = Layer::solid("plain", 2, 2, [255, 0, 0, 255], 2);
        layer.props.extend([
            (7, mode.to_be_bytes().to_vec()),
            (37, 0u32.to_be_bytes().to_vec()),
            (36, (-1i32).to_be_bytes().to_vec()),
            (35, (-1i32).to_be_bytes().to_vec()),
        ]);
        let bytes = fixture(2, 2, 11, 150, 1, &[layer], None);
        let doc = Document::parse(&bytes).unwrap();
        doc.validate_pixels().unwrap();
        assert_eq!(doc.layers.len(), 1);
        assert_eq!(doc.layer_pixels(0, false).unwrap().data[0], [1., 0., 0., 1.]);
    }
}
