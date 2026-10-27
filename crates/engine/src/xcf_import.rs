//! Plain XCF layers remain linked to their original source file.
use effectcraft_color::Label;
use effectcraft_keyframe::Value;
use effectcraft_project::{AlphaMode, Comp, Footage, FootageKind, ItemKind, LayerSource, Project, SourceLayer, build};
use effectcraft_time::{FrameRate, Tick};
use std::sync::Arc;
pub(crate) fn import(
    proj: &mut Project,
    doc: &effectcraft_xcf::Document<'_>,
    path: &str,
    name: &str,
    retain: bool,
    rate: FrameRate,
    duration: Tick,
) -> crate::psd_import::PsdImport {
    let folder = proj.add_item(&format!("{name} Layers"), Label::Yellow, None, ItemKind::Folder);
    let mut comp = Comp::new(doc.width, doc.height, rate, duration);
    let mut items = vec![];
    let mut warnings = vec![];
    let pipe = effectcraft_render::color::Pipe::of(&proj.settings);
    if doc.layers.iter().any(|l| l.visible && l.linear_composite != (pipe.linear || pipe.linear_blend)) {
        warnings.push("XCF layers imported. GIMP's compositing space differs from this project's colour settings; translucent overlaps may differ. Use matching Project Settings (Blend Colors Using 1.0 Gamma for linear compositing), or import as Footage for a flattened image.".into());
    }
    for (index, source) in doc.layers.iter().enumerate() {
        let size = if retain { (source.width, source.height) } else { (doc.width, doc.height) };
        let footage = Footage {
            path: path.into(),
            kind: FootageKind::Still,
            width: size.0,
            height: size.1,
            pixel_aspect: 1.,
            frame_rate: rate,
            has_video: true,
            alpha: AlphaMode::Straight,
            loop_count: 1,
            codec: "XCF".into(),
            layer: Some(SourceLayer { index: index as u32, name: source.name.clone(), layer_size: retain, xcf_tattoo: source.tattoo, ..Default::default() }),
            ..Default::default()
        };
        let label = if source.name.is_empty() { format!("Layer {}", index + 1) } else { source.name.clone() };
        let item = proj.add_item(&label, Label::Lavender, Some(folder), ItemKind::Footage(footage));
        items.push(item);
        let name = comp.unique_layer_name(&label);
        let mut l = build::layer(proj, &comp, &name, LayerSource::Footage { item }, size, None);
        l.switches.video = source.visible;
        if retain && let Some(p) = l.props.prop_mut("transform/position") {
            let v = [f64::from(source.x) + f64::from(size.0) / 2., f64::from(source.y) + f64::from(size.1) / 2.];
            p.value = match p.value {
                Value::Vec3(old) => Value::Vec3([v[0], v[1], old[2]]),
                _ => Value::Vec2(v),
            };
        }
        if let Some(p) = l.props.prop_mut("transform/opacity") {
            p.value = Value::Scalar(f64::from(source.opacity) * 100.);
        }
        comp.layers.push(l);
    }
    let comp = proj.add_item(name, Label::Sandstone, None, ItemKind::Comp(Arc::new(comp)));
    items.push(comp);
    crate::psd_import::PsdImport { comp, folder, items, warnings }
}
