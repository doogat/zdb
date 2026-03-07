use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Zettel content helpers (mirrors zdb-core bench helpers)
// ---------------------------------------------------------------------------

const ZETTEL_COUNT: usize = 200;

fn zettel_content(i: usize) -> String {
    let word = match i % 5 {
        0 => "architecture",
        1 => "refactoring",
        2 => "deployment",
        3 => "performance",
        _ => "documentation",
    };
    format!(
        "---\ntitle: Note about {word} {i}\ndate: 2026-01-01\ntags:\n  - bench\n  - {word}\n---\n\
         This zettel discusses {word} in the context of item {i}.\n\
         Some additional body text for search indexing.\n\
         ---\n- source:: bench-{i}"
    )
}

fn zettel_path(i: usize) -> String {
    format!("zettelkasten/{:014}.md", 20260101000000u64 + i as u64)
}

// ---------------------------------------------------------------------------
// Lightweight server harness (mirrors tests/e2e/common.rs ServerGuard)
// ---------------------------------------------------------------------------

fn zdb_bin() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop(); // zdb-server/ -> workspace root
    let target = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dir.join("target"));
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    target.join(profile).join(format!("zdb{}", std::env::consts::EXE_SUFFIX))
}

static PORT_COUNTER: AtomicU16 = AtomicU16::new(18200);

struct BenchServer {
    child: Child,
    port: u16,
    token: String,
    _dir: TempDir,
}

impl BenchServer {
    fn start(dir: TempDir) -> Self {
        let port = PORT_COUNTER.fetch_add(2, Ordering::SeqCst);
        let pg_port = port + 1;

        let mut child = std::process::Command::new(zdb_bin())
            .arg("--repo")
            .arg(dir.path())
            .arg("serve")
            .arg("--port")
            .arg(port.to_string())
            .arg("--pg-port")
            .arg(pg_port.to_string())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to start zdb server (run `cargo build -p zdb-cli --release` first)");

        let stderr = child.stderr.take().unwrap();
        let reader = BufReader::new(stderr);
        let mut token = String::new();
        let mut http_ready = false;
        let mut pg_ready = false;

        for line in reader.lines() {
            let line = line.unwrap();
            if line.contains("auth token:") {
                if let Some(path) = line.split("auth token: ").nth(1) {
                    token = std::fs::read_to_string(path.trim())
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                }
            }
            if line.contains("pgwire listening on") {
                pg_ready = true;
            } else if line.contains("listening on") {
                http_ready = true;
            }
            if http_ready && pg_ready {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));

        Self {
            child,
            port,
            token,
            _dir: dir,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/graphql", self.port)
    }
}

impl Drop for BenchServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// Test repo seeding
// ---------------------------------------------------------------------------

fn seed_repo(dir: &Path) {
    // Init via CLI
    let out = std::process::Command::new(zdb_bin())
        .arg("init")
        .arg(dir)
        .output()
        .expect("zdb init failed");
    assert!(out.status.success(), "zdb init: {}", String::from_utf8_lossy(&out.stderr));

    // Seed zettels by writing files + git commit
    for i in 0..ZETTEL_COUNT {
        let path = dir.join(zettel_path(i));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, zettel_content(i)).unwrap();
    }
    let status = std::process::Command::new("git")
        .current_dir(dir)
        .args(["add", "."])
        .status()
        .unwrap();
    assert!(status.success());
    let status = std::process::Command::new("git")
        .current_dir(dir)
        .args(["commit", "-m", "seed"])
        .env("GIT_AUTHOR_NAME", "bench")
        .env("GIT_AUTHOR_EMAIL", "bench@test")
        .env("GIT_COMMITTER_NAME", "bench")
        .env("GIT_COMMITTER_EMAIL", "bench@test")
        .status()
        .unwrap();
    assert!(status.success());
}

fn setup_server() -> BenchServer {
    let dir = TempDir::new().unwrap();
    seed_repo(dir.path());
    BenchServer::start(dir)
}

// ---------------------------------------------------------------------------
// Async helpers
// ---------------------------------------------------------------------------

fn build_client(token: &str) -> reqwest::Client {
    reqwest::Client::builder()
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(
                "Authorization",
                format!("Bearer {token}").parse().unwrap(),
            );
            h
        })
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
}

async fn graphql_request(
    client: &reqwest::Client,
    url: &str,
    query: &str,
) -> reqwest::Result<serde_json::Value> {
    let body = serde_json::json!({ "query": query });
    client
        .post(url)
        .json(&body)
        .send()
        .await?
        .json()
        .await
}

/// Send `n` concurrent copies of `query` and return all results.
async fn concurrent_reads(
    client: &reqwest::Client,
    url: &str,
    query: &str,
    n: usize,
) -> Vec<serde_json::Value> {
    let mut handles = Vec::with_capacity(n);
    for _ in 0..n {
        let c = client.clone();
        let u = url.to_string();
        let q = query.to_string();
        handles.push(tokio::spawn(async move {
            graphql_request(&c, &u, &q).await.unwrap()
        }));
    }
    let mut results = Vec::with_capacity(n);
    for h in handles {
        results.push(h.await.unwrap());
    }
    results
}

// ---------------------------------------------------------------------------
// GraphQL queries
// ---------------------------------------------------------------------------

const Q_LIST: &str = "{ zettels(limit: 20) { id title } }";
const Q_SEARCH: &str = r#"{ search(query: "architecture", limit: 10) { items { id title } totalCount } }"#;

fn q_get(i: usize) -> String {
    format!(
        r#"{{ zettel(id: "{:014}") {{ id title body }} }}"#,
        20260101000000u64 + i as u64
    )
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_single_reads(c: &mut Criterion) {
    let server = setup_server();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = build_client(&server.token);
    let url = server.url();

    let mut group = c.benchmark_group("server/single_read");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("list_zettels", |b| {
        b.iter(|| {
            rt.block_on(graphql_request(&client, &url, Q_LIST)).unwrap();
        });
    });

    group.bench_function("get_zettel", |b| {
        b.iter(|| {
            rt.block_on(graphql_request(&client, &url, &q_get(0))).unwrap();
        });
    });

    group.bench_function("search", |b| {
        b.iter(|| {
            rt.block_on(graphql_request(&client, &url, Q_SEARCH)).unwrap();
        });
    });

    group.finish();
}

fn bench_concurrent_reads(c: &mut Criterion) {
    let server = setup_server();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = build_client(&server.token);
    let url = server.url();

    let mut group = c.benchmark_group("server/concurrent_reads");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(15));

    for concurrency in [1, 4, 8, 16] {
        group.throughput(Throughput::Elements(concurrency as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            &concurrency,
            |b, &n| {
                b.iter(|| {
                    rt.block_on(concurrent_reads(&client, &url, Q_LIST, n));
                });
            },
        );
    }

    group.finish();
}

fn bench_concurrent_search(c: &mut Criterion) {
    let server = setup_server();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = build_client(&server.token);
    let url = server.url();

    let mut group = c.benchmark_group("server/concurrent_search");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(15));

    for concurrency in [1, 4, 8, 16] {
        group.throughput(Throughput::Elements(concurrency as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            &concurrency,
            |b, &n| {
                b.iter(|| {
                    rt.block_on(concurrent_reads(&client, &url, Q_SEARCH, n));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_single_reads,
    bench_concurrent_reads,
    bench_concurrent_search
);
criterion_main!(benches);
