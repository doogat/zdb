//! Read-only connection pool for concurrent query execution.
//!
//! Routes read operations to `spawn_blocking` tasks with fresh
//! `Index` and `GitRepo` handles, bypassing the single-writer actor.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use zdb_core::error::ZettelError;
use zdb_core::git_ops::GitRepo;
use zdb_core::indexer::Index;
use zdb_core::sql_engine::{SqlEngine, SqlResult};
use zdb_core::types::{
    AttachmentInfo, PaginatedSearchResult, ParsedZettel, TableSchema, ZettelId,
};

use crate::actor;

type Result<T> = std::result::Result<T, ZettelError>;

/// Pool of read-only connections for concurrent query execution.
///
/// Each read acquires a semaphore permit and runs on `spawn_blocking`
/// with its own `Index` + `GitRepo` handles. Write operations must
/// still go through [`crate::actor::ActorHandle`].
#[derive(Clone)]
pub struct ReadPool {
    inner: Arc<Inner>,
}

struct Inner {
    repo_path: PathBuf,
    db_path: PathBuf,
    redb_path: PathBuf,
    semaphore: Arc<Semaphore>,
}

impl ReadPool {
    pub fn new(repo_path: PathBuf, pool_size: usize) -> Result<Self> {
        let db_path = repo_path.join(".zdb/index.db");
        let redb_path = repo_path.join(".zdb/nosql.redb");

        // Validate resources can be opened
        let _ = GitRepo::open(&repo_path)?;
        let _ = Index::open(&db_path)?;

        Ok(Self {
            inner: Arc::new(Inner {
                repo_path,
                db_path,
                redb_path,
                semaphore: Arc::new(Semaphore::new(pool_size)),
            }),
        })
    }

    pub fn default_pool_size() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get().min(4))
            .unwrap_or(2)
    }

    // --- Index + GitRepo reads ---

    pub async fn get_zettel(&self, id: String) -> Result<ParsedZettel> {
        self.with_index_repo(move |index, repo| actor::get_zettel(repo, index, &id)).await
    }

    pub async fn list_zettels(
        &self,
        zettel_type: Option<String>,
        tag: Option<String>,
        backlinks_of: Option<String>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<ParsedZettel>> {
        self.with_index_repo(move |index, repo| {
            actor::list_zettels(repo, index, zettel_type, tag, backlinks_of, limit, offset)
        })
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn filtered_list(
        &self,
        table_name: String,
        where_sql: String,
        params: Vec<rusqlite::types::Value>,
        order_sql: Option<String>,
        tag: Option<String>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<ParsedZettel>> {
        self.with_index_repo(move |index, repo| {
            actor::filtered_list(
                repo,
                index,
                &table_name,
                &where_sql,
                &params,
                order_sql.as_deref(),
                tag.as_deref(),
                limit,
                offset,
            )
        })
        .await
    }

    pub async fn get_type_schemas(&self) -> Result<Vec<TableSchema>> {
        self.with_index_repo(move |index, repo| actor::get_type_schemas(repo, index))
            .await
    }

    pub async fn execute_select(&self, sql: String) -> Result<SqlResult> {
        self.with_index_repo(move |index, repo| {
            let mut engine = SqlEngine::new(index, repo);
            engine.execute(&sql)
        })
        .await
    }

    pub async fn list_attachments(&self, zettel_id: String) -> Result<Vec<AttachmentInfo>> {
        self.with_index_repo(move |_index, repo| {
            let id = ZettelId(zettel_id);
            zdb_core::attachments::list_attachments(repo, &id)
        })
        .await
    }

    // --- Index-only reads ---

    pub async fn search(
        &self,
        query: String,
        limit: usize,
        offset: usize,
    ) -> Result<PaginatedSearchResult> {
        self.with_index(move |index| index.search_paginated(&query, limit, offset))
            .await
    }

    pub async fn count_zettels(
        &self,
        zettel_type: Option<String>,
        tag: Option<String>,
        backlinks_of: Option<String>,
    ) -> Result<i64> {
        self.with_index(move |index| actor::count_zettels(index, zettel_type, tag, backlinks_of))
            .await
    }

    pub async fn get_backlinks(&self, id: String) -> Result<Vec<String>> {
        self.with_index(move |index| index.backlinks(&id)).await
    }

    pub async fn aggregate_query(
        &self,
        sql: String,
        params: Vec<rusqlite::types::Value>,
    ) -> Result<Vec<String>> {
        self.with_index(move |index| {
            index
                .query_raw_with_params(&sql, &params)
                .map(|rows| rows.into_iter().next().unwrap_or_default())
        })
        .await
    }

    // --- NoSQL (redb) reads ---

    pub async fn nosql_get(&self, id: String) -> Result<Option<ParsedZettel>> {
        self.with_redb(move |redb| redb.get(&id)).await
    }

    pub async fn nosql_scan_type(&self, type_name: String) -> Result<Vec<String>> {
        self.with_redb(move |redb| redb.scan_by_type(&type_name))
            .await
    }

    pub async fn nosql_scan_tag(&self, tag: String) -> Result<Vec<String>> {
        self.with_redb(move |redb| redb.scan_by_tag(&tag)).await
    }

    pub async fn nosql_backlinks(&self, id: String) -> Result<Vec<String>> {
        self.with_redb(move |redb| redb.backlinks(&id)).await
    }

    // --- Dispatch helpers ---

    async fn acquire(&self) -> Result<OwnedSemaphorePermit> {
        Arc::clone(&self.inner.semaphore)
            .acquire_owned()
            .await
            .map_err(|_| ZettelError::Validation("read pool closed".into()))
    }

    async fn with_index<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Index) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let permit = self.acquire().await?;
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let index = Index::open(&inner.db_path)?;
            f(&index)
        })
        .await
        .map_err(|e| ZettelError::Validation(format!("read task panicked: {e}")))?
    }

    async fn with_index_repo<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Index, &GitRepo) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let permit = self.acquire().await?;
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let index = Index::open(&inner.db_path)?;
            let repo = GitRepo::open(&inner.repo_path)?;
            f(&index, &repo)
        })
        .await
        .map_err(|e| ZettelError::Validation(format!("read task panicked: {e}")))?
    }

    async fn with_redb<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&zdb_core::nosql::RedbIndex) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let permit = self.acquire().await?;
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let redb = zdb_core::nosql::RedbIndex::open(&inner.redb_path)?;
            f(&redb)
        })
        .await
        .map_err(|e| ZettelError::Validation(format!("read task panicked: {e}")))?
    }
}
