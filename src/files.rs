use crate::{
    core::{Cancellation, FileObject, now},
    platform,
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct Scope {
    read: Vec<PathBuf>,
    write: Vec<PathBuf>,
    full_read: bool,
    full_write: bool,
}
impl Scope {
    pub fn new(read: &[PathBuf], write: &[PathBuf]) -> Result<Self> {
        let mut all = read.to_vec();
        all.extend_from_slice(write);
        let read: Vec<_> = all
            .iter()
            .map(|p| normalize_existing(p))
            .collect::<Result<_>>()?;
        let write = write
            .iter()
            .map(|p| normalize_existing(p))
            .collect::<Result<Vec<_>>>()?;
        for path in read.iter().chain(&write) {
            ensure!(path.is_dir(), "范围必须是目录：{}", path.display());
        }
        Ok(Self {
            read: compact_roots(read),
            write: compact_roots(write),
            full_read: false,
            full_write: false,
        })
    }
    /// Default search locations do not define authority in full mode.
    pub fn full(search_roots: &[PathBuf], writable: bool) -> Result<Self> {
        let mut scope = Self::new(search_roots, &[])?;
        scope.full_read = true;
        scope.full_write = writable;
        Ok(scope)
    }
    pub fn access_description(&self) -> &'static str {
        if self.full_read {
            "完全访问：可按用户要求指定本机目录，受系统权限与路径检查约束；默认只搜索列出的常用位置。写操作仍需确认。"
        } else {
            "受限访问：仅允许配置的目录。"
        }
    }
    pub fn read_roots(&self) -> &[PathBuf] {
        &self.read
    }
    pub fn write_roots(&self) -> &[PathBuf] {
        &self.write
    }
    pub fn check(&self, path: &Path, write: bool) -> Result<PathBuf> {
        let path = normalize_existing(path)?;
        let roots = if write { &self.write } else { &self.read };
        ensure!(
            (if write {
                self.full_write
            } else {
                self.full_read
            }) || roots.iter().any(|r| path.starts_with(r)),
            "路径不在已配置的{}范围：{}",
            if write { "写入" } else { "读取" },
            path.display()
        );
        Ok(path)
    }
    pub fn destination(&self, path: &Path) -> Result<(PathBuf, Vec<PathBuf>)> {
        self.resolve_destination(path, true)
    }
    /// Read-only preparation diagnostics do not grant permission to create the path.
    pub(crate) fn inspect_destination(&self, path: &Path) -> Result<(PathBuf, Vec<PathBuf>)> {
        self.resolve_destination(path, false)
    }
    fn resolve_destination(&self, path: &Path, write: bool) -> Result<(PathBuf, Vec<PathBuf>)> {
        ensure!(path.is_absolute(), "目的目录必须是绝对路径");
        validate_components(path)?;
        let mut ancestor = path.to_owned();
        let mut missing = Vec::new();
        while !ancestor.try_exists()? {
            missing.push(
                ancestor
                    .file_name()
                    .context("找不到已有的目标父目录")?
                    .to_owned(),
            );
            ensure!(ancestor.pop(), "找不到已有的目标父目录");
        }
        let mut target = self.check(&ancestor, write)?;
        ensure!(target.is_dir(), "目的地的父路径不是目录");
        let mut create = Vec::new();
        for component in missing.into_iter().rev() {
            target.push(component);
            create.push(target.clone());
        }
        ensure!(create.len() <= 8, "一次最多创建 8 层目标目录");
        Ok((target, create))
    }
}

fn compact_roots(mut roots: Vec<PathBuf>) -> Vec<PathBuf> {
    roots.sort_by(|a, b| {
        a.components()
            .count()
            .cmp(&b.components().count())
            .then(a.cmp(b))
    });
    let mut compact: Vec<PathBuf> = Vec::new();
    for root in roots {
        if !compact.iter().any(|parent| root.starts_with(parent)) {
            compact.push(root);
        }
    }
    compact
}

pub fn validate_components(path: &Path) -> Result<()> {
    for part in path.components() {
        ensure!(!matches!(part, Component::ParentDir), "路径不能包含 ..");
        if let Component::Normal(name) = part {
            let name = name.to_string_lossy();
            ensure!(!name.contains('\0'), "路径包含空字符");
            #[cfg(windows)]
            {
                ensure!(
                    !name.contains(':') && !name.ends_with(['.', ' ']),
                    "不支持备用数据流或歧义路径"
                );
                let base = name.split('.').next().unwrap_or("").to_ascii_uppercase();
                ensure!(
                    !["CON", "PRN", "AUX", "NUL", "CLOCK$", "CONIN$", "CONOUT$"]
                        .contains(&base.as_str())
                        && !(base.len() == 4
                            && (base.starts_with("COM") || base.starts_with("LPT"))
                            && base.as_bytes()[3].is_ascii_digit()),
                    "不支持设备路径"
                );
            }
        }
        #[cfg(windows)]
        if let Component::Prefix(p) = part {
            ensure!(
                matches!(
                    p.kind(),
                    std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
                ),
                "首版只支持本机盘符路径"
            );
        }
    }
    Ok(())
}
pub fn normalize_existing(path: &Path) -> Result<PathBuf> {
    validate_components(path)?;
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut part = PathBuf::new();
    for component in absolute.components() {
        part.push(component);
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        let metadata =
            fs::symlink_metadata(&part).with_context(|| format!("无法访问 {}", part.display()))?;
        ensure!(
            !platform::is_link(&metadata),
            "不跟随链接或重解析点：{}",
            part.display()
        );
    }
    fs::canonicalize(absolute).context("无法规范化路径")
}

#[derive(Default)]
pub struct Objects {
    inventory: Option<std::sync::Arc<crate::inventory::Inventory>>,
    index: Option<std::sync::Arc<dyn crate::search_index::IndexClient>>,
    entries: HashMap<String, FileObject>,
    by_path: HashMap<PathBuf, String>,
}
impl Objects {
    pub fn set_index(
        &mut self,
        index: Option<std::sync::Arc<dyn crate::search_index::IndexClient>>,
    ) {
        self.index = index;
    }
    pub fn has_index(&self) -> bool {
        self.index.is_some()
    }
    pub fn reset(&mut self) {
        self.entries.clear();
        self.by_path.clear();
    }
    pub fn set_inventory(&mut self, inventory: std::sync::Arc<crate::inventory::Inventory>) {
        self.inventory = Some(inventory);
    }
    pub fn insert(&mut self, path: &Path) -> Result<FileObject> {
        let path = normalize_existing(path)?;
        let identity = platform::identity(&path)?;
        if let Some(id) = self.by_path.get(&path)
            && let Some(existing) = self.entries.get(id)
            && existing.identity == identity
        {
            return Ok(existing.clone());
        }
        ensure!(
            self.entries.len() < 2000,
            "当前会话对象已达 2000 项，请 /reset 后重新查询"
        );
        let modified_at = fs::metadata(&path)?
            .modified()
            .ok()
            .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());
        let object = FileObject {
            id: uuid::Uuid::new_v4().to_string(),
            path: path.to_owned(),
            identity,
            observed_at: now(),
            modified_at,
        };
        self.entries.insert(object.id.clone(), object.clone());
        self.by_path.insert(path, object.id.clone());
        Ok(object)
    }
    pub fn get(&self, id: &str) -> Result<&FileObject> {
        self.entries
            .get(id)
            .context("引用不属于本次会话，请先查找文件")
    }
    pub fn checked(&self, id: &str, scope: &Scope, write: bool) -> Result<FileObject> {
        let object = self.get(id)?.clone();
        scope.check(&object.path, write)?;
        platform::require_identity(&object.path, &object.identity)?;
        Ok(object)
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Search {
    pub query: String,
    pub terms: Vec<String>,
    pub match_mode: MatchMode,
    pub kind: Kind,
    pub directory: Option<PathBuf>,
    pub extension: Option<String>,
    pub modified_after: Option<String>,
    pub modified_before: Option<String>,
    pub min_bytes: Option<u64>,
    pub max_bytes: Option<u64>,
    pub sort: Sort,
    pub page: usize,
    pub limit: Option<usize>,
}
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    #[default]
    Relevance,
    ModifiedDesc,
    Name,
    SizeDesc,
}
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    #[default]
    All,
    Any,
}
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Any,
    File,
    Directory,
}
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub source: &'static str,
    pub roots: Vec<PathBuf>,
    pub observed_at: String,
    pub items: Vec<FileObject>,
    pub page: usize,
    pub matched_in_scan: usize,
    pub has_more: bool,
    pub truncated: bool,
    pub skipped: usize,
    pub elapsed_ms: u128,
    pub discovery_ms: u128,
    pub validation_ms: u128,
    pub coverage_note: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_receipt: Option<crate::search_index::Receipt>,
}
pub fn search(
    request: &Search,
    scope: &Scope,
    objects: &mut Objects,
    cancel: &Cancellation,
) -> Result<SearchResult> {
    let search_started = Instant::now();
    let limit = request.limit.unwrap_or(20);
    ensure!(
        (1..=50).contains(&limit) && request.page <= 100,
        "每页 1–50 项，页码 0–100"
    );
    ensure!(request.query.len() <= 512, "关键词过长");
    ensure!(
        request.terms.len() <= 12
            && request
                .terms
                .iter()
                .all(|t| !t.trim().is_empty() && t.len() <= 128),
        "terms 需为最多 12 个非空关键词，每项不超过 128 字节"
    );
    ensure!(
        request.query.is_empty() || request.terms.is_empty(),
        "query 与 terms 二选一，避免歧义"
    );
    ensure!(
        request
            .min_bytes
            .zip(request.max_bytes)
            .is_none_or(|(a, b)| a <= b),
        "大小范围无效"
    );
    let after = request
        .modified_after
        .as_ref()
        .map(|s| chrono::DateTime::parse_from_rfc3339(s))
        .transpose()?;
    let before = request
        .modified_before
        .as_ref()
        .map(|s| chrono::DateTime::parse_from_rfc3339(s))
        .transpose()?;
    ensure!(after.zip(before).is_none_or(|(a, b)| a < b), "时间范围无效");
    let mut index_unavailable = false;
    if let Some(index) = objects.index.clone()
        && request.directory.is_none()
        && !request.query.trim().is_empty()
        && request.terms.is_empty()
        && matches!(request.kind, Kind::Any)
        && matches!(request.sort, Sort::Relevance)
        && request.extension.is_none()
        && after.is_none()
        && before.is_none()
        && request.min_bytes.is_none()
        && request.max_bytes.is_none()
        && request.page == 0
    {
        if let Ok(batch) = index.search(request, cancel) {
            cancel.check()?;
            let validation_started = Instant::now();
            let mut items = Vec::new();
            let mut skipped = 0;
            let mut receipt = batch.receipt;
            for candidate in batch.candidates {
                cancel.check()?;
                let inserted = (|| -> Result<FileObject> {
                    // No Scope expansion: these are discoverable names, not permission to open/read.
                    candidate.check_mapping()?;
                    let identity = platform::identity(&normalize_existing(&candidate.path)?)?;
                    ensure!(
                        identity.volume == candidate.volume && identity.index == candidate.file_id,
                        "索引身份已变化"
                    );
                    let object = objects.insert(&candidate.path)?;
                    candidate.check_mapping()?;
                    ensure!(object.identity == identity, "索引对象在复核期间变化");
                    Ok(object)
                })();
                match inserted {
                    Ok(item) => items.push(item),
                    Err(_) => skipped += 1,
                }
                if items.len() == limit {
                    break;
                }
            }
            if skipped > 0 {
                receipt.complete = false;
                receipt.reasons.push("candidate_changed".into());
            }
            return Ok(SearchResult {
                source: "windows_service",
                roots: Vec::new(),
                observed_at: now(),
                matched_in_scan: items.len(),
                items,
                page: request.page,
                has_more: batch.has_more,
                truncated: !receipt.complete,
                skipped,
                elapsed_ms: search_started.elapsed().as_millis(),
                discovery_ms: validation_started
                    .duration_since(search_started)
                    .as_millis(),
                validation_ms: validation_started.elapsed().as_millis(),
                coverage_note: "全盘名称索引：仅展示当前系统用户可见的候选；按服务快照顺序显示，完整性与时间见各卷回执。搜索覆盖不授予正文读取或打开权限，实际操作仍按访问设置重新核验。",
                index_receipt: Some(receipt),
            });
        }
        cancel.check()?;
        index_unavailable = true;
    }
    let roots = match &request.directory {
        Some(p) => vec![scope.check(p, false)?],
        None => scope.read.clone(),
    };
    if roots.is_empty() && objects.index.is_some() {
        bail!(if index_unavailable {
            "全盘索引服务当前不可用，请在设置 → 文件搜索检查状态；没有兼容目录可补充查询。搜索不会自动请求管理员权限。"
        } else {
            "当前筛选需要目录补充查询，尚无兼容目录。可指定已授权目录，或改用单个文件名关键词查询全盘；不会忽略当前筛选条件。"
        });
    }
    ensure!(
        !roots.is_empty(),
        "尚未设置读取目录。使用 --read-root 或 daosh config 配置"
    );
    let mut inventory_note = "现场扫描：此筛选需要现场观察，未忽略任何筛选条件。";
    if let Some(inventory) = &objects.inventory
        && request.terms.is_empty()
        && matches!(request.kind, Kind::Any)
        && request.extension.is_none()
        && after.is_none()
        && before.is_none()
        && request.min_bytes.is_none()
        && request.max_bytes.is_none()
    {
        match inventory.records_with_reason(&roots, scope) {
            Ok(records) => {
                let mut result = search_records(
                    request,
                    roots,
                    records,
                    scope,
                    objects,
                    cancel,
                    search_started,
                )?;
                if index_unavailable {
                    result.coverage_note = "全盘索引服务当前不可用；仅使用列出的已记录兼容目录快照，非全盘；候选仍重新检查权限与身份。";
                } else if objects.index.is_some() {
                    result.coverage_note = "当前筛选或页码使用列出的已记录兼容目录快照，非全盘；所有筛选与排序仍保留，候选重新检查权限与身份。";
                }
                return Ok(result);
            }
            Err(note) => inventory_note = note,
        }
    }
    let inventory_fallback = objects.inventory.is_some();
    let start = search_started;
    let query = request.query.to_lowercase();
    let terms: Vec<_> = request
        .terms
        .iter()
        .map(|t| t.trim().to_lowercase())
        .collect();
    let mut found = Vec::new();
    let mut skipped = 0;
    let mut truncated = false;
    let mut visited = 0;
    let mut seen = std::collections::HashSet::new();
    let skipped_entries = std::cell::Cell::new(0usize);
    'roots: for root in &roots {
        let walker = walkdir::WalkDir::new(root)
            .follow_links(false)
            .max_depth(32)
            .into_iter()
            .filter_entry(|e| {
                let allowed = fs::symlink_metadata(e.path()).is_ok_and(|m| !platform::is_link(&m));
                if !allowed {
                    skipped_entries.set(skipped_entries.get() + 1);
                }
                allowed
            });
        for entry in walker {
            cancel.check()?;
            visited += 1;
            if start.elapsed() >= Duration::from_secs(5)
                || visited > 100_000
                || found.len() >= 10_000
            {
                truncated = true;
                break 'roots;
            }
            let entry = match entry {
                Ok(x) => x,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            if entry.depth() == 32 && entry.file_type().is_dir() {
                truncated = true;
            }
            let metadata = match fs::symlink_metadata(entry.path()) {
                Ok(m) => m,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            if platform::is_link(&metadata) {
                skipped += 1;
                continue;
            }
            if matches!(request.kind, Kind::File) && !metadata.is_file()
                || matches!(request.kind, Kind::Directory) && !metadata.is_dir()
            {
                continue;
            }
            let relative = entry.path().strip_prefix(root).unwrap_or(entry.path());
            let Some(score) = match_score(
                relative,
                &entry.file_name().to_string_lossy(),
                &query,
                &terms,
                &request.match_mode,
            ) else {
                continue;
            };
            if let Some(ext) = &request.extension
                && (metadata.is_dir()
                    || !entry.path().extension().is_some_and(|s| {
                        s.to_string_lossy()
                            .eq_ignore_ascii_case(ext.trim_start_matches('.'))
                    }))
            {
                continue;
            }
            if request.min_bytes.is_some_and(|n| metadata.len() < n)
                || request.max_bytes.is_some_and(|n| metadata.len() > n)
            {
                continue;
            }
            let modified = metadata
                .modified()
                .ok()
                .map(chrono::DateTime::<chrono::Utc>::from);
            if after.is_some_and(|a| modified.is_none_or(|m| m < a))
                || before.is_some_and(|b| modified.is_none_or(|m| m >= b))
            {
                continue;
            }
            if seen.insert(entry.path().to_owned()) {
                found.push((entry.path().to_owned(), metadata.len(), modified, score));
            }
        }
    }
    match request.sort {
        Sort::Relevance => found.sort_by(|a, b| {
            b.3.cmp(&a.3)
                .then_with(|| b.2.cmp(&a.2))
                .then_with(|| a.0.cmp(&b.0))
        }),
        Sort::ModifiedDesc => found.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0))),
        Sort::SizeDesc => found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))),
        Sort::Name => found.sort_by(|a, b| a.0.cmp(&b.0)),
    }
    let matched_in_scan = found.len();
    let offset = request.page * limit;
    let validation_started = Instant::now();
    let mut items = Vec::new();
    for (path, _, _, _) in found.into_iter().skip(offset).take(limit) {
        match scope.check(&path, false).and_then(|p| objects.insert(&p)) {
            Ok(item) => items.push(item),
            Err(_) => skipped += 1,
        }
    }
    Ok(SearchResult {
        source: "scan",
        roots,
        observed_at: now(),
        items,
        page: request.page,
        matched_in_scan,
        has_more: matched_in_scan > offset + limit,
        truncated,
        skipped: skipped + skipped_entries.get(),
        elapsed_ms: start.elapsed().as_millis(),
        discovery_ms: validation_started.duration_since(start).as_millis(),
        validation_ms: validation_started.elapsed().as_millis(),
        index_receipt: None,
        coverage_note: if index_unavailable {
            "全盘索引服务当前不可用；本次仅在列出的授权兼容目录内补充查询，非全盘。最多 5 秒、100000 项、32 层；可在设置检查服务，不会自动请求管理员权限。"
        } else if objects.index.is_some() {
            "目录补充查询：本次条件只在列出的授权目录内按全部筛选执行，非全盘；最多 5 秒、100000 项、32 层，不读取正文。"
        } else if inventory_fallback {
            inventory_note
        } else {
            "仅在列出的目录内扫描文件名、相对路径和元数据；不读取正文，不能据文件名确认用途；跳过链接/不可访问项，翻页时重新观察。"
        },
    })
}

fn search_records(
    request: &Search,
    roots: Vec<PathBuf>,
    mut records: Vec<crate::inventory::Record>,
    scope: &Scope,
    objects: &mut Objects,
    cancel: &Cancellation,
    start: Instant,
) -> Result<SearchResult> {
    let query = request.query.to_lowercase();
    let mut seen = std::collections::HashSet::new();
    records.retain(|r| {
        seen.insert(r.path.clone())
            && r.path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
                .contains(&query)
    });
    cancel.check()?;
    match request.sort {
        Sort::Relevance | Sort::ModifiedDesc => records.sort_by(|a, b| {
            b.modified
                .cmp(&a.modified)
                .then_with(|| a.path.cmp(&b.path))
        }),
        Sort::SizeDesc => records.sort_by(|a, b| {
            b.identity
                .size
                .cmp(&a.identity.size)
                .then_with(|| a.path.cmp(&b.path))
        }),
        Sort::Name => records.sort_by(|a, b| a.path.cmp(&b.path)),
    }
    let matched_in_scan = records.len();
    let limit = request.limit.unwrap_or(20);
    let offset = request.page * limit;
    let validation_started = Instant::now();
    let mut items = Vec::new();
    let mut skipped = 0;
    for record in records.into_iter().skip(offset).take(limit) {
        cancel.check()?;
        match scope.check(&record.path, false).and_then(|path| {
            platform::require_identity(&path, &record.identity)?;
            objects.insert(&path)
        }) {
            Ok(item) => items.push(item),
            Err(_) => skipped += 1,
        }
    }
    Ok(SearchResult {
        source: "inventory",
        index_receipt: None,
        roots,
        observed_at: now(),
        items,
        page: request.page,
        matched_in_scan,
        has_more: matched_in_scan > offset + limit,
        truncated: false,
        skipped,
        elapsed_ms: start.elapsed().as_millis(),
        discovery_ms: validation_started.duration_since(start).as_millis(),
        validation_ms: validation_started.elapsed().as_millis(),
        coverage_note: "已记录目录：按上次完整快照匹配和排序，非实时；后续新增/修改可能尚未收录；应用运行时辅助更新。候选已重新检查权限与身份，失效项跳过。可在设置 → 文件搜索立即更新。",
    })
}

fn match_score(
    relative: &Path,
    name: &str,
    query: &str,
    terms: &[String],
    mode: &MatchMode,
) -> Option<usize> {
    let name = name.to_lowercase();
    if terms.is_empty() {
        return name.contains(query).then_some(0);
    }
    let path = relative.to_string_lossy().to_lowercase();
    let parent = relative
        .parent()
        .and_then(Path::file_name)
        .map(|p| p.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let scores: Vec<_> = terms
        .iter()
        .map(|term| {
            if name.contains(term) {
                3
            } else if parent.contains(term) {
                2
            } else if path.contains(term) {
                1
            } else {
                0
            }
        })
        .collect();
    let matches = match mode {
        MatchMode::All => scores.iter().all(|s| *s > 0),
        MatchMode::Any => scores.iter().any(|s| *s > 0),
    };
    matches.then(|| scores.iter().sum())
}

pub fn open(object: &FileObject) -> Result<serde_json::Value> {
    open_using(object, platform::open_document)
}
fn open_using(
    object: &FileObject,
    executor: impl FnOnce(&Path, &crate::core::FileIdentity) -> Result<()>,
) -> Result<serde_json::Value> {
    // The session decides authorization and the platform pins/rechecks identity.
    // Windows chooses the association; no extension-derived verb or parameters.
    executor(&object.path, &object.identity)?;
    Ok(
        serde_json::json!({"verification":"handoff", "status":"accepted", "path":object.path,
        "message":"系统已接受打开请求；尚未验证应用窗口或文件显示状态"}),
    )
}

#[cfg(test)]
mod open_tests {
    use super::*;
    #[test]
    fn system_open_handoff_has_no_extension_filter_and_keeps_failure_truthful() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["fixture.html", "fixture.exe", "fixture.unknown", "fixture"] {
            let path = dir.path().join(name);
            std::fs::write(&path, b"fictional, never executed").unwrap();
            let object = FileObject {
                id: "fixture".into(),
                identity: platform::identity(&path).unwrap(),
                path,
                observed_at: now(),
                modified_at: None,
            };
            let reply = open_using(&object, |path, identity| {
                assert_eq!(path, &object.path);
                assert_eq!(identity.index, object.identity.index);
                Ok(())
            })
            .unwrap();
            assert_eq!(reply["verification"], "handoff");
            assert!(open_using(&object, |_, _| anyhow::bail!("no association")).is_err());
        }
        let object = FileObject {
            id: "directory".into(),
            path: dir.path().into(),
            identity: platform::identity(dir.path()).unwrap(),
            observed_at: now(),
            modified_at: None,
        };
        assert!(
            open_using(&object, |_, identity| {
                assert!(identity.directory);
                Ok(())
            })
            .is_ok()
        );
    }
}
