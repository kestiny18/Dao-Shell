use crate::{
    core::{Cancellation, DirectoryEffect, MoveItem, Operation, Status, now},
    files::{Objects, Scope},
    platform,
    storage::Journal,
};
use anyhow::{Context, Result, ensure};
use std::{collections::HashSet, fs, path::Path};

pub fn prepare(
    ids: &[String],
    destination: &Path,
    scope: &Scope,
    objects: &Objects,
    journal: &mut Journal,
) -> Result<Operation> {
    ensure!(cfg!(windows), "文件移动首版只支持 Windows");
    let blockers = move_blockers(ids, destination, scope, objects, journal)?;
    ensure!(
        blockers.is_empty(),
        "无法准备移动：\n- {}",
        blockers.join("\n- ")
    );
    ensure!(
        !ids.is_empty() && ids.len() <= 20,
        "一次只能移动 1–20 个普通文件"
    );
    let (destination, create) = scope.destination(destination)?;
    let parent = create
        .first()
        .and_then(|p| p.parent())
        .unwrap_or(&destination);
    let parent_identity = platform::identity(parent)?;
    let mut seen = HashSet::new();
    let mut names = HashSet::new();
    let mut items = Vec::new();
    let unresolved = journal.list()?;
    for id in ids {
        let object = objects.checked(id, scope, true)?;
        ensure!(!object.identity.directory, "不支持移动目录");
        ensure!(
            object.identity.volume == parent_identity.volume,
            "首版不支持跨卷移动"
        );
        ensure!(
            seen.insert((object.identity.volume, object.identity.index)),
            "批次不能包含重复文件或同一文件的硬链接"
        );
        let filename = object.path.file_name().context("无效的文件名")?;
        ensure!(
            names.insert(filename.to_string_lossy().to_lowercase()),
            "多个源文件会占用同一个目标名称"
        );
        let target = destination.join(filename);
        ensure!(object.path != target, "文件已经位于目标目录");
        ensure!(
            !target.try_exists()?,
            "目的文件已存在，不会覆盖：{}",
            target.display()
        );
        ensure!(
            !unresolved
                .iter()
                .filter(|op| matches!(
                    op.status,
                    Status::Unknown
                        | Status::Executing
                        | Status::Verifying
                        | Status::AwaitingConfirmation
                ))
                .any(|op| op
                    .items
                    .iter()
                    .any(|i| i.identity.same_object(&object.identity) || i.destination == target)),
            "相同对象有未核对操作；请先使用 history --reconcile 核对，不能自动重试"
        );
        items.push(MoveItem {
            source: object.path,
            destination: target,
            identity: object.identity,
            status: Status::NotStarted,
            evidence: String::new(),
        });
    }
    let operation = Operation {
        id: uuid::Uuid::new_v4().to_string(),
        created_at: now(),
        status: Status::AwaitingConfirmation,
        confirmed_at: None,
        destination_anchor: parent.to_owned(),
        destination_identity: parent_identity,
        items,
        directories: create
            .into_iter()
            .map(|path| DirectoryEffect {
                path,
                status: Status::NotStarted,
                identity: None,
            })
            .collect(),
    };
    journal.save(&operation)?;
    Ok(operation)
}

/// Report independent blockers before building a plan. All reads remain inside Scope;
/// failed identity/read checks suppress dependent checks rather than trusting stale objects.
fn move_blockers(
    ids: &[String],
    destination: &Path,
    scope: &Scope,
    objects: &Objects,
    journal: &Journal,
) -> Result<Vec<String>> {
    let mut blockers = Vec::new();
    if ids.is_empty() || ids.len() > 20 {
        blockers.push("一次只能移动 1–20 个普通文件".into());
    }
    if let Err(error) = scope.destination(destination) {
        blockers.push(format!("目标目录：{error:#}"));
    }
    let target = scope
        .inspect_destination(destination)
        .and_then(|(path, missing)| {
            let parent = missing.first().and_then(|p| p.parent()).unwrap_or(&path);
            let identity = platform::identity(parent)?;
            Ok((path, identity))
        });
    let target = match target {
        Ok(target) => Some(target),
        Err(error) => {
            blockers.push(format!(
                "目标位置无法核实，其跨卷和名称冲突检查未完成：{error:#}"
            ));
            None
        }
    };
    let unresolved = journal.list()?;
    let mut seen = HashSet::new();
    let mut names = HashSet::new();
    // Bound work even for malformed oversized requests.
    for (index, id) in ids.iter().take(20).enumerate() {
        let label = format!("第 {} 项", index + 1);
        let object = match objects.checked(id, scope, false) {
            Ok(object) => object,
            Err(error) => {
                blockers.push(format!("{label}：{error:#}"));
                continue;
            }
        };
        if let Err(error) = scope.check(&object.path, true) {
            blockers.push(format!("{label}源文件：{error:#}"));
        }
        if object.identity.directory {
            blockers.push(format!("{label}：不支持移动目录"));
        }
        if !seen.insert((object.identity.volume, object.identity.index)) {
            blockers.push(format!("{label}：批次不能包含重复文件或同一文件的硬链接"));
        }
        let Some(filename) = object.path.file_name() else {
            blockers.push(format!("{label}：无效的文件名"));
            continue;
        };
        if !names.insert(filename.to_string_lossy().to_lowercase()) {
            blockers.push(format!("{label}：多个源文件会占用同一个目标名称"));
        }
        let target_path = target.as_ref().map(|(directory, identity)| {
            if identity.volume != object.identity.volume {
                blockers.push(format!(
                    "{label}：首版不支持跨卷移动，增加写权限也不能解除此限制"
                ));
            }
            let path = directory.join(filename);
            if object.path == path {
                blockers.push(format!("{label}：文件已经位于目标目录"));
            }
            match path.try_exists() {
                Ok(true) => blockers.push(format!(
                    "{label}：目的文件已存在，不会覆盖：{}",
                    path.display()
                )),
                Err(error) => blockers.push(format!("{label}：目的文件无法核实：{error}")),
                Ok(false) => {}
            }
            path
        });
        if unresolved
            .iter()
            .filter(|op| {
                matches!(
                    op.status,
                    Status::Unknown
                        | Status::Executing
                        | Status::Verifying
                        | Status::AwaitingConfirmation
                )
            })
            .any(|op| {
                op.items.iter().any(|item| {
                    item.identity.same_object(&object.identity)
                        || target_path.as_ref() == Some(&item.destination)
                })
            })
        {
            blockers.push(format!(
                "{label}：相同对象有未核对操作；请先使用 history --reconcile 核对，不能自动重试"
            ));
        }
    }
    Ok(blockers)
}

/// Confirmation is a local UI callback. It is never deserialized from model arguments.
pub fn run(
    mut operation: Operation,
    scope: &Scope,
    journal: &mut Journal,
    cancel: &Cancellation,
    confirm: impl FnOnce(&Operation) -> Result<bool>,
) -> Result<Operation> {
    ensure!(
        operation.status == Status::AwaitingConfirmation,
        "此方案不是待确认状态"
    );
    let approved = match confirm(&operation) {
        Ok(approved) => approved,
        Err(error) => {
            operation.status = Status::Cancelled;
            journal.save(&operation)?;
            return Err(error);
        }
    };
    if !approved || cancel.is_cancelled() {
        operation.status = Status::Cancelled;
        journal.save(&operation)?;
        return Ok(operation);
    }
    // Revalidate the entire immutable plan before the first effect.
    if let Err(error) = preflight(&operation, scope) {
        operation.status = Status::Invalidated;
        for item in &mut operation.items {
            item.evidence = format!("{error:#}");
        }
        journal.save(&operation)?;
        return Ok(operation);
    }
    operation.confirmed_at = Some(now());
    operation.status = Status::Executing;
    journal.save(&operation)?;
    let outcome = execute(&mut operation, scope, journal, cancel);
    if let Err(error) = outcome {
        // A journal failure can occur after an OS effect: don't report a clean failure or retry.
        operation.status = Status::Unknown;
        let _ = journal.save(&operation);
        return Err(error.context(format!("操作 {} 需核对；不会重试", operation.id)));
    }
    operation.status = operation.aggregate(cancel.is_cancelled());
    journal.save(&operation)?;
    Ok(operation)
}

fn preflight(operation: &Operation, scope: &Scope) -> Result<()> {
    scope.check(&operation.destination_anchor, true)?;
    ensure!(
        platform::identity(&operation.destination_anchor)?
            .same_object(&operation.destination_identity),
        "目的目录身份已变化，需重新确认"
    );
    for item in &operation.items {
        scope.check(&item.source, true)?;
        platform::require_identity(&item.source, &item.identity)?;
        let (_, missing) = scope.destination(item.destination.parent().unwrap())?;
        ensure!(
            missing
                .iter()
                .all(|p| operation.directories.iter().any(|d| &d.path == p)),
            "需要创建的目录发生变化"
        );
        ensure!(!item.destination.try_exists()?, "目标文件已存在，方案失效");
    }
    for directory in &operation.directories {
        ensure!(
            !directory.path.try_exists()?,
            "待建目录状态变化，需重新确认"
        );
    }
    Ok(())
}

fn execute(
    operation: &mut Operation,
    scope: &Scope,
    journal: &mut Journal,
    cancel: &Cancellation,
) -> Result<()> {
    // Guards live through the whole operation, including verification.
    let mut guards = vec![platform::pin_directory(&operation.destination_anchor)?];
    ensure!(
        platform::identity(&operation.destination_anchor)?
            .same_object(&operation.destination_identity),
        "目的目录在执行前已变化"
    );
    for index in 0..operation.directories.len() {
        if cancel.is_cancelled() {
            return Ok(());
        }
        let path = operation.directories[index].path.clone();
        let parent = scope.check(path.parent().unwrap(), true)?;
        guards.push(platform::pin_directory(&parent)?);
        operation.directories[index].status = Status::Executing;
        journal.save(operation)?;
        match fs::create_dir(&path) {
            Ok(()) => {
                // If identity/pinning fails after mkdir, retain Unknown rather than claim no effect.
                operation.directories[index].status = Status::Unknown;
                operation.directories[index].identity = Some(platform::identity(&path)?);
                guards.push(platform::pin_directory(&path)?);
                operation.directories[index].status = Status::Succeeded;
                journal.save(operation)?;
            }
            Err(_) => {
                operation.directories[index].status = Status::Failed;
                journal.save(operation)?;
                return Ok(());
            }
        }
    }
    for index in 0..operation.items.len() {
        if cancel.is_cancelled() {
            break;
        }
        let item = operation.items[index].clone();
        let valid = (|| -> Result<()> {
            scope.check(&item.source, true)?;
            scope.check(item.destination.parent().unwrap(), true)?;
            platform::require_identity(&item.source, &item.identity)?;
            ensure!(!item.destination.try_exists()?, "目的文件已出现，不会覆盖");
            Ok(())
        })();
        if let Err(error) = valid {
            operation.items[index].status = Status::Failed;
            operation.items[index].evidence = format!("执行前检查失败：{error:#}");
            journal.save(operation)?;
            break;
        }
        operation.items[index].status = Status::Executing;
        journal.save(operation)?;
        let call = platform::move_file(&item.source, &item.destination, &item.identity);
        operation.items[index].status = Status::Verifying;
        journal.save(operation)?;
        let (status, evidence) = observe_move(&item);
        operation.items[index].status = status;
        operation.items[index].evidence = match call {
            Ok(()) => evidence,
            Err(e) => format!("{e:#}；{evidence}"),
        };
        journal.save(operation)?;
        if status != Status::Succeeded {
            break;
        }
    }
    Ok(())
}

fn observe_move(item: &MoveItem) -> (Status, String) {
    let source = observe(&item.source);
    let destination = observe(&item.destination);
    match (source, destination) {
        (Ok(None), Ok(Some(target))) if target.same_object(&item.identity) => (
            Status::Succeeded,
            "源路径已不存在，目的路径的卷和文件身份匹配".into(),
        ),
        (Ok(Some(source)), Ok(target))
            if source == item.identity
                && target
                    .as_ref()
                    .is_none_or(|t| !t.same_object(&item.identity)) =>
        {
            (Status::Failed, "文件仍在源位置，目标未达成".into())
        }
        _ => (
            Status::Unknown,
            "无法确定移动结果；保留记录，不自动重试".into(),
        ),
    }
}
fn observe(path: &Path) -> Result<Option<crate::core::FileIdentity>> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            crate::files::normalize_existing(path)?;
            Ok(Some(platform::identity(path)?))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Restart only observes known paths. It never replays a write or invents user confirmation.
pub fn reconcile(journal: &mut Journal) -> Result<Vec<Operation>> {
    let mut changed = Vec::new();
    for mut operation in journal.list()? {
        if operation.status == Status::AwaitingConfirmation {
            operation.status = Status::Cancelled;
        } else if matches!(
            operation.status,
            Status::Executing | Status::Verifying | Status::Unknown
        ) {
            for item in &mut operation.items {
                if matches!(
                    item.status,
                    Status::Executing | Status::Verifying | Status::Unknown
                ) {
                    (item.status, item.evidence) = observe_move(item);
                }
            }
            for directory in &mut operation.directories {
                if matches!(directory.status, Status::Executing | Status::Unknown) {
                    directory.status = match observe(&directory.path) {
                        Ok(Some(current))
                            if directory
                                .identity
                                .as_ref()
                                .is_some_and(|id| id.same_object(&current)) =>
                        {
                            Status::Succeeded
                        }
                        Ok(None) => Status::Failed,
                        _ => Status::Unknown,
                    };
                }
            }
            operation.status = operation.aggregate(false);
        } else {
            continue;
        }
        journal.save(&operation)?;
        changed.push(operation);
    }
    Ok(changed)
}
