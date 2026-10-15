//! Ordinary-token fixture adapter for IPC tests. This is not the production USN backend.
use super::super::*;
use anyhow::Context;
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};
#[derive(Serialize, Deserialize)]
struct Snapshot {
    root: String,
    entries: BTreeMap<String, Vec<(String, String)>>,
}
pub(super) struct FixtureSource {
    root: PathBuf,
    root_id: String,
    metadata: windows::WindowsVolumeSource,
    current: Snapshot,
}
impl FixtureSource {
    pub fn open(root: &Path) -> Result<Self> {
        ensure!(
            root.is_absolute() && root.is_dir(),
            "fixture root must be existing absolute directory"
        );
        for path in root.ancestors() {
            use std::os::windows::fs::MetadataExt;
            ensure!(
                std::fs::symlink_metadata(path)?.file_attributes() & 0x400 == 0,
                "reparse fixture root"
            );
        }
        let drive = root
            .to_str()
            .context("non Unicode fixture")?
            .get(..2)
            .context("fixture drive required")?;
        let metadata = windows::WindowsVolumeSource::open_metadata(drive)?;
        let root_id = metadata.id_for_path(root)?;
        let mut source = Self {
            root: root.into(),
            root_id: root_id.clone(),
            metadata,
            current: Snapshot {
                root: root_id,
                entries: BTreeMap::new(),
            },
        };
        source.current = source.scan()?;
        Ok(source)
    }
    fn scan(&self) -> Result<Snapshot> {
        let mut entries: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for entry in walkdir::WalkDir::new(&self.root)
            .follow_links(false)
            .max_depth(32)
        {
            let entry = entry?;
            if entry.depth() == 0 {
                continue;
            }
            ensure!(entries.len() < 2000, "fixture limit exceeded");
            let path = entry.path();
            let id = self.metadata.id_for_path(path)?;
            let parent = self
                .metadata
                .id_for_path(path.parent().context("fixture parent missing")?)?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .context("fixture non Unicode name")?
                .to_owned();
            entries.entry(id).or_default().push((parent, name));
        }
        for names in entries.values_mut() {
            names.sort();
        }
        Ok(Snapshot {
            root: self.root_id.clone(),
            entries,
        })
    }
}
impl VolumeSource for FixtureSource {
    fn identity(&self) -> VolumeIdentity {
        VolumeIdentity {
            backend: "windows-fixture-poll-v1".into(),
            stable_id: self.root_id.clone(),
        }
    }
    fn root(&self) -> String {
        self.root_id.clone()
    }
    fn checkpoint(&mut self) -> Result<Checkpoint> {
        Ok(Checkpoint(serde_json::to_string(&self.current)?))
    }
    fn validate(&mut self, p: &Checkpoint) -> Result<()> {
        let p: Snapshot = serde_json::from_str(&p.0)?;
        ensure!(
            p.root == self.root_id,
            "rebuild_required: fixture root changed"
        );
        Ok(())
    }
    fn enumerate(&mut self, _: Option<&str>) -> Result<Enumeration> {
        Ok(Enumeration {
            ids: self.current.entries.keys().cloned().collect(),
            next: None,
        })
    }
    fn changes(&mut self, p: &Checkpoint) -> Result<Changes> {
        self.validate(p)?;
        let old: Snapshot = serde_json::from_str(&p.0)?;
        self.current = self.scan()?;
        let next = self.checkpoint()?;
        let ids = if next == *p {
            Vec::new()
        } else {
            old.entries
                .keys()
                .chain(self.current.entries.keys())
                .cloned()
                .collect::<HashSet<_>>()
                .into_iter()
                .collect()
        };
        Ok(Changes {
            ids,
            next,
            caught_up: true,
        })
    }
    fn resolve(&mut self, id: &str) -> Result<Resolved> {
        if id == self.root_id {
            return Ok(Resolved::Present(Node {
                id: id.into(),
                directory: true,
                names: Vec::new(),
            }));
        }
        let Some(names) = self.current.entries.get(id) else {
            return Ok(Resolved::Deleted);
        };
        let result = self.metadata.resolve(id)?;
        match result {
            Resolved::Present(mut node) => {
                node.names = names
                    .iter()
                    .map(|(p, n)| Name {
                        parent: p.clone(),
                        name: n.clone(),
                    })
                    .collect();
                Ok(Resolved::Present(node))
            }
            other => Ok(other),
        }
    }
}
