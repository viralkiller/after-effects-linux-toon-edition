//! Footage that changed on disk: File ▸ Reload Footage (Cmd+Alt+L) and Settings ▸ Import ▸
//! Reload Footage Changed on Disk.
//!
//! Reloading re-reads what the file decides (its size, a movie's duration, streams and frame
//! rate, the codec, a data file's text) and keeps what the project decided: the Photoshop layer
//! or PDF page an item shows and its interpretation (alpha, conformed frame rate, pixel aspect,
//! fields, colour). A Photoshop layer is found again by name when layers were added or removed
//! around it. The media layer then forgets the file and the layer cache is emptied, so the next
//! render reads the new pixels.
//!
//! The automatic part compares each footage file's size and modification time with what they
//! were when the file was last seen. The desktop app takes those stamps every two seconds (and
//! when its window comes back to the front) on a worker thread, so a file on a slow network share
//! never holds up the interface ([`start_scan`], [`poll_scan`]); `footage.reloadChanged` takes them
//! on the spot (CLI, agents, tests). Like the footage check after File ▸ Open, an automatic reload
//! records the state of the disk: it is not an undo step and doesn't modify the project.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use effectcraft_project::{Footage, FootageKind, ItemId, ItemKind, SourceLayer};
use serde_json::{Value, json};

use crate::Session;

/// A file's size and modification time (see [`crate::Services::stamp`]).
pub type Stamp = (u64, u128);

/// Stamps taken on a worker thread, waiting for [`poll_scan`].
pub type Scan = Arc<Mutex<Option<Vec<(String, Option<Stamp>)>>>>;

/// The files footage items read: each item's file and every frame of an image sequence. Image
/// sequences count only with Automatically Reload Footage set to All Footage (After Effects'
/// default, Non-Sequence Footage, leaves them alone: thousands of frames on a share add up).
fn watched(s: &Session) -> BTreeSet<String> {
    let sequences = s.prefs.import.auto_reload_footage == "all";
    let mut paths = BTreeSet::new();
    for it in s.project.items.values() {
        if let ItemKind::Footage(f) = &it.kind {
            if !f.sequence.is_empty() && !sequences {
                continue;
            }
            if !f.path.is_empty() {
                paths.insert(f.path.clone());
            }
            paths.extend(f.sequence.iter().cloned());
        }
    }
    paths
}

/// Size and position in the stack of Photoshop layer `l` in the file at `path` now: the layer
/// at its recorded index when the name still matches, else the first layer with its name.
fn psd_layer(s: &Session, path: &str, l: &SourceLayer) -> Option<(u32, Option<(u32, u32)>)> {
    let psd = effectcraft_psd::Psd::parse(s.services.read_file(path).ok()?).ok()?;
    let found = psd.layers.get(l.index as usize).filter(|x| x.name == l.name).or_else(|| psd.layers.iter().find(|x| x.name == l.name))?;
    let size = (!found.rect.is_empty()).then(|| (found.rect.width(), found.rect.height()));
    Some((u32::try_from(found.index).ok()?, size))
}

/// Footage `f` as its file is now, given the importer's fresh look at the file (`probed`).
fn refreshed(s: &Session, f: &Footage, probed: Footage) -> Footage {
    let mut nf = f.clone();
    nf.missing = false;
    // An image sequence picks up frames added to (or removed from) its run; its frame rate and
    // order are the interpretation's.
    if f.kind == FootageKind::Sequence {
        if probed.kind == FootageKind::Sequence {
            (nf.sequence, nf.width, nf.height) = (probed.sequence, probed.width, probed.height);
            nf.sync_sequence_duration();
        }
        return nf;
    }
    nf.codec = probed.codec;
    nf.has_video = probed.has_video;
    nf.has_audio = probed.has_audio;
    if matches!(f.kind, FootageKind::Video | FootageKind::Audio) {
        nf.duration = probed.duration;
        // Interpret Footage ▸ Conform to frame rate stays; the file's own rate is refreshed.
        match f.native_rate {
            Some(_) => nf.native_rate = Some(probed.native_rate.unwrap_or(probed.frame_rate)),
            None => nf.frame_rate = probed.frame_rate,
        }
    }
    if f.kind == FootageKind::Data {
        nf.data = probed.data;
    }
    let (mut w, mut h) = (probed.width, probed.height);
    match &f.layer {
        // A smart object's size is its embedded file's, recorded at import.
        Some(l) if l.embedded.is_some() => (w, h) = (f.width, f.height),
        Some(l) => match psd_layer(s, &f.path, l) {
            Some((index, size)) => {
                if let Some(nl) = nf.layer.as_mut() {
                    nl.index = index;
                }
                // Retain Layer Sizes: the layer's own bounds; otherwise the document's.
                if l.layer_size {
                    (w, h) = size.unwrap_or((f.width, f.height));
                }
            }
            // The layer is gone from the file: keep the item as it was.
            None => (w, h) = (f.width, f.height),
        },
        // A later page of a PDF: the probe measures the first page.
        None if f.page != 0 => (w, h) = (f.width, f.height),
        None => {}
    }
    (nf.width, nf.height) = (w, h);
    nf
}

/// Reload footage items `ids` from their files (File ▸ Reload Footage): an undo step; items
/// whose file can't be read are marked missing. Returns how many items were reloaded.
pub fn reload_items(s: &mut Session, ids: &[ItemId]) -> crate::Result<usize> {
    let ids = layered_reload_items(s, ids);
    Ok(reload(s, &ids, true)?.0)
}

/// Expand a layered source to all its imported items, including merged footage.
/// Match complete stored paths: identically named files in other folders stay separate.
/// Existing timeline layers and animation are not rebuilt.
fn layered_reload_items(s: &Session, ids: &[ItemId]) -> Vec<ItemId> {
    let paths: BTreeSet<&str> = ids
        .iter()
        .filter_map(|id| {
            let ItemKind::Footage(f) = &s.project.item(*id)?.kind else { return None };
            let ext = std::path::Path::new(&f.path).extension().and_then(|v| v.to_str()).unwrap_or("");
            let layered =
                ["psd", "psb", "xcf"].iter().any(|v| ext.eq_ignore_ascii_case(v)) || ["PSD", "PSB", "XCF"].iter().any(|v| f.codec.eq_ignore_ascii_case(v));
            (layered && !f.path.is_empty()).then_some(f.path.as_str())
        })
        .collect();
    let mut expanded: BTreeSet<ItemId> = ids.iter().copied().collect();
    for item in s.project.items.values() {
        if let ItemKind::Footage(f) = &item.kind
            && paths.contains(f.path.as_str())
        {
            expanded.insert(item.id);
        }
    }
    expanded.into_iter().collect()
}

#[cfg(test)]
mod layered_reload_tests {
    use super::*;
    use effectcraft_color::Label;

    fn add(s: &mut Session, path: &str, codec: &str, layer: Option<SourceLayer>) -> ItemId {
        Arc::make_mut(&mut s.project).add_item(
            "source",
            Label::Lavender,
            None,
            ItemKind::Footage(Footage { path: path.into(), codec: codec.into(), kind: FootageKind::Still, layer, ..Default::default() }),
        )
    }

    #[test]
    fn selecting_one_layer_reloads_all_imported_items_of_its_document() {
        for (ext, codec) in [("psd", "PSD"), ("PSB", "PSB"), ("xcf", "XCF")] {
            let mut s = Session::default();
            let path = format!("/shots/character.{ext}");
            let first = add(&mut s, &path, codec, Some(SourceLayer { index: 0, name: "front".into(), ..Default::default() }));
            let second = add(&mut s, &path, codec, Some(SourceLayer { index: 1, name: "back".into(), ..Default::default() }));
            let merged = add(&mut s, &path, codec, None);
            let other = add(&mut s, &format!("/other/character.{ext}"), codec, None);
            let before = s.project.to_json();
            let ids = layered_reload_items(&s, &[first, first]);
            assert_eq!(ids, vec![first, second, merged]);
            assert!(!ids.contains(&other));
            assert_eq!(layered_reload_items(&s, &[merged]), ids);
            assert_eq!(s.project.to_json(), before);
        }
    }

    #[test]
    fn ordinary_footage_and_empty_selections_are_not_expanded() {
        let mut s = Session::default();
        let first = add(&mut s, "/shots/plain.png", "PNG", None);
        add(&mut s, "/shots/plain.png", "PNG", None);
        assert_eq!(layered_reload_items(&s, &[first]), vec![first]);
        assert!(layered_reload_items(&s, &[]).is_empty());
    }
}


fn is_xcf(f: &Footage) -> bool {
    f.codec.eq_ignore_ascii_case("XCF") || f.path.to_ascii_lowercase().ends_with(".xcf")
}

type XcfAddition = (String, Option<ItemId>, Footage);

/// Prepare a whole document from one validated snapshot. Never rebuild compositions.
fn xcf_plan(s: &Session, path: &str) -> crate::Result<(Vec<(ItemId, Footage)>, Vec<XcfAddition>)> {
    let fail = |e: String| crate::EngineError::Other(format!("XCF reload {path}: {e}"));
    let before = s.services.stamp(path);
    let bytes = s.services.read_file(path).map_err(|e| fail(e.to_string()))?;
    let doc = effectcraft_xcf::Document::parse(&bytes).map_err(|e| fail(e.to_string()))?;
    doc.validate_pixels().map_err(|e| fail(e.to_string()))?;
    if s.services.stamp(path) != before {
        return Err(fail("file changed while being read; retry after GIMP finishes saving".into()));
    }
    // Legacy references without tattoos must be unambiguous; never guess by index.
    let mut updates = Vec::new();
    let mut groups: std::collections::BTreeMap<(Option<ItemId>, bool), (Footage, BTreeSet<usize>)> = Default::default();
    for item in s.project.items.values() {
        let ItemKind::Footage(f) = &item.kind else { continue };
        if f.path != path { continue; }
        let mut nf = f.clone();
        nf.codec = "XCF".into();
        nf.missing = false;
        if let Some(reference) = &f.layer {
            if reference.xcf_tattoo.is_none() && doc.layers.iter().filter(|l| l.name == reference.name).count() > 1 {
                return Err(fail(format!("ambiguous layer name '{}'; give it a unique name in GIMP", reference.name)));
            }
            let group = groups.entry((item.parent, reference.layer_size)).or_insert_with(|| (f.clone(), BTreeSet::new()));
            match doc.find_layer(reference.xcf_tattoo, &reference.name) {
                Ok(index) => {
                    let Some(source) = doc.layers.get(index) else { return Err(fail("invalid layer index".into())) };
                    group.1.insert(index);
                    if let Some(layer) = &mut nf.layer {
                        layer.index = index as u32;
                        layer.name = source.name.clone();
                        layer.xcf_tattoo = source.tattoo;
                    }
                    (nf.width, nf.height) = if reference.layer_size { (source.width, source.height) } else { (doc.width, doc.height) };
                }
                Err(_) => nf.missing = true, // Preserve the old identity, dimensions and animation.
            }
        } else {
            (nf.width, nf.height) = (doc.width, doc.height);
        }
        updates.push((item.id, nf));
    }
    let mut additions = Vec::new();
    for ((parent, retain), (template, represented)) in groups {
        for (index, source) in doc.layers.iter().enumerate() {
            if represented.contains(&index) { continue; }
            if source.tattoo.is_none() && doc.layers.iter().filter(|l| l.name == source.name).count() != 1 {
                return Err(fail(format!("new layer '{}' has no unique identity", source.name)));
            }
            let mut nf = template.clone();
            nf.missing = false;
            nf.codec = "XCF".into();
            (nf.width, nf.height) = if retain { (source.width, source.height) } else { (doc.width, doc.height) };
            nf.layer = Some(SourceLayer { index: index as u32, name: source.name.clone(), xcf_tattoo: source.tattoo, layer_size: retain, ..Default::default() });
            let label = if source.name.is_empty() { format!("Layer {}", index + 1) } else { source.name.clone() };
            additions.push((label, parent, nf));
        }
    }
    Ok((updates, additions))
}

fn add_xcf_items(proj: &mut effectcraft_project::Project, additions: Vec<XcfAddition>) {
    for (name, parent, footage) in additions {
        proj.add_item(&name, effectcraft_color::Label::Lavender, parent, ItemKind::Footage(footage));
    }
}

/// Reload items `ids`. `undoable` makes it an edit; otherwise it records the disk without
/// modifying existing timeline layers. Discovered XCF items mark the project unsaved.
/// An automatic read failure (another app still writing) leaves the document unchanged. Returns the number reloaded and the files that couldn't be read
/// (only without `undoable`).
fn reload(s: &mut Session, ids: &[ItemId], undoable: bool) -> crate::Result<(usize, BTreeSet<String>)> {
    let mut updates = vec![];
    let mut additions = vec![];
    let mut seen_xcf = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut failed = BTreeSet::new();
    for i in ids {
        if let Some(ItemKind::Footage(f)) = s.project.item(*i).map(|x| &x.kind) {
            if is_xcf(f) {
                if seen_xcf.insert(f.path.clone()) {
                    match xcf_plan(s, &f.path) {
                        Ok((items, added)) => {
                            updates.extend(items);
                            additions.extend(added);
                            paths.insert(f.path.clone());
                        }
                        Err(e) if undoable => return Err(e),
                        Err(_) => { failed.insert(f.path.clone()); }
                    }
                }
                continue;
            }
            if s.importer.is_none() {
                return Err(crate::EngineError::Other("media import is not available in this build".into()));
            }
            // Probed the way it imports by default, so a sequence is found as one.
            let nf = match s.probe_footage(&f.path) {
                Ok(p) => refreshed(s, f, p),
                Err(_) if undoable => Footage { missing: true, ..f.clone() },
                Err(_) => {
                    failed.insert(f.path.clone());
                    continue;
                }
            };
            paths.insert(f.path.clone());
            paths.extend(f.sequence.iter().chain(&nf.sequence).cloned());
            updates.push((*i, nf));
        }
    }
    let n = updates.len();
    let discovered = additions.len();
    if undoable {
        s.edit("Reload Footage", None, |proj, _| {
            for (i, f) in updates {
                if let Some(it) = proj.item_mut(i) {
                    it.kind = ItemKind::Footage(f);
                }
            }
            add_xcf_items(proj, additions);
            Ok(())
        })?;
    } else {
        let dirty = s.is_dirty();
        updates.retain(|(id, f)| !matches!(s.project.item(*id).map(|i| &i.kind), Some(ItemKind::Footage(cur)) if cur == f));
        if !updates.is_empty() || discovered > 0 {
            let p = Arc::make_mut(&mut s.project);
            for (id, f) in updates {
                if let Some(item) = p.item_mut(id) { item.kind = ItemKind::Footage(f); }
            }
            add_xcf_items(p, additions);
            s.bump();
            if !dirty && discovered == 0 {
                s.mark_saved();
            }
        }
    }
    forget(s, &paths);
    Ok((n, failed))
}

/// Make the media layer and the layer cache let go of `paths`, so renders read them again.
fn forget(s: &mut Session, paths: &BTreeSet<String>) {
    if paths.is_empty() {
        return;
    }
    for p in paths {
        s.footage.forget(p);
    }
    s.layer_cache.clear();
    // Pixel-only changes leave project metadata identical. Advance the render
    // revision so displayed frames and pending old-revision results are stale.
    // Session::bump preserves the saved state when the project is unchanged.
    s.bump();
    s.events.push(crate::Event::PurgeCaches);
}

/// Compare fresh stamps of the watched files with the ones last seen. A file seen for the first
/// time is remembered (XCF is also synchronized on this first scan); one whose stamp moved is reloaded: the items reading it get their
/// size and duration from it again and its pixels are read again. A file that can't be stamped
/// right now (an app saving by delete and rename) keeps its last stamp, and one that can't be
/// read (still being written) is retried on the next scan for XCF; missing files are left to
/// the footage check. Returns `{files, reloaded}`.
pub fn apply_stamps(s: &mut Session, now: Vec<(String, Option<Stamp>)>) -> crate::Result<Value> {
    let before = std::mem::take(&mut s.footage_stamps);
    let xcf_paths: BTreeSet<String> = s.project.items.values().filter_map(|i| match &i.kind {
        ItemKind::Footage(f) if is_xcf(f) => Some(f.path.clone()),
        _ => None,
    }).collect();
    let mut changed = BTreeSet::new();
    // Only what is watched now is remembered (files no longer used are dropped).
    for (path, stamp) in now {
        match (before.get(&path), stamp) {
            (Some(old), None) => {
                s.footage_stamps.insert(path, *old);
            }
            (old, Some(stamp)) => {
                if old.is_some_and(|o| *o != stamp) || (old.is_none() && xcf_paths.contains(&path)) {
                    changed.insert(path.clone());
                }
                s.footage_stamps.insert(path, stamp);
            }
            (None, None) => {}
        }
    }
    if changed.is_empty() {
        return Ok(json!({"files": 0, "reloaded": 0}));
    }
    let ids: Vec<ItemId> = s.project.items.values().filter(|i| matches!(&i.kind, ItemKind::Footage(f) if changed.contains(&f.path))).map(|i| i.id).collect();
    let (reloaded, failed) = if ids.is_empty() { (0, BTreeSet::new()) } else { reload(s, &ids, false)? };
    // XCF failures retain the previous stamp for retry; other media keeps its current policy.
    for p in &failed {
        changed.remove(p);
        if xcf_paths.contains(p) {
            // Do not acknowledge a failed XCF snapshot: retry even if its stamp stays fixed.
            if let Some(old) = before.get(p) { s.footage_stamps.insert(p.clone(), *old); }
            else { s.footage_stamps.remove(p); }
        }
    }
    if changed.is_empty() {
        return Ok(json!({"files": 0, "reloaded": 0}));
    }
    // Changed frames of image sequences (their items keep their metadata).
    forget(s, &changed);
    let files = changed.len();
    s.toast(if reloaded > 0 {
        format!("Reloaded {reloaded} footage item(s) changed on disk")
    } else {
        format!("Reloaded {files} footage file(s) changed on disk")
    });
    Ok(json!({"files": files, "reloaded": reloaded}))
}

/// `footage.reloadChanged`: stamp the watched files now and reload the ones that changed.
pub(crate) fn command(s: &mut Session, _: &Value) -> crate::Result<Value> {
    let services = s.services.clone();
    let now = watched(s).into_iter().map(|p| (p.clone(), services.stamp(&p))).collect();
    apply_stamps(s, now)
}

/// Start stamping the watched files on a worker thread, unless a scan is still running (or the
/// build has no threads: the web app, whose footage lives in browser storage).
pub fn start_scan(s: &mut Session) {
    if s.footage_scan.is_some() || cfg!(target_arch = "wasm32") {
        return;
    }
    let paths = watched(s);
    if paths.is_empty() {
        return;
    }
    let services = s.services.clone();
    let slot: Scan = Arc::default();
    let out = slot.clone();
    let spawned = std::thread::Builder::new().name("footage-watch".into()).spawn(move || {
        let stamps = paths.into_iter().map(|p| (p.clone(), services.stamp(&p))).collect();
        *out.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(stamps);
    });
    if spawned.is_ok() {
        s.footage_scan = Some(slot);
    }
}

/// Apply a finished scan (`None` while there is none, or it is still running).
pub fn poll_scan(s: &mut Session) -> Option<crate::Result<Value>> {
    let stamps = s.footage_scan.as_ref()?.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take()?;
    s.footage_scan = None;
    Some(apply_stamps(s, stamps))
}

#[cfg(test)]
mod reload_revision_tests {
    use super::*;

    #[test]
    fn pixel_only_reload_advances_revision_without_modifying_saved_project() {
        let mut s = Session::default();
        s.mark_saved();
        let revision = s.revision;
        let project = s.project.to_json();
        let paths = BTreeSet::from(["reload-test.psd".to_string()]);

        forget(&mut s, &paths);

        assert!(s.revision > revision);
        assert_eq!(s.project.to_json(), project);
        assert!(!s.is_dirty());
        assert!(s.events.iter().any(|e| matches!(
            e, crate::Event::ProjectChanged { revision: r } if *r == s.revision
        )));
        assert!(s.events.iter().any(|e| matches!(e, crate::Event::PurgeCaches)));
    }

    #[test]
    fn empty_reload_does_not_advance_revision() {
        let mut s = Session::default();
        let revision = s.revision;
        forget(&mut s, &BTreeSet::new());
        assert_eq!(s.revision, revision);
    }
}
