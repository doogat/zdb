use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::ZettelError;
use crate::git_ops::GitRepo;
use crate::indexer::Index;
use crate::parser;
use crate::sync_manager::SyncManager;

/// FFI error enum exposed to Swift/Kotlin via UniFFI.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ZdbError {
    #[error("Git: {msg}")]
    Git { msg: String },
    #[error("Yaml: {msg}")]
    Yaml { msg: String },
    #[error("Sql: {msg}")]
    Sql { msg: String },
    #[error("Automerge: {msg}")]
    Automerge { msg: String },
    #[error("Io: {msg}")]
    Io { msg: String },
    #[error("Parse: {msg}")]
    Parse { msg: String },
    #[error("NotFound: {msg}")]
    NotFound { msg: String },
    #[error("Config: {msg}")]
    Config { msg: String },
    #[error("Validation: {msg}")]
    Validation { msg: String },
    #[error("SqlEngine: {msg}")]
    SqlEngine { msg: String },
    #[error("VersionMismatch: {msg}")]
    VersionMismatch { msg: String },
}

impl From<ZettelError> for ZdbError {
    fn from(e: ZettelError) -> Self {
        match e {
            ZettelError::Git(msg) => ZdbError::Git { msg },
            ZettelError::Yaml(msg) => ZdbError::Yaml { msg },
            ZettelError::Sql(msg) => ZdbError::Sql { msg },
            ZettelError::Automerge(msg) => ZdbError::Automerge { msg },
            ZettelError::Io(e) => ZdbError::Io { msg: e.to_string() },
            ZettelError::Toml(msg) => ZdbError::Config { msg },
            ZettelError::Parse(msg) => ZdbError::Parse { msg },
            ZettelError::NotFound(msg) => ZdbError::NotFound { msg },
            ZettelError::Validation(msg) => ZdbError::Validation { msg },
            ZettelError::InvalidPath(msg) => ZdbError::Validation { msg },
            ZettelError::SqlEngine(msg) => ZdbError::SqlEngine { msg },
            ZettelError::VersionMismatch { repo, driver } => ZdbError::VersionMismatch {
                msg: format!("repo format v{repo}, driver supports up to v{driver}"),
            },
            #[cfg(feature = "nosql")]
            ZettelError::Redb(msg) => ZdbError::Io { msg },
        }
    }
}

/// FFI-safe search result.
#[derive(uniffi::Record)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub path: String,
    pub snippet: String,
    pub rank: f64,
}

impl From<crate::types::SearchResult> for SearchResult {
    fn from(r: crate::types::SearchResult) -> Self {
        Self {
            id: r.id,
            title: r.title,
            path: r.path,
            snippet: r.snippet,
            rank: r.rank,
        }
    }
}

/// FFI-safe paginated search result.
#[derive(uniffi::Record)]
pub struct PaginatedSearchResult {
    pub hits: Vec<SearchResult>,
    pub total_count: u64,
}

impl From<crate::types::PaginatedSearchResult> for PaginatedSearchResult {
    fn from(r: crate::types::PaginatedSearchResult) -> Self {
        Self {
            hits: r.hits.into_iter().map(Into::into).collect(),
            total_count: r.total_count as u64,
        }
    }
}

/// FFI-safe rebuild report.
#[derive(uniffi::Record)]
pub struct RebuildReport {
    pub indexed: u64,
    pub tables_materialized: u64,
    pub types_inferred: Vec<String>,
}

/// FFI-safe attachment metadata.
#[derive(uniffi::Record)]
pub struct AttachmentInfo {
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub path: String,
}

impl From<crate::types::AttachmentInfo> for AttachmentInfo {
    fn from(a: crate::types::AttachmentInfo) -> Self {
        Self {
            name: a.name,
            mime: a.mime,
            size: a.size,
            path: a.path,
        }
    }
}

/// High-level facade composing GitRepo + Index for mobile/desktop FFI consumers.
#[derive(uniffi::Object)]
pub struct ZettelDriver {
    repo: Mutex<GitRepo>,
    index: Mutex<Index>,
    #[allow(dead_code)]
    repo_path: PathBuf,
}

#[uniffi::export]
impl ZettelDriver {
    /// Open an existing ZettelDB repository.
    #[uniffi::constructor]
    pub fn new(repo_path: String) -> Result<Self, ZdbError> {
        let path = Path::new(&repo_path);
        let repo = GitRepo::open(path).map_err(ZdbError::from)?;
        let db_dir = path.join(".zdb");
        std::fs::create_dir_all(&db_dir)
            .map_err(|e| ZdbError::from(ZettelError::Io(e)))?;
        let db_path = db_dir.join("index.db");
        let index = Index::open(&db_path).map_err(ZdbError::from)?;
        Ok(Self {
            repo: Mutex::new(repo),
            index: Mutex::new(index),
            repo_path: path.to_path_buf(),
        })
    }

    /// Initialize a new ZettelDB repository at `repo_path` and open it.
    #[uniffi::constructor]
    pub fn create_repo(repo_path: String) -> Result<Self, ZdbError> {
        let path = Path::new(&repo_path);
        GitRepo::init(path).map_err(ZdbError::from)?;
        Self::new(repo_path)
    }

    pub fn create_zettel(&self, content: String, message: String) -> Result<String, ZdbError> {
        let parsed = parser::parse(&content, "new.md").map_err(ZdbError::from)?;
        let id = parsed
            .meta
            .id
            .as_ref()
            .map(|z| z.0.clone())
            .unwrap_or_else(|| parser::generate_id().0);
        let rel_path = format!("zettelkasten/{id}.md");

        let repo = self.repo.lock().unwrap();
        repo.commit_file(&rel_path, &content, &message)
            .map_err(ZdbError::from)?;

        let index = self.index.lock().unwrap();
        index.index_zettel(&parsed).map_err(ZdbError::from)?;

        Ok(id)
    }

    pub fn read_zettel(&self, id: String) -> Result<String, ZdbError> {
        let index = self.index.lock().unwrap();
        let path = index.resolve_path(&id).map_err(ZdbError::from)?;
        drop(index);

        let repo = self.repo.lock().unwrap();
        repo.read_file(&path).map_err(ZdbError::from)
    }

    pub fn update_zettel(
        &self,
        id: String,
        content: String,
        message: String,
    ) -> Result<(), ZdbError> {
        let index = self.index.lock().unwrap();
        let rel_path = index.resolve_path(&id).map_err(ZdbError::from)?;
        drop(index);

        let repo = self.repo.lock().unwrap();
        repo.commit_file(&rel_path, &content, &message)
            .map_err(ZdbError::from)?;

        let parsed = parser::parse(&content, &rel_path).map_err(ZdbError::from)?;
        let index = self.index.lock().unwrap();
        index.index_zettel(&parsed).map_err(ZdbError::from)
    }

    pub fn delete_zettel(&self, id: String, message: String) -> Result<(), ZdbError> {
        let index = self.index.lock().unwrap();
        let rel_path = index.resolve_path(&id).map_err(ZdbError::from)?;
        drop(index);

        let repo = self.repo.lock().unwrap();
        repo.delete_file(&rel_path, &message)
            .map_err(ZdbError::from)?;

        let index = self.index.lock().unwrap();
        index.remove_zettel(&id).map_err(ZdbError::from)
    }

    pub fn search(&self, query: String) -> Result<Vec<SearchResult>, ZdbError> {
        let index = self.index.lock().unwrap();
        let results = index.search(&query).map_err(ZdbError::from)?;
        Ok(results.into_iter().map(Into::into).collect())
    }

    pub fn search_paginated(
        &self,
        query: String,
        limit: u32,
        offset: u32,
    ) -> Result<PaginatedSearchResult, ZdbError> {
        let index = self.index.lock().unwrap();
        let result = index
            .search_paginated(&query, limit as usize, offset as usize)
            .map_err(ZdbError::from)?;
        Ok(result.into())
    }

    pub fn reindex(&self) -> Result<RebuildReport, ZdbError> {
        let repo = self.repo.lock().unwrap();
        let index = self.index.lock().unwrap();
        let report = index.rebuild(&*repo).map_err(ZdbError::from)?;
        Ok(RebuildReport {
            indexed: report.indexed as u64,
            tables_materialized: report.tables_materialized as u64,
            types_inferred: report.types_inferred,
        })
    }

    pub fn register_node(&self, name: String) -> Result<String, ZdbError> {
        let repo = self.repo.lock().unwrap();
        let node = crate::sync_manager::register_node(&repo, &name).map_err(ZdbError::from)?;
        Ok(node.uuid)
    }

    pub fn compact(&self) -> Result<(), ZdbError> {
        let repo = self.repo.lock().unwrap();
        let sync_mgr = SyncManager::open(&repo).map_err(ZdbError::from)?;
        crate::compaction::compact(&repo, &sync_mgr, true).map_err(ZdbError::from)?;
        Ok(())
    }

    pub fn list_zettels(&self) -> Result<Vec<String>, ZdbError> {
        let repo = self.repo.lock().unwrap();
        repo.list_zettels().map_err(ZdbError::from)
    }

    pub fn execute_sql(&self, sql: String) -> Result<String, ZdbError> {
        let index = self.index.lock().unwrap();
        let affected = index.execute_sql(&sql, &[]).map_err(ZdbError::from)?;
        Ok(affected.to_string())
    }

    pub fn attach_file(
        &self,
        zettel_id: String,
        file_path: String,
    ) -> Result<AttachmentInfo, ZdbError> {
        let bytes = std::fs::read(&file_path).map_err(|e| ZdbError::from(ZettelError::Io(e)))?;
        let filename = Path::new(&file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| ZdbError::Validation {
                msg: "invalid filename".into(),
            })?
            .to_owned();
        let mime = crate::types::AttachmentInfo::mime_from_filename(&filename).to_owned();
        let id = crate::types::ZettelId(zettel_id);
        let repo = self.repo.lock().unwrap();
        let index = self.index.lock().unwrap();
        let info = crate::attachments::attach_file(&repo, &index, &id, &filename, &bytes, &mime)
            .map_err(ZdbError::from)?;
        Ok(info.into())
    }

    pub fn detach_file(&self, zettel_id: String, filename: String) -> Result<(), ZdbError> {
        let id = crate::types::ZettelId(zettel_id);
        let repo = self.repo.lock().unwrap();
        let index = self.index.lock().unwrap();
        crate::attachments::detach_file(&repo, &index, &id, &filename).map_err(ZdbError::from)
    }

    pub fn list_attachments(&self, zettel_id: String) -> Result<Vec<AttachmentInfo>, ZdbError> {
        let id = crate::types::ZettelId(zettel_id);
        let repo = self.repo.lock().unwrap();
        let list = crate::attachments::list_attachments(&repo, &id).map_err(ZdbError::from)?;
        Ok(list.into_iter().map(AttachmentInfo::from).collect())
    }

    pub fn export_full_bundle(&self, output_path: String) -> Result<String, ZdbError> {
        let repo = self.repo.lock().unwrap();
        let sync_mgr = SyncManager::open(&repo).map_err(ZdbError::from)?;
        let path = crate::bundle::export_full_bundle(&repo, &sync_mgr, Path::new(&output_path))
            .map_err(ZdbError::from)?;
        Ok(path.to_string_lossy().into_owned())
    }

    pub fn import_bundle(&self, bundle_path: String) -> Result<(), ZdbError> {
        let repo = self.repo.lock().unwrap();
        let mut sync_mgr = SyncManager::open(&repo).map_err(ZdbError::from)?;
        let index = self.index.lock().unwrap();
        crate::bundle::import_bundle(&repo, &mut sync_mgr, &index, Path::new(&bundle_path))
            .map_err(ZdbError::from)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn init_creates_repo_and_opens_driver() {
        let tmp = TempDir::new().unwrap();
        let driver = ZettelDriver::create_repo(tmp.path().to_str().unwrap().to_string())
            .expect("init should succeed");
        let list = driver.list_zettels().unwrap();
        assert!(list.is_empty(), "fresh repo should have no zettels");
    }

    #[test]
    fn register_node_returns_uuid() {
        let tmp = TempDir::new().unwrap();
        let driver = ZettelDriver::create_repo(tmp.path().to_str().unwrap().to_string()).unwrap();
        let uuid = driver.register_node("TestNode".to_string()).unwrap();
        assert!(!uuid.is_empty(), "uuid should not be empty");
        assert_eq!(uuid.len(), 36, "uuid should be 36 chars");
    }
}
