//! Exact software-state contract. An incomplete inventory never authorizes a conditional install.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub source_id: String,
    pub package_id: String,
    pub version: String,
    pub scope: Scope,
    pub architecture: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    User,
    System,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Installation {
    AbsentVerified {
        scope: Scope,
    },
    Present {
        identity: Identity,
        instance: String,
    },
    Unknown {
        reason: String,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Install,
    Upgrade,
    Uninstall,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    pub identity: Identity,
    pub name: String,
    pub publisher: String,
    pub installation: Installation,
    pub agreement_digest: Option<String>,
    pub agreements: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub operation_id: String,
    pub operation: Operation,
    pub package: Package,
    pub expires_at: i64,
}
impl Plan {
    pub fn prepare(operation: Operation, package: Package) -> Result<Self> {
        let id = &package.identity;
        ensure!(
            id.source_id == "Microsoft.Winget.Source_8wekyb3d8bbwe"
                && !id.package_id.is_empty()
                && id.package_id.len() <= 256
                && !id.version.is_empty()
                && id.version.len() <= 128,
            "unsupported_package_identity"
        );
        ensure!(
            matches!(id.architecture.as_str(), "x64" | "arm64" | "x86"),
            "unsupported_architecture"
        );
        match (&operation, &package.installation) {
            (Operation::Install, Installation::AbsentVerified { scope }) => {
                ensure!(*scope == id.scope, "scope_changed")
            }
            (
                Operation::Upgrade | Operation::Uninstall,
                Installation::Present { identity, instance },
            ) => {
                ensure!(
                    !instance.is_empty()
                        && identity.source_id == id.source_id
                        && identity.package_id == id.package_id
                        && identity.scope == id.scope
                        && identity.architecture == id.architecture,
                    "ambiguous_installation"
                );
                if operation == Operation::Uninstall {
                    ensure!(identity.version == id.version, "version_changed");
                }
            }
            _ => anyhow::bail!("installation_state_not_authoritative"),
        }
        ensure!(
            package.agreements.is_empty() == package.agreement_digest.is_none(),
            "agreement_evidence_missing"
        );
        Ok(Self {
            operation_id: uuid::Uuid::new_v4().to_string(),
            operation,
            package,
            expires_at: chrono::Utc::now().timestamp() + 300,
        })
    }
    pub fn revalidate(&self, current: &Package, accepted_digest: Option<&str>) -> Result<()> {
        ensure!(
            chrono::Utc::now().timestamp() <= self.expires_at,
            "plan_expired"
        );
        ensure!(&self.package == current, "package_state_changed");
        ensure!(
            self.package.agreement_digest.as_deref() == accepted_digest,
            "agreement_confirmation_required"
        );
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Verified,
    Failed,
    CancelledBeforeDispatch,
    Unknown,
    AwaitingUser,
    AwaitingReboot,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResult {
    pub outcome: Outcome,
    pub provider_code: String,
    pub reboot_required: Option<bool>,
}
/// Implementations receive a frozen plan, never model-supplied switches or URLs.
pub trait Provider {
    fn search(&mut self, query: &str) -> Result<Vec<Package>>;
    fn inspect(&mut self, identity: &Identity) -> Result<Package>;
    fn dispatch(
        &mut self,
        plan: &Plan,
        cancel: &crate::core::Cancellation,
        progress: &mut dyn FnMut(&str),
    ) -> Result<DispatchResult>;
    fn compare_versions(&self, identity: &Identity, installed: &str) -> Result<std::cmp::Ordering>;
}
#[cfg(test)]
mod tests {
    use super::*;
    fn package() -> Package {
        Package {
            identity: Identity {
                source_id: "Microsoft.Winget.Source_8wekyb3d8bbwe".into(),
                package_id: "Fixture.Safe".into(),
                version: "1".into(),
                scope: Scope::User,
                architecture: "x64".into(),
            },
            name: "Fixture".into(),
            publisher: "Fixture".into(),
            installation: Installation::AbsentVerified { scope: Scope::User },
            agreement_digest: None,
            agreements: vec![],
        }
    }
    #[test]
    fn unknown_is_not_absent() {
        let mut p = package();
        p.installation = Installation::Unknown {
            reason: "incomplete".into(),
        };
        assert!(Plan::prepare(Operation::Install, p).is_err());
    }
    #[test]
    fn source_scope_version_and_agreements_are_frozen() {
        let mut p = package();
        p.agreements = vec!["local terms".into()];
        p.agreement_digest = Some("digest".into());
        let plan = Plan::prepare(Operation::Install, p.clone()).unwrap();
        assert!(plan.revalidate(&p, None).is_err());
        assert!(plan.revalidate(&p, Some("digest")).is_ok());
        p.identity.version = "2".into();
        assert!(plan.revalidate(&p, Some("digest")).is_err());
    }
    #[test]
    fn uninstall_requires_exact_instance_and_version() {
        let mut p = package();
        p.installation = Installation::Present {
            identity: p.identity.clone(),
            instance: "provider-instance".into(),
        };
        assert!(Plan::prepare(Operation::Uninstall, p.clone()).is_ok());
        p.identity.version = "2".into();
        assert!(Plan::prepare(Operation::Uninstall, p).is_err());
    }
}
