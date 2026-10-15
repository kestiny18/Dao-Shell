//! Desktop name-discovery boundary. Discovery never grants file-operation authority.
use crate::{core::Cancellation, files::Search};
use anyhow::Result;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Candidate {
    pub path: PathBuf,
    pub volume: u64,
    pub file_id: u64,
    pub mount: Option<(String, String)>,
}
impl Candidate {
    pub fn check_mapping(&self) -> Result<()> {
        anyhow::ensure!(self.path.is_absolute(), "candidate path must be absolute");
        #[cfg(windows)]
        if let Some((mount, guid)) = &self.mount {
            windows::check_mapping(mount, guid, self.volume)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Default)]
pub struct VolumeReceipt {
    pub volume: String,
    pub generation: Option<u64>,
    pub updated_unix_ms: Option<u128>,
    pub complete: bool,
    pub phase: String,
    pub reasons: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Default)]
pub struct Receipt {
    pub volumes: Vec<VolumeReceipt>,
    pub complete: bool,
    pub reasons: Vec<String>,
}
#[derive(Default)]
pub struct Page {
    pub candidates: Vec<Candidate>,
    pub receipt: Receipt,
    pub has_more: bool,
}
pub trait IndexClient: Send + Sync {
    fn search(&self, request: &Search, cancel: &Cancellation) -> Result<Page>;
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::WindowsServiceIndexClient;
#[cfg(windows)]
pub mod install;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        files::{self, Objects, Scope},
        platform,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Mock {
        paths: Vec<PathBuf>,
        calls: Arc<AtomicUsize>,
        fail: bool,
        wrong_identity: bool,
    }
    impl IndexClient for Mock {
        fn search(&self, _: &Search, cancel: &Cancellation) -> Result<Page> {
            cancel.check()?;
            self.calls.fetch_add(1, Ordering::Relaxed);
            anyhow::ensure!(!self.fail, "service disconnected");
            let candidates = self
                .paths
                .iter()
                .map(|path| {
                    let identity = platform::identity(path)?;
                    Ok(Candidate {
                        path: path.clone(),
                        volume: identity.volume,
                        file_id: identity.index + u64::from(self.wrong_identity),
                        mount: None,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Page {
                candidates,
                receipt: Receipt {
                    complete: true,
                    ..Default::default()
                },
                has_more: false,
            })
        }
    }
    fn query(value: serde_json::Value) -> Search {
        serde_json::from_value(value).unwrap()
    }
    #[test]
    fn global_names_do_not_grant_scope_or_other_session_authority() {
        let fixture = tempfile::tempdir().unwrap();
        let allowed = fixture.path().join("allowed");
        let outside = fixture.path().join("outside");
        std::fs::create_dir(&allowed).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let path = outside.join("合同.txt");
        std::fs::write(&path, "fixture").unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let client = Arc::new(Mock {
            paths: vec![path.clone()],
            calls: calls.clone(),
            fail: false,
            wrong_identity: false,
        });
        let scope = Scope::new(&[allowed], &[]).unwrap();
        let mut first = Objects::default();
        first.set_index(Some(client.clone()));
        let result = files::search(
            &query(serde_json::json!({"query":"合同"})),
            &scope,
            &mut first,
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(result.source, "windows_service");
        assert_eq!(result.items.len(), 1);
        assert!(first.checked(&result.items[0].id, &scope, false).is_err());
        assert!(first.checked(&result.items[0].id, &scope, true).is_err());
        let mut second = Objects::default();
        second.set_index(Some(client));
        assert!(second.get(&result.items[0].id).is_err());
        let empty_scope = Scope::new(&[], &[]).unwrap();
        assert_eq!(
            files::search(
                &query(serde_json::json!({"query":"合同"})),
                &empty_scope,
                &mut second,
                &Cancellation::default()
            )
            .unwrap()
            .items
            .len(),
            1
        );
        first.reset();
        assert!(first.get(&result.items[0].id).is_err());
        assert!(first.has_index());
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }
    #[test]
    fn complex_filters_preserve_terms_path_type_and_size_in_explicit_roots() {
        let fixture = tempfile::tempdir().unwrap();
        let parent = fixture.path().join("客户端");
        std::fs::create_dir(&parent).unwrap();
        std::fs::write(parent.join("初始化.sql"), "fixture content").unwrap();
        std::fs::write(parent.join("初始化.txt"), "fixture content").unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let mut objects = Objects::default();
        objects.set_index(Some(Arc::new(Mock {
            paths: vec![],
            calls: calls.clone(),
            fail: false,
            wrong_identity: false,
        })));
        let request = query(
            serde_json::json!({"terms":["客户端","初始化"],"kind":"file","extension":"sql","min_bytes":5,"sort":"name"}),
        );
        let scope = Scope::new(&[fixture.path().to_owned()], &[]).unwrap();
        let result =
            files::search(&request, &scope, &mut objects, &Cancellation::default()).unwrap();
        assert_eq!(result.source, "scan");
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].path.file_name().unwrap(), "初始化.sql");
        assert!(result.coverage_note.contains("非全盘"));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        let error = files::search(
            &request,
            &Scope::new(&[], &[]).unwrap(),
            &mut objects,
            &Cancellation::default(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("当前筛选需要目录补充"));
    }
    #[test]
    fn disconnected_service_falls_back_only_to_roots_and_cancel_never_falls_back() {
        let fixture = tempfile::tempdir().unwrap();
        std::fs::write(fixture.path().join("fixture.txt"), "data").unwrap();
        let scope = Scope::new(&[fixture.path().to_owned()], &[]).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let mut objects = Objects::default();
        objects.set_index(Some(Arc::new(Mock {
            paths: vec![],
            calls: calls.clone(),
            fail: true,
            wrong_identity: false,
        })));
        let request = query(serde_json::json!({"query":"fixture"}));
        let result =
            files::search(&request, &scope, &mut objects, &Cancellation::default()).unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.roots, scope.read_roots());
        assert!(result.coverage_note.contains("服务当前不可用"));
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(files::search(&request, &scope, &mut objects, &cancel).is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let error = files::search(
            &request,
            &Scope::new(&[], &[]).unwrap(),
            &mut objects,
            &Cancellation::default(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("全盘索引服务当前不可用"));
    }
    #[test]
    fn changed_candidate_identity_is_skipped_and_coverage_is_not_complete() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.txt");
        std::fs::write(&path, "data").unwrap();
        let mut objects = Objects::default();
        objects.set_index(Some(Arc::new(Mock {
            paths: vec![path],
            calls: Arc::new(AtomicUsize::new(0)),
            fail: false,
            wrong_identity: true,
        })));
        let result = files::search(
            &query(serde_json::json!({"query":"fixture"})),
            &Scope::new(&[], &[]).unwrap(),
            &mut objects,
            &Cancellation::default(),
        )
        .unwrap();
        assert!(result.items.is_empty());
        assert!(result.truncated);
        assert_eq!(
            result.index_receipt.unwrap().reasons,
            vec!["candidate_changed"]
        );
    }
}
