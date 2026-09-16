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
}
impl Scope {
    pub fn new(read: &[PathBuf], write: &[PathBuf]) -> Result<Self> {
        let mut all = read.to_vec();
        all.extend_from_slice(write);
        let mut read: Vec<_> = all
            .iter()
            .map(|p| normalize_existing(p))
            .collect::<Result<_>>()?;
        read.sort();
        read.dedup();
        let write = write
            .iter()
            .map(|p| normalize_existing(p))
            .collect::<Result<Vec<_>>>()?;
        for path in read.iter().chain(&write) {
            ensure!(path.is_dir(), "范围必须是目录：{}", path.display());
        }
        Ok(Self { read, write })
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
            roots.iter().any(|r| path.starts_with(r)),
            "路径不在已配置的{}范围：{}",
            if write { "写入" } else { "读取" },
            path.display()
        );
        Ok(path)
    }
    pub fn destination(&self, path: &Path) -> Result<(PathBuf, Vec<PathBuf>)> {
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
        let mut target = self.check(&ancestor, true)?;
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
    entries: HashMap<String, FileObject>,
}
impl Objects {
    pub fn insert(&mut self, path: &Path) -> Result<FileObject> {
        ensure!(
            self.entries.len() < 2000,
            "当前会话对象已达 2000 项，请 /reset 后重新查询"
        );
        let identity = platform::identity(path)?;
        let modified_at = fs::metadata(path)?
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
    ModifiedDesc,
    Name,
    SizeDesc,
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
    pub coverage_note: &'static str,
}
pub fn search(
    request: &Search,
    scope: &Scope,
    objects: &mut Objects,
    cancel: &Cancellation,
) -> Result<SearchResult> {
    let limit = request.limit.unwrap_or(20);
    ensure!(
        (1..=50).contains(&limit) && request.page <= 100,
        "每页 1–50 项，页码 0–100"
    );
    ensure!(request.query.len() <= 512, "关键词过长");
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
    let roots = match &request.directory {
        Some(p) => vec![scope.check(p, false)?],
        None => scope.read.clone(),
    };
    ensure!(
        !roots.is_empty(),
        "尚未设置读取目录。使用 --read-root 或 dao-shell config 配置"
    );
    let start = Instant::now();
    let query = request.query.to_lowercase();
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
            if !entry
                .file_name()
                .to_string_lossy()
                .to_lowercase()
                .contains(&query)
            {
                continue;
            }
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
                found.push((entry.path().to_owned(), metadata.len(), modified));
            }
        }
    }
    match request.sort {
        Sort::ModifiedDesc => found.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0))),
        Sort::SizeDesc => found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))),
        Sort::Name => found.sort_by(|a, b| a.0.cmp(&b.0)),
    }
    let matched_in_scan = found.len();
    let offset = request.page * limit;
    let mut items = Vec::new();
    for (path, _, _) in found.into_iter().skip(offset).take(limit) {
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
        coverage_note: "仅在列出的目录内扫描名称和元数据；不读取正文；跳过链接/不可访问项，翻页时重新观察。",
    })
}

pub fn open(object: &FileObject) -> Result<serde_json::Value> {
    if !object.identity.directory {
        let ext = object
            .path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        // Positive allowlist: shortcuts, scripts and executables cannot pass as documents.
        if ![
            "pdf", "txt", "md", "csv", "json", "png", "jpg", "jpeg", "gif", "bmp", "webp", "mp3",
            "wav", "mp4", "mkv", "docx", "xlsx", "pptx", "odt", "ods", "odp",
        ]
        .contains(&ext.as_str())
        {
            bail!("首版不打开此类型；仅支持常见文档、图片、音视频与目录");
        }
    }
    platform::open_document(&object.path, &object.identity)?;
    Ok(
        serde_json::json!({"verification":"handoff", "status":"accepted", "path":object.path,
        "message":"系统已接受打开请求；尚未验证应用窗口或文件显示状态"}),
    )
}
