//! Features intentionally unavailable in this fork. Legacy project data is retained.

pub fn removed_command(id: &str) -> bool {
    id.starts_with("contentFill.")
        || matches!(id, "layer.newContentAwareFill" | "mask.interpolate" | "mask.interpolationOptions")
        || id.starts_with("track.")
        || id.starts_with("warp.")
        || id.starts_with("comp.vr.")
        || id.starts_with("mediaBrowser.")
        || id.starts_with("learn.")
        || id.starts_with("roto.")
        || id.starts_with("face.")
        || matches!(
            id,
            "file.openDemoProject"
                | "track.camera"
                | "track.extractFaceMeasurements"
                | "camera.analyze"
                | "camera.cancel"
                | "camera.solveStatus"
                | "camera.points"
                | "camera.selectPoints"
                | "camera.createFromSolve"
                | "camera.create"
                | "camera.setGroundPlane"
                | "camera.deletePoints"
                | "help.inAppTutorials"
                | "help.onlineTutorials"
                | "help.discord"
                | "help.website"
                | "help.appPage"
                | "help.github"
                | "help.reportIssue"
                | "help.sibling"
        )
}

pub fn legacy_effect(id: &str) -> bool {
    effectcraft_effects::legacy_only(id)
}

pub fn project_uses_legacy_features(project: &effectcraft_project::Project) -> bool {
    project.comps().any(|(_, c)| c.layers.iter().any(|l| l.effects().is_some_and(|fx| fx.groups().any(|g| legacy_effect(&g.match_id)))))
}

pub fn legacy_instances(project: &effectcraft_project::Project) -> Vec<(u64, u64, u64)> {
    project
        .comps()
        .flat_map(|(cid, c)| {
            c.layers.iter().flat_map(move |l| {
                l.effects().into_iter().flat_map(move |fx| fx.groups().filter(|g| legacy_effect(&g.match_id)).map(move |g| (cid.0, l.id.0, g.uid)))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn removed_features_are_not_commands_and_cannot_execute() {
        let mut s = crate::Session::default();
        for id in [
            "contentFill.set",
            "contentFill.generate",
            "layer.newContentAwareFill",
            "mask.interpolate",
            "mask.interpolationOptions",
            "learn.start",
            "file.openDemoProject",
            "help.discord",
            "track.camera",
            "comp.vr.createEnvironment",
            "comp.vr.extractCubemap",
            "comp.vr.environments",
            "comp.vr.setView",
            "mediaBrowser.list",
            "mediaBrowser.go",
            "mediaBrowser.import",
            "track.motion",
            "track.stabilize",
            "track.mask",
            "track.warpStabilizer",
            "camera.analyze",
            "roto.stroke",
            "roto.models",
            "face.models",
            "track.extractFaceMeasurements",
        ] {
            assert!(crate::commands::find(id).is_none(), "{id}");
            assert!(s.execute(id, json!({})).is_err(), "{id}");
            assert!(s.execute_checked(id, json!({})).is_err(), "{id}");
        }
        for id in [
            "layer.addMask",
            "keys.wiggle",
            "keys.smooth",
            "motion.sketch",
            "file.import",
            "camera.fromView",
            "camera.stereoRig",
            "help.docs",
            "file.newFromTemplate",
        ] {
            assert!(crate::commands::find(id).is_some(), "{id}");
        }
        for name in ["faceOutline", "faceDetailed", "Face Tracking (Outline Only)"] {
            assert!(s.execute("track.maskMethod", json!({"method": name})).is_err());
        }
        assert!(s.execute("track.maskMethod", json!({"method": "perspective"})).is_err());
    }

    #[test]
    fn effects_are_hidden_but_legacy_data_can_be_read() {
        for id in
            ["ec.perspective.cameratracker", "ec.matte.rotobrush", "ec.utility.facetrackpoints", "ec.utility.facemeasurements", "ec.distort.warpstabilizer"]
        {
            assert!(effectcraft_effects::all().iter().all(|e| e.id != id));
            assert!(effectcraft_effects::find(id).is_some());
        }
    }
}
