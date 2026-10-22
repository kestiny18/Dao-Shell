use super::{bindings::Microsoft::Management::Deployment::*, contract::*};
use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};
use windows::Win32::System::Com::*;
use windows_core::{GUID, HSTRING, Interface};
const SOURCE: &str = "Microsoft.Winget.Source_8wekyb3d8bbwe";
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() }
    }
}
pub struct NativeProvider {
    manager: PackageManager,
    _apartment: Apartment,
}
fn create<T: Interface>(id: u128) -> windows_core::Result<T> {
    unsafe { CoCreateInstance(&GUID::from_u128(id), None, CLSCTX_ALL) }
}
impl NativeProvider {
    pub fn open() -> Result<Self> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
        let apartment = Apartment;
        let manager = create(0xc53a4f16_787e_42a4_b304_29effb4bf597)?;
        Ok(Self {
            manager,
            _apartment: apartment,
        })
    }
    fn catalog(&self, scope: Scope) -> Result<PackageCatalog> {
        let remote = self
            .manager
            .GetPackageCatalogByName(&HSTRING::from("winget"))?;
        let info = remote.Info()?;
        ensure!(
            info.Id()? == SOURCE
                && info.Type()? == "Microsoft.PreIndexed.Package"
                && matches!(
                    info.Argument()?.to_string().as_str(),
                    "https://cdn.winget.microsoft.com/cache" | "https://winget.azureedge.net/cache"
                ),
            "source_identity_mismatch"
        );
        ensure!(
            remote.SourceAgreements()?.Size()? == 0,
            "source_agreement_requires_system_ui"
        );
        remote.SetAcceptSourceAgreements(false)?;
        let options: CreateCompositePackageCatalogOptions =
            create(0x526534b8_7e46_47c8_8416_b1685c327d37)?;
        options.Catalogs()?.Append(&remote)?;
        options.SetCompositeSearchBehavior(CompositeSearchBehavior::AllCatalogs)?;
        options.SetInstalledScope(match scope {
            Scope::User => PackageInstallScope::User,
            Scope::System => PackageInstallScope::System,
        })?;
        let result = self
            .manager
            .CreateCompositePackageCatalog(&options)?
            .Connect()?;
        ensure!(
            result.Status()? == ConnectResultStatus::Ok,
            "catalog_connection_incomplete"
        );
        Ok(result.PackageCatalog()?)
    }
    fn find(&self, query: &str, exact: bool, scope: Scope) -> Result<Vec<CatalogPackage>> {
        ensure!(
            !query.is_empty() && query.len() <= 256 && !query.chars().any(char::is_control),
            "invalid_software_query"
        );
        let options: FindPackagesOptions = create(0x572ded96_9c60_4526_8f92_ee7d91d38c1a)?;
        options.SetResultLimit(21)?;
        for field in if exact {
            vec![PackageMatchField::Id]
        } else {
            vec![PackageMatchField::Id, PackageMatchField::Name]
        } {
            let filter: PackageMatchFilter = create(0xd02c9daf_99dc_429c_b503_4e504e4ab000)?;
            filter.SetField(field)?;
            filter.SetOption(if exact {
                PackageFieldMatchOption::EqualsCaseInsensitive
            } else {
                PackageFieldMatchOption::ContainsCaseInsensitive
            })?;
            filter.SetValue(&HSTRING::from(query))?;
            options.Selectors()?.Append(&filter)?;
        }
        let result = self.catalog(scope)?.FindPackages(&options)?;
        ensure!(
            result.Status()? == FindPackagesResultStatus::Ok,
            "catalog_search_incomplete"
        );
        let matches = result.Matches()?;
        ensure!(matches.Size()? <= 20, "catalog_result_limit");
        matches
            .into_iter()
            .map(|m| Ok(m.CatalogPackage()?))
            .collect()
    }
    fn exact(
        &self,
        id: &Identity,
    ) -> Result<(CatalogPackage, PackageVersionInfo, PackageVersionId)> {
        ensure!(id.source_id == SOURCE, "source_identity_mismatch");
        let mut rows = self.find(&id.package_id, true, id.scope)?;
        ensure!(rows.len() == 1, "package_identity_ambiguous");
        let package = rows.remove(0);
        ensure!(package.Id()? == id.package_id, "package_identity_mismatch");
        let versions = package.AvailableVersions()?;
        let mut matching = Vec::new();
        for v in versions {
            if v.Version()? == id.version && v.PackageCatalogId()? == SOURCE {
                matching.push(v);
            }
        }
        ensure!(matching.len() == 1, "version_identity_ambiguous");
        let version = matching.remove(0);
        let info = package.GetPackageVersionInfo(&version)?;
        Ok((package, info, version))
    }
    fn options(id: &Identity, version: &PackageVersionId) -> Result<InstallOptions> {
        let o: InstallOptions = create(0x1095f097_eb96_453b_b4e6_1613637f3b14)?;
        o.SetPackageVersionId(version)?;
        o.SetPackageInstallMode(PackageInstallMode::Interactive)?;
        o.SetPackageInstallScope(match id.scope {
            Scope::User => PackageInstallScope::User,
            Scope::System => PackageInstallScope::System,
        })?;
        let arch = match id.architecture.as_str() {
            "x64" => windows::System::ProcessorArchitecture::X64,
            "x86" => windows::System::ProcessorArchitecture::X86,
            "arm64" => windows::System::ProcessorArchitecture::Arm64,
            _ => anyhow::bail!("unsupported_architecture"),
        };
        o.AllowedArchitectures()?.Append(arch)?;
        o.SetAllowHashMismatch(false)?;
        o.SetForce(false)?;
        o.SetAllowUpgradeToUnknownVersion(false)?;
        Ok(o)
    }
    fn describe(&self, id: &Identity) -> Result<Package> {
        let (package, info, version) = self.exact(id)?;
        let o = Self::options(id, &version)?;
        let installer = info.GetApplicableInstaller(&o)?;
        ensure!(
            installer.Scope()?
                == match id.scope {
                    Scope::User => PackageInstallerScope::User,
                    Scope::System => PackageInstallerScope::System,
                },
            "installer_scope_unknown"
        );
        let installed = unsafe {
            let mut raw = std::ptr::null_mut();
            let hr = (Interface::vtable(&package).InstalledVersion)(
                Interface::as_raw(&package),
                &mut raw,
            );
            hr.ok()?;
            if raw.is_null() {
                None
            } else {
                Some(PackageVersionInfo::from_raw(raw))
            }
        };
        let installation = if let Some(installed) = installed {
            let scope = installed
                .GetMetadata(PackageVersionMetadataField::InstalledScope)?
                .to_string();
            ensure!(
                scope.eq_ignore_ascii_case(match id.scope {
                    Scope::User => "user",
                    Scope::System => "machine",
                }) || id.scope == Scope::System && scope.eq_ignore_ascii_case("system"),
                "installed_scope_unknown"
            );
            let mut keys: Vec<String> = installed
                .ProductCodes()?
                .into_iter()
                .map(|s| s.to_string())
                .chain(
                    installed
                        .PackageFamilyNames()?
                        .into_iter()
                        .map(|s| s.to_string()),
                )
                .collect();
            keys.sort();
            keys.dedup();
            ensure!(!keys.is_empty(), "installation_instance_unknown");
            let instance = format!("{:x}", Sha256::digest(serde_json::to_vec(&keys)?));
            let mut installed_id = id.clone();
            installed_id.version = installed.Version()?.to_string();
            Installation::Present {
                identity: installed_id,
                instance,
            }
        } else {
            Installation::AbsentVerified { scope: id.scope }
        };
        let mut agreements = Vec::new();
        for a in info.GetCatalogPackageMetadata()?.Agreements()? {
            agreements.push(format!("{}\n{}\n{}", a.Label()?, a.Text()?, a.Url()?));
        }
        ensure!(
            agreements.iter().map(String::len).sum::<usize>() < 16000,
            "agreement_budget"
        );
        let agreement_digest = if agreements.is_empty() {
            None
        } else {
            Some(format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&agreements)?)
            ))
        };
        Ok(Package {
            identity: id.clone(),
            name: package.Name()?.to_string(),
            publisher: info.Publisher()?.to_string(),
            installation,
            agreements,
            agreement_digest,
        })
    }
}
impl Provider for NativeProvider {
    fn search(&mut self, query: &str) -> Result<Vec<Package>> {
        let mut out = Vec::new();
        for p in self.find(query, false, Scope::User)? {
            let info = p.DefaultInstallVersion()?;
            let id = Identity {
                source_id: SOURCE.into(),
                package_id: p.Id()?.to_string(),
                version: info.Version()?.to_string(),
                scope: Scope::User,
                architecture: if cfg!(target_arch = "aarch64") {
                    "arm64"
                } else {
                    "x64"
                }
                .into(),
            };
            out.push(self.describe(&id)?);
        }
        Ok(out)
    }
    fn inspect(&mut self, id: &Identity) -> Result<Package> {
        self.describe(id)
    }
    fn compare_versions(&self, id: &Identity, installed: &str) -> Result<std::cmp::Ordering> {
        let (_, info, _) = self.exact(id)?;
        match info.CompareToVersion(&HSTRING::from(installed))? {
            CompareResult::Lesser => Ok(std::cmp::Ordering::Less),
            CompareResult::Equal => Ok(std::cmp::Ordering::Equal),
            CompareResult::Greater => Ok(std::cmp::Ordering::Greater),
            _ => anyhow::bail!("version_comparison_unknown"),
        }
    }
    fn dispatch(
        &mut self,
        plan: &Plan,
        cancel: &crate::core::Cancellation,
        progress: &mut dyn FnMut(&str),
    ) -> Result<DispatchResult> {
        let (p, info, version) = self.exact(&plan.package.identity)?;
        if plan.operation == Operation::Upgrade
            && let Installation::Present { identity, .. } = &plan.package.installation
        {
            ensure!(
                info.CompareToVersion(&HSTRING::from(&identity.version))? == CompareResult::Greater,
                "target_not_newer"
            );
        }
        if cancel.is_cancelled() {
            return Ok(DispatchResult {
                outcome: Outcome::CancelledBeforeDispatch,
                provider_code: "cancelled_before_dispatch".into(),
                reboot_required: Some(false),
            });
        }
        if plan.operation == Operation::Uninstall {
            let o: UninstallOptions = create(0xe1d9a11e_9f85_4d87_9c17_2b93143adb8d)?;
            o.SetPackageUninstallMode(PackageUninstallMode::Interactive)?;
            o.SetPackageUninstallScope(match plan.package.identity.scope {
                Scope::User => PackageUninstallScope::User,
                Scope::System => PackageUninstallScope::System,
            })?;
            let op = self.manager.UninstallPackageAsync(&p, &o)?;
            let result = wait(&op, cancel, progress)?;
            let reboot = result.RebootRequired()?;
            Ok(DispatchResult {
                outcome: if reboot {
                    Outcome::AwaitingReboot
                } else if result.Status()? == UninstallResultStatus::Ok {
                    Outcome::Verified
                } else {
                    Outcome::Unknown
                },
                provider_code: format!("uninstall_status_{}", result.Status()?.0),
                reboot_required: Some(reboot),
            })
        } else {
            let o = Self::options(&plan.package.identity, &version)?;
            o.SetAcceptPackageAgreements(plan.package.agreement_digest.is_some())?;
            let op = if plan.operation == Operation::Install {
                self.manager.InstallPackageAsync(&p, &o)?
            } else {
                self.manager.UpgradePackageAsync(&p, &o)?
            };
            let result = wait(&op, cancel, progress)?;
            let reboot = result.RebootRequired()?;
            Ok(DispatchResult {
                outcome: if reboot {
                    Outcome::AwaitingReboot
                } else if result.Status()? == InstallResultStatus::Ok {
                    Outcome::Verified
                } else {
                    Outcome::Unknown
                },
                provider_code: format!("install_status_{}", result.Status()?.0),
                reboot_required: Some(reboot),
            })
        }
    }
}
fn wait<R: windows_core::RuntimeType, P: windows_core::RuntimeType + Stage>(
    op: &windows_future::IAsyncOperationWithProgress<R, P>,
    cancel: &crate::core::Cancellation,
    progress: &mut dyn FnMut(&str),
) -> Result<R> {
    let (tx, rx) = std::sync::mpsc::sync_channel(8);
    op.SetProgress(&windows_future::AsyncOperationProgressHandler::new(
        move |_, p: windows_core::Ref<P>| {
            if let Some(p) = p.as_ref() {
                let _ = tx.try_send(p.stage());
            }
            Ok(())
        },
    ))?;
    let began = std::time::Instant::now();
    let mut cancellation_requested = false;
    progress("provider_dispatched");
    while op.Status()? == windows_future::AsyncStatus::Started {
        for stage in rx.try_iter() {
            progress(stage);
        }
        if cancel.is_cancelled() && !cancellation_requested {
            op.Cancel()?;
            cancellation_requested = true;
            progress("cancel_requested_may_continue");
        }
        if began.elapsed() > std::time::Duration::from_secs(900) {
            anyhow::bail!("provider_outcome_unknown");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(op.GetResults()?)
}

trait Stage {
    fn stage(&self) -> &'static str;
}
impl Stage for InstallProgress {
    fn stage(&self) -> &'static str {
        match self.State.0 {
            0 => "queued",
            1 => "downloading",
            2 => "installing",
            3 => "post_install",
            _ => "provider_progress",
        }
    }
}
impl Stage for UninstallProgress {
    fn stage(&self) -> &'static str {
        match self.State.0 {
            0 => "queued",
            1 => "uninstalling",
            2 => "post_uninstall",
            _ => "provider_progress",
        }
    }
}
