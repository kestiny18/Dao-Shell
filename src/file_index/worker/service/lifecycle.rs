//! Restart-safe manifest transaction; health is an actual authenticated pipe exchange.
use super::{
    package::Manifest,
    scm::{Controller, ServiceInfo},
};
use anyhow::{Result, ensure};

pub(super) trait Repository {
    fn manifest(&self) -> Result<Manifest>;
    fn save(&mut self, manifest: &Manifest) -> Result<()>;
    fn command(&self, build: &str) -> Result<String>;
    fn verify(&self, manifest: &Manifest, build: &str) -> Result<()>;
    fn health(&self, manifest: &Manifest) -> Result<()>;
}
pub(super) fn activate_new(repo: &impl Repository, scm: &mut impl Controller) -> Result<()> {
    let manifest = repo.manifest()?;
    manifest.validate()?;
    ensure!(scm.inspect()?.is_none(), "service already exists");
    repo.verify(&manifest, &manifest.current)?;
    scm.create(&repo.command(&manifest.current)?)?;
    let result = (|| {
        scm.start()?;
        repo.health(&manifest)
    })();
    if let Err(error) = result {
        // Never delete while a failed start/health attempt might still be running.
        scm.stop()?;
        scm.delete()?;
        return Err(error);
    }
    Ok(())
}
fn trusted(info: &ServiceInfo, repo: &impl Repository, manifest: &Manifest) -> Result<()> {
    ensure!(
        info.account.eq_ignore_ascii_case("LocalSystem"),
        "unexpected service account"
    );
    let current = repo.command(&manifest.current)?;
    let previous = manifest
        .previous
        .as_deref()
        .map(|v| repo.command(v))
        .transpose()?;
    ensure!(
        info.image == current || previous.as_ref() == Some(&info.image),
        "unexpected service image"
    );
    Ok(())
}
pub(super) fn switch(
    repo: &mut impl Repository,
    scm: &mut impl Controller,
    build: &str,
) -> Result<()> {
    let old = repo.manifest()?;
    old.validate()?;
    ensure!(!old.pending, "installation pending: repair required");
    ensure!(
        !old.uninstalled,
        "service was uninstalled: install required"
    );
    repo.verify(&old, build)?;
    if let Some(info) = scm.inspect()? {
        trusted(&info, repo, &old)?;
    } else {
        anyhow::bail!("service missing: repair required");
    }
    // No filesystem or configuration mutation before confirmed stop.
    scm.stop()?;
    let next = Manifest {
        current: build.into(),
        previous: Some(old.current.clone()),
        pending: true,
        recover_to: Some(old.current.clone()),
        ..old.clone()
    };
    repo.save(&next)?;
    let transition = (|| {
        scm.configure(&repo.command(build)?)?;
        scm.start()?;
        repo.health(&next)
    })();
    if let Err(error) = transition {
        // Failed rollback stop must leave pending evidence and both payloads intact.
        scm.stop()?;
        repo.verify(&old, &old.current)?;
        let restoring = Manifest {
            previous: Some(build.into()),
            pending: true,
            recover_to: Some(old.current.clone()),
            ..old.clone()
        };
        repo.save(&restoring)?;
        scm.configure(&repo.command(&old.current)?)?;
        scm.start()?;
        repo.health(&old)?;
        repo.save(&Manifest {
            pending: false,
            recover_to: None,
            ..restoring
        })?;
        return Err(error);
    }
    repo.save(&Manifest {
        pending: false,
        recover_to: None,
        ..next
    })
}
pub(super) fn repair(repo: &mut impl Repository, scm: &mut impl Controller) -> Result<()> {
    let original = repo.manifest()?;
    original.validate()?;
    if let Some(info) = scm.inspect()? {
        trusted(&info, repo, &original)?;
    }
    scm.stop()?;
    let mut target = original.clone();
    if original.pending
        && let Some(recovery) = &original.recover_to
        && recovery != &original.current
    {
        target.current = recovery.clone();
        target.previous = Some(original.current.clone());
    }
    repo.verify(&target, &target.current)?;
    target.pending = true;
    repo.save(&target)?;
    let command = repo.command(&target.current)?;
    if scm.inspect()?.is_some() {
        scm.configure(&command)?;
    } else {
        scm.create(&command)?;
    }
    if let Err(error) = (|| {
        scm.start()?;
        repo.health(&target)
    })() {
        scm.stop()?;
        return Err(error);
    }
    target.pending = false;
    target.recover_to = None;
    target.uninstalled = false;
    repo.save(&target)
}
pub(super) fn start(repo: &impl Repository, scm: &mut impl Controller) -> Result<()> {
    let manifest = repo.manifest()?;
    manifest.validate()?;
    ensure!(
        !manifest.pending && !manifest.uninstalled,
        "installation requires repair or reinstall"
    );
    let info = scm
        .inspect()?
        .ok_or_else(|| anyhow::anyhow!("service not installed"))?;
    trusted(&info, repo, &manifest)?;
    ensure!(
        info.image == repo.command(&manifest.current)?,
        "current service version mismatch"
    );
    repo.verify(&manifest, &manifest.current)?;
    scm.start()?;
    repo.health(&manifest)
}
pub(super) fn stop(repo: &impl Repository, scm: &mut impl Controller) -> Result<()> {
    let manifest = repo.manifest()?;
    manifest.validate()?;
    if let Some(info) = scm.inspect()? {
        trusted(&info, repo, &manifest)?;
    }
    scm.stop()
}
pub(super) fn uninstall(repo: &impl Repository, scm: &mut impl Controller) -> Result<()> {
    let manifest = repo.manifest()?;
    manifest.validate()?;
    if let Some(info) = scm.inspect()? {
        trusted(&info, repo, &manifest)?;
    }
    scm.stop()?;
    scm.delete()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    struct Repo {
        value: Manifest,
        saves: usize,
        fail_health: Cell<bool>,
    }
    impl Repository for Repo {
        fn manifest(&self) -> Result<Manifest> {
            Ok(self.value.clone())
        }
        fn save(&mut self, m: &Manifest) -> Result<()> {
            self.saves += 1;
            self.value = m.clone();
            Ok(())
        }
        fn command(&self, b: &str) -> Result<String> {
            Ok(format!("\"C:\\protected\\{b}\\worker.exe\" service-run"))
        }
        fn verify(&self, _: &Manifest, b: &str) -> Result<()> {
            ensure!(super::super::package::valid_digest(b), "bad digest");
            Ok(())
        }
        fn health(&self, _: &Manifest) -> Result<()> {
            ensure!(!self.fail_health.replace(false), "pipe not healthy");
            Ok(())
        }
    }
    struct Scm {
        info: Option<ServiceInfo>,
        stop_fails: bool,
        configures: usize,
        deleted: bool,
    }
    impl Controller for Scm {
        fn inspect(&self) -> Result<Option<ServiceInfo>> {
            Ok(self.info.clone())
        }
        fn create(&mut self, image: &str) -> Result<()> {
            self.info = Some(ServiceInfo {
                image: image.into(),
                account: "LocalSystem".into(),
                state: 1,
                pid: 0,
                start_type: 2,
            });
            Ok(())
        }
        fn configure(&mut self, image: &str) -> Result<()> {
            self.configures += 1;
            self.info.as_mut().unwrap().image = image.into();
            Ok(())
        }
        fn start(&mut self) -> Result<()> {
            self.info.as_mut().unwrap().state = 4;
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            ensure!(!self.stop_fails, "SCM transition timed out");
            Ok(())
        }
        fn delete(&mut self) -> Result<()> {
            self.deleted = true;
            self.info = None;
            Ok(())
        }
    }
    fn fixture() -> (Repo, Scm) {
        let repo = Repo {
            value: Manifest {
                schema: 1,
                owner_sid: "S-1-5-21-1-2-3-1001".into(),
                current: "A".repeat(64),
                previous: None,
                pending: false,
                recover_to: None,
                uninstalled: false,
            },
            saves: 0,
            fail_health: Cell::new(false),
        };
        let scm = Scm {
            info: Some(ServiceInfo {
                image: repo.command(&repo.value.current).unwrap(),
                account: "LocalSystem".into(),
                state: 4,
                pid: 12,
                start_type: 2,
            }),
            stop_fails: false,
            configures: 0,
            deleted: false,
        };
        (repo, scm)
    }
    #[test]
    fn stop_timeout_never_switches_or_deletes() {
        let (mut r, mut s) = fixture();
        s.stop_fails = true;
        assert!(switch(&mut r, &mut s, &"B".repeat(64)).is_err());
        assert_eq!(r.saves, 0);
        assert_eq!(s.configures, 0);
        assert!(uninstall(&r, &mut s).is_err());
        assert!(!s.deleted);
    }
    #[test]
    fn running_without_pipe_health_rolls_back() {
        let (mut r, mut s) = fixture();
        r.fail_health.set(true);
        assert!(switch(&mut r, &mut s, &"B".repeat(64)).is_err());
        assert_eq!(r.value.current, "A".repeat(64));
        assert!(!r.value.pending);
        assert_eq!(s.configures, 2);
    }
    #[test]
    fn pending_switch_repairs_to_verified_previous() {
        let (mut r, mut s) = fixture();
        r.value.previous = Some(r.value.current.clone());
        r.value.recover_to = r.value.previous.clone();
        r.value.current = "B".repeat(64);
        r.value.pending = true;
        s.info.as_mut().unwrap().image = r.command(&r.value.current).unwrap();
        repair(&mut r, &mut s).unwrap();
        assert_eq!(r.value.current, "A".repeat(64));
        assert!(!r.value.pending);
    }
    #[test]
    fn missing_service_repair_and_unknown_image_denied() {
        let (mut r, mut s) = fixture();
        s.info = None;
        repair(&mut r, &mut s).unwrap();
        assert!(s.info.is_some());
        s.info.as_mut().unwrap().image = "unknown".into();
        let count = r.saves;
        assert!(repair(&mut r, &mut s).is_err());
        assert_eq!(r.saves, count);
    }
    #[test]
    fn failed_initial_health_removes_only_new_service_and_retains_manifest() {
        let (r, mut s) = fixture();
        s.info = None;
        r.fail_health.set(true);
        assert!(activate_new(&r, &mut s).is_err());
        assert!(s.deleted);
        assert!(s.info.is_none());
        assert_eq!(r.value.current, "A".repeat(64));
        assert_eq!(r.saves, 0);
    }
    #[test]
    fn rollback_stop_failure_keeps_repairable_pending_record() {
        let (mut r, mut s) = fixture();
        // Inject a failed health response that also prevents subsequent stop.
        struct StopAfterStart<'a>(&'a mut Scm);
        impl Controller for StopAfterStart<'_> {
            fn inspect(&self) -> Result<Option<ServiceInfo>> {
                self.0.inspect()
            }
            fn create(&mut self, v: &str) -> Result<()> {
                self.0.create(v)
            }
            fn configure(&mut self, v: &str) -> Result<()> {
                self.0.configure(v)
            }
            fn start(&mut self) -> Result<()> {
                self.0.start()?;
                self.0.stop_fails = true;
                Ok(())
            }
            fn stop(&mut self) -> Result<()> {
                self.0.stop()
            }
            fn delete(&mut self) -> Result<()> {
                self.0.delete()
            }
        }
        r.fail_health.set(true);
        assert!(switch(&mut r, &mut StopAfterStart(&mut s), &"B".repeat(64)).is_err());
        assert!(r.value.pending);
        assert_eq!(r.value.recover_to, Some("A".repeat(64)));
        s.stop_fails = false;
        repair(&mut r, &mut s).unwrap();
        assert_eq!(r.value.current, "A".repeat(64));
    }
    #[test]
    fn successful_upgrade_then_rollback_preserves_both_versions() {
        let (mut r, mut s) = fixture();
        switch(&mut r, &mut s, &"B".repeat(64)).unwrap();
        assert_eq!(r.value.previous, Some("A".repeat(64)));
        assert!(!r.value.pending);
        switch(&mut r, &mut s, &"A".repeat(64)).unwrap();
        assert_eq!(r.value.current, "A".repeat(64));
        assert_eq!(r.value.previous, Some("B".repeat(64)));
    }
}
