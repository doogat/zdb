//! Property-based tests for parser, CRDT resolver, and indexer.
//!
//! Uses proptest to systematically explore the input space.
//! Default case counts: parser 10_000, CRDT 1_000, indexer 500.
//! Override with PROPTEST_CASES env var (e.g. 100 for CI).

use std::collections::HashMap;
use proptest::prelude::*;
use zdb_core::crdt_resolver;
use zdb_core::indexer::Index;
use zdb_core::parser;
use zdb_core::traits::ZettelSource;
use zdb_core::types::{CommitHash, ConflictFile, Value, ZettelId, ZettelMeta};

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

/// Safe ASCII alphanumeric string (no YAML-special chars).
fn safe_word() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9]{0,15}".prop_map(|s| s)
}

/// Safe sentence for body text (no `---` on its own line, no `::` to avoid inline fields).
fn safe_sentence() -> impl Strategy<Value = String> {
    prop::collection::vec("[a-zA-Z0-9 ,\\.!?]{1,60}", 1..=3)
        .prop_map(|parts| parts.join(" "))
}

/// Generate a 14-digit timestamp ID.
fn arb_zettel_id() -> impl Strategy<Value = String> {
    // YYYYMMDDHHmmss — constrain to valid-ish ranges
    (2020u32..2030, 1u32..=12, 1u32..=28, 0u32..=23, 0u32..=59, 0u32..=59).prop_map(
        |(y, mo, d, h, mi, s)| format!("{y:04}{mo:02}{d:02}{h:02}{mi:02}{s:02}"),
    )
}

/// Generate a Value for extra frontmatter fields (leaf types only for safety).
fn arb_value_leaf() -> impl Strategy<Value = Value> {
    prop_oneof![
        safe_word().prop_map(Value::String),
        (0.0f64..1000.0).prop_map(|n| Value::Number((n * 100.0).round() / 100.0)),
        any::<bool>().prop_map(Value::Bool),
    ]
}

/// Generate a tag (simple alphanumeric).
fn arb_tag() -> impl Strategy<Value = String> {
    "[a-z]{2,10}"
}

/// Generate an extra-field key that won't collide with reserved frontmatter keys.
fn arb_extra_key() -> impl Strategy<Value = String> {
    // Prefix with "x" to avoid any collision with reserved keys
    safe_word().prop_map(|w| format!("x{w}"))
}

/// Generate a safe zettel type name (prefixed to avoid SQL reserved words).
fn arb_zettel_type() -> impl Strategy<Value = String> {
    safe_word().prop_map(|w| format!("zt{w}"))
}

/// Generate ZettelMeta.
fn arb_zettel_meta() -> impl Strategy<Value = ZettelMeta> {
    (
        arb_zettel_id(),
        prop::option::of(safe_word()),
        prop::option::of("[0-9]{4}-[0-9]{2}-[0-9]{2}"),
        prop::option::of(arb_zettel_type()),
        prop::collection::vec(arb_tag(), 0..=4),
        prop::collection::btree_map(arb_extra_key(), arb_value_leaf(), 0..=3),
    )
        .prop_map(|(id, title, date, ztype, tags, extra)| ZettelMeta {
            id: Some(ZettelId(id)),
            title,
            date,
            zettel_type: ztype,
            tags,
            extra,
        })
}

/// Generate a body paragraph (safe text, no zone-boundary triggers).
fn arb_paragraph() -> impl Strategy<Value = String> {
    safe_sentence()
}

/// Generate a multi-paragraph body.
fn arb_body() -> impl Strategy<Value = String> {
    prop::collection::vec(arb_paragraph(), 1..=5).prop_map(|paras| paras.join("\n\n"))
}

/// Generate a reference section line: `- key:: value`.
fn arb_ref_line() -> impl Strategy<Value = String> {
    (safe_word(), safe_word()).prop_map(|(k, v)| format!("- {k}:: {v}"))
}

/// Generate a reference section (0 or more ref lines).
fn arb_reference_section() -> impl Strategy<Value = String> {
    prop::collection::vec(arb_ref_line(), 0..=4).prop_map(|lines| lines.join("\n"))
}

/// Generate a complete zettel Markdown string from parts.
fn arb_zettel_markdown() -> impl Strategy<Value = String> {
    (arb_zettel_meta(), arb_body(), arb_reference_section())
        .prop_map(|(meta, body, ref_section)| build_zettel_markdown(&meta, &body, &ref_section))
}

// ---------------------------------------------------------------------------
// Parser properties
// ---------------------------------------------------------------------------

/// Helper: build markdown from meta + body + ref_section (shared by generators and tests).
fn build_zettel_markdown(meta: &ZettelMeta, body: &str, ref_section: &str) -> String {
    let mut out = String::from("---\n");

    if let Some(ref id) = meta.id {
        out.push_str(&format!("id: {}\n", id.0));
    }
    if let Some(ref title) = meta.title {
        out.push_str(&format!("title: {title}\n"));
    }
    if let Some(ref date) = meta.date {
        out.push_str(&format!("date: {date}\n"));
    }
    if !meta.tags.is_empty() {
        out.push_str("tags:\n");
        for tag in &meta.tags {
            out.push_str(&format!("  - {tag}\n"));
        }
    }
    if let Some(ref t) = meta.zettel_type {
        out.push_str(&format!("type: {t}\n"));
    }
    for (key, value) in &meta.extra {
        match value {
            Value::String(s) => out.push_str(&format!("{key}: {s}\n")),
            Value::Number(n) => out.push_str(&format!("{key}: {n}\n")),
            Value::Bool(b) => out.push_str(&format!("{key}: {b}\n")),
            _ => {}
        }
    }

    out.push_str("---\n");
    out.push_str(body);

    if !ref_section.is_empty() {
        out.push_str("\n---\n");
        out.push_str(ref_section);
    }

    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    /// Smoke: generated zettels are parseable.
    #[test]
    fn generated_zettels_parse(md in arb_zettel_markdown()) {
        let result = parser::parse(&md, "test.md");
        prop_assert!(result.is_ok(), "parse failed: {:?}\ninput:\n{}", result.err(), md);
    }

    /// Parse-serialize-parse idempotency: parse(serialize(parse(x))) == parse(x).
    #[test]
    fn parser_idempotency(md in arb_zettel_markdown()) {
        let parsed = parser::parse(&md, "test.md").unwrap();
        let serialized = parser::serialize(&parsed);
        let reparsed = parser::parse(&serialized, "test.md").unwrap();

        // Compare meta fields
        prop_assert_eq!(&parsed.meta.id, &reparsed.meta.id);
        prop_assert_eq!(&parsed.meta.title, &reparsed.meta.title);
        prop_assert_eq!(&parsed.meta.date, &reparsed.meta.date);
        prop_assert_eq!(&parsed.meta.zettel_type, &reparsed.meta.zettel_type);
        prop_assert_eq!(&parsed.meta.tags, &reparsed.meta.tags);
        prop_assert_eq!(&parsed.meta.extra, &reparsed.meta.extra);
        // Compare body and reference section
        prop_assert_eq!(&parsed.body, &reparsed.body);
        prop_assert_eq!(&parsed.reference_section, &reparsed.reference_section);
        // Second round-trip should be stable too
        let serialized2 = parser::serialize(&reparsed);
        prop_assert_eq!(&serialized, &serialized2);
    }

    /// Zone isolation: modifying body doesn't change frontmatter or references.
    #[test]
    fn parser_zone_isolation(
        md in arb_zettel_markdown(),
        extra_text in "[a-zA-Z ]{5,30}",
    ) {
        let parsed = parser::parse(&md, "test.md").unwrap();
        let mut mutated = parsed.clone();
        mutated.body.push_str(&format!("\n\n{extra_text}"));

        let serialized = parser::serialize(&mutated);
        let reparsed = parser::parse(&serialized, "test.md").unwrap();

        // Frontmatter unchanged
        prop_assert_eq!(&parsed.meta.id, &reparsed.meta.id);
        prop_assert_eq!(&parsed.meta.title, &reparsed.meta.title);
        prop_assert_eq!(&parsed.meta.date, &reparsed.meta.date);
        prop_assert_eq!(&parsed.meta.zettel_type, &reparsed.meta.zettel_type);
        prop_assert_eq!(&parsed.meta.tags, &reparsed.meta.tags);
        prop_assert_eq!(&parsed.meta.extra, &reparsed.meta.extra);
        // Reference section unchanged
        prop_assert_eq!(&parsed.reference_section, &reparsed.reference_section);
        // Body contains the appended text
        prop_assert!(reparsed.body.contains(&extra_text));
    }

    /// Reference section detection: `---` thematic breaks in body don't become ref boundaries.
    #[test]
    fn parser_no_false_ref_boundary(
        meta in arb_zettel_meta(),
        body_before in arb_paragraph(),
        body_after in arb_paragraph(),
    ) {
        // Build a zettel with `---` in the body (thematic break) but NO reference section
        let body = format!("{body_before}\n\nSome text here.\n\n---\n\n{body_after}");
        let md = build_zettel_markdown(&meta, &body, "");
        let parsed = parser::parse(&md, "test.md").unwrap();

        // The `---` in the body should NOT create a reference section because
        // the content after it doesn't match `- key:: value` pattern
        prop_assert!(
            parsed.reference_section.is_empty(),
            "false positive ref section detected: {:?}\ninput:\n{}",
            parsed.reference_section,
            md,
        );
        // Body should contain both parts
        prop_assert!(parsed.body.contains(&body_before));
        prop_assert!(parsed.body.contains(&body_after));
    }
}

/// Compare reference sections as sorted line sets (CRDT merge may reorder).
fn sorted_lines(s: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    lines.sort();
    lines
}

// ---------------------------------------------------------------------------
// CRDT generators
// ---------------------------------------------------------------------------

/// Generate a ConflictFile with non-overlapping frontmatter edits (different fields).
fn arb_conflict_divergent_fm() -> impl Strategy<Value = ConflictFile> {
    // Ours changes title, theirs adds a new extra field — no overlap
    (arb_zettel_meta(), safe_word(), safe_word(), safe_word()).prop_map(
        |(meta, new_title, extra_key, extra_val)| {
            let ancestor_md = build_zettel_markdown(&meta, "Body text.", "");
            let mut meta_a = meta.clone();
            meta_a.title = Some(new_title);
            let mut meta_b = meta;
            let extra_key = format!("xConflict{extra_key}");
            meta_b
                .extra
                .insert(extra_key, Value::String(extra_val));
            let ours = build_zettel_markdown(&meta_a, "Body text.", "");
            let theirs = build_zettel_markdown(&meta_b, "Body text.", "");

            ConflictFile {
                path: "zettelkasten/test.md".into(),
                ancestor: Some(ancestor_md),
                ours,
                theirs,
                ours_hlc: None,
                theirs_hlc: None,
            }
        },
    )
}

/// Generate a ConflictFile with non-overlapping body edits.
fn arb_conflict_body_edits() -> impl Strategy<Value = ConflictFile> {
    (
        arb_zettel_meta(),
        arb_paragraph(),
        arb_paragraph(),
        arb_paragraph(),
        safe_sentence(),
        safe_sentence(),
    )
        .prop_map(|(meta, para1, para2, para3, edit_first, edit_last)| {
            let body = format!("{para1}\n\n{para2}\n\n{para3}");
            let ancestor_md = build_zettel_markdown(&meta, &body, "");

            let body_ours = format!("{edit_first}\n\n{para2}\n\n{para3}");
            let body_theirs = format!("{para1}\n\n{para2}\n\n{edit_last}");
            let ours = build_zettel_markdown(&meta, &body_ours, "");
            let theirs = build_zettel_markdown(&meta, &body_theirs, "");

            ConflictFile {
                path: "zettelkasten/test.md".into(),
                ancestor: Some(ancestor_md),
                ours,
                theirs,
                ours_hlc: None,
                theirs_hlc: None,
            }
        })
}

/// Generate a ConflictFile with concurrent reference additions.
fn arb_conflict_ref_additions() -> impl Strategy<Value = ConflictFile> {
    (
        arb_zettel_meta(),
        arb_body(),
        safe_word(),
        safe_word(),
        safe_word(),
        safe_word(),
    )
        .prop_map(|(meta, body, key_a, val_a, key_b, val_b)| {
            let ancestor_md = build_zettel_markdown(&meta, &body, "");

            // Use prefixed keys to avoid cross-zone inline field duplicates
            let ref_ours = format!("- rOurs{key_a}:: {val_a}");
            let ref_theirs = format!("- rTheirs{key_b}:: {val_b}");
            let ours = build_zettel_markdown(&meta, &body, &ref_ours);
            let theirs = build_zettel_markdown(&meta, &body, &ref_theirs);

            ConflictFile {
                path: "zettelkasten/test.md".into(),
                ancestor: Some(ancestor_md),
                ours,
                theirs,
                ours_hlc: None,
                theirs_hlc: None,
            }
        })
}

// ---------------------------------------------------------------------------
// CRDT properties
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1_000))]

    /// Merge commutativity: swapping ours/theirs produces same result.
    #[test]
    fn crdt_commutativity(conflict in arb_conflict_divergent_fm()) {
        let swapped = ConflictFile {
            path: conflict.path.clone(),
            ancestor: conflict.ancestor.clone(),
            ours: conflict.theirs.clone(),
            theirs: conflict.ours.clone(),
            ours_hlc: conflict.theirs_hlc.clone(),
            theirs_hlc: conflict.ours_hlc.clone(),
        };

        let result_ab = crdt_resolver::resolve_conflicts(vec![conflict], None).unwrap();
        let result_ba = crdt_resolver::resolve_conflicts(vec![swapped], None).unwrap();

        prop_assert_eq!(result_ab.len(), 1);
        prop_assert_eq!(result_ba.len(), 1);
        prop_assert_eq!(&result_ab[0].content, &result_ba[0].content);
    }

    /// Body-edit commutativity: swapping ours/theirs body edits produces same result.
    #[test]
    fn crdt_commutativity_body(conflict in arb_conflict_body_edits()) {
        let swapped = ConflictFile {
            path: conflict.path.clone(),
            ancestor: conflict.ancestor.clone(),
            ours: conflict.theirs.clone(),
            theirs: conflict.ours.clone(),
            ours_hlc: conflict.theirs_hlc.clone(),
            theirs_hlc: conflict.ours_hlc.clone(),
        };

        let result_ab = crdt_resolver::resolve_conflicts(vec![conflict], None).unwrap();
        let result_ba = crdt_resolver::resolve_conflicts(vec![swapped], None).unwrap();

        prop_assert_eq!(result_ab.len(), 1);
        prop_assert_eq!(result_ba.len(), 1);
        prop_assert_eq!(&result_ab[0].content, &result_ba[0].content);
    }

    /// Reference-addition commutativity: swapping ours/theirs ref additions produces same result.
    #[test]
    fn crdt_commutativity_refs(conflict in arb_conflict_ref_additions()) {
        let swapped = ConflictFile {
            path: conflict.path.clone(),
            ancestor: conflict.ancestor.clone(),
            ours: conflict.theirs.clone(),
            theirs: conflict.ours.clone(),
            ours_hlc: conflict.theirs_hlc.clone(),
            theirs_hlc: conflict.ours_hlc.clone(),
        };

        let result_ab = crdt_resolver::resolve_conflicts(vec![conflict], None).unwrap();
        let result_ba = crdt_resolver::resolve_conflicts(vec![swapped], None).unwrap();

        prop_assert_eq!(result_ab.len(), 1);
        prop_assert_eq!(result_ba.len(), 1);
        // Compare structurally — ref line order may differ
        let merged_ab = parser::parse(&result_ab[0].content, "test.md").unwrap();
        let merged_ba = parser::parse(&result_ba[0].content, "test.md").unwrap();
        prop_assert_eq!(&merged_ab.meta.id, &merged_ba.meta.id);
        prop_assert_eq!(&merged_ab.meta.title, &merged_ba.meta.title);
        prop_assert_eq!(&merged_ab.meta.date, &merged_ba.meta.date);
        prop_assert_eq!(&merged_ab.meta.zettel_type, &merged_ba.meta.zettel_type);
        prop_assert_eq!(&merged_ab.meta.tags, &merged_ba.meta.tags);
        prop_assert_eq!(&merged_ab.meta.extra, &merged_ba.meta.extra);
        prop_assert_eq!(&merged_ab.body, &merged_ba.body);
        prop_assert_eq!(sorted_lines(&merged_ab.reference_section),
                        sorted_lines(&merged_ba.reference_section));
    }

    /// Merge idempotency: merging identical ours/theirs returns original.
    #[test]
    fn crdt_idempotency(md in arb_zettel_markdown()) {
        let conflict = ConflictFile {
            path: "zettelkasten/test.md".into(),
            ancestor: Some(md.clone()),
            ours: md.clone(),
            theirs: md.clone(),
            ours_hlc: None,
            theirs_hlc: None,
        };

        let result = crdt_resolver::resolve_conflicts(vec![conflict], None).unwrap();
        prop_assert_eq!(result.len(), 1);

        // Re-parse both and compare structurally (serialization may normalize)
        let original = parser::parse(&md, "test.md").unwrap();
        let merged = parser::parse(&result[0].content, "test.md").unwrap();

        prop_assert_eq!(&original.meta.id, &merged.meta.id);
        prop_assert_eq!(&original.meta.title, &merged.meta.title);
        prop_assert_eq!(&original.meta.tags, &merged.meta.tags);
        prop_assert_eq!(&original.body, &merged.body);
        // Reference lines may be reordered by set merge; compare as sorted sets
        let orig_refs = sorted_lines(&original.reference_section);
        let merged_refs = sorted_lines(&merged.reference_section);
        prop_assert_eq!(&orig_refs, &merged_refs);
    }

    /// Frontmatter field independence: changing field X doesn't affect field Y.
    #[test]
    fn crdt_field_independence(conflict in arb_conflict_divergent_fm()) {
        let ancestor = parser::parse(conflict.ancestor.as_ref().unwrap(), "test.md").unwrap();
        let ours = parser::parse(&conflict.ours, "test.md").unwrap();
        let theirs = parser::parse(&conflict.theirs, "test.md").unwrap();

        let result = crdt_resolver::resolve_conflicts(vec![conflict], None).unwrap();
        prop_assert_eq!(result.len(), 1);
        let merged = parser::parse(&result[0].content, "test.md").unwrap();

        // Ours changed title — merged should have ours' title
        prop_assert_eq!(&ours.meta.title, &merged.meta.title);
        // Theirs added extra field — merged should have it
        for (k, v) in &theirs.meta.extra {
            if !ancestor.meta.extra.contains_key(k) {
                prop_assert_eq!(merged.meta.extra.get(k), Some(v),
                    "theirs extra field {} missing in merge", k);
            }
        }
        // Fields unchanged by either side should match ancestor
        prop_assert_eq!(&ancestor.meta.id, &merged.meta.id);
        prop_assert_eq!(&ancestor.meta.date, &merged.meta.date);
        prop_assert_eq!(&ancestor.meta.zettel_type, &merged.meta.zettel_type);
        prop_assert_eq!(&ancestor.meta.tags, &merged.meta.tags);
        // Ancestor's original extra fields should survive
        for (k, v) in &ancestor.meta.extra {
            prop_assert_eq!(merged.meta.extra.get(k), Some(v),
                "ancestor extra field {} lost in merge", k);
        }
    }

    /// Non-overlapping body edits both survive merge.
    #[test]
    fn crdt_non_overlapping_body(conflict in arb_conflict_body_edits()) {
        // Extract what the edits were
        let ours_parsed = parser::parse(&conflict.ours, "test.md").unwrap();
        let theirs_parsed = parser::parse(&conflict.theirs, "test.md").unwrap();
        let ours_first_para = ours_parsed.body.lines().next().unwrap_or("").to_string();
        let theirs_last_para = theirs_parsed.body.lines().last().unwrap_or("").to_string();

        let result = crdt_resolver::resolve_conflicts(vec![conflict], None).unwrap();
        prop_assert_eq!(result.len(), 1);

        let merged = parser::parse(&result[0].content, "test.md").unwrap();
        // Both edits should be present
        prop_assert!(
            merged.body.contains(&ours_first_para),
            "ours edit missing: {:?}\nmerged body: {:?}",
            ours_first_para,
            merged.body,
        );
        prop_assert!(
            merged.body.contains(&theirs_last_para),
            "theirs edit missing: {:?}\nmerged body: {:?}",
            theirs_last_para,
            merged.body,
        );
    }

    /// Concurrent reference additions both survive merge.
    #[test]
    fn crdt_reference_union(conflict in arb_conflict_ref_additions()) {
        // Extract added ref keys
        let ours_parsed = parser::parse(&conflict.ours, "test.md").unwrap();
        let theirs_parsed = parser::parse(&conflict.theirs, "test.md").unwrap();

        let result = crdt_resolver::resolve_conflicts(vec![conflict], None).unwrap();
        prop_assert_eq!(result.len(), 1);

        let merged = parser::parse(&result[0].content, "test.md").unwrap();
        // Both ref sections should be present in merged
        for line in ours_parsed.reference_section.lines() {
            if !line.trim().is_empty() {
                prop_assert!(
                    merged.reference_section.contains(line),
                    "ours ref line missing: {:?}\nmerged ref: {:?}",
                    line,
                    merged.reference_section,
                );
            }
        }
        for line in theirs_parsed.reference_section.lines() {
            if !line.trim().is_empty() {
                prop_assert!(
                    merged.reference_section.contains(line),
                    "theirs ref line missing: {:?}\nmerged ref: {:?}",
                    line,
                    merged.reference_section,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Indexer mock + generators
// ---------------------------------------------------------------------------

/// In-memory mock ZettelSource for integration test use (mirrors traits::mock::MockSource).
struct MockSource {
    files: HashMap<String, String>,
    head: String,
}

impl MockSource {
    fn new() -> Self {
        Self {
            files: HashMap::new(),
            head: "abc123".to_string(),
        }
    }
}

impl ZettelSource for MockSource {
    fn list_zettels(&self) -> zdb_core::error::Result<Vec<String>> {
        let mut paths: Vec<String> = self
            .files
            .keys()
            .filter(|p| p.starts_with("zettelkasten/") && p.ends_with(".md"))
            .cloned()
            .collect();
        paths.sort();
        Ok(paths)
    }

    fn read_file(&self, path: &str) -> zdb_core::error::Result<String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| zdb_core::error::ZettelError::NotFound(path.to_string()))
    }

    fn head_oid(&self) -> zdb_core::error::Result<CommitHash> {
        Ok(CommitHash(self.head.clone()))
    }

    fn diff_paths(&self, _old_oid: &str, _new_oid: &str) -> zdb_core::error::Result<Vec<(zdb_core::types::DiffKind, String)>> {
        Ok(Vec::new())
    }
}

/// Generate a set of random zettels as (path, content) pairs with unique IDs.
fn arb_zettel_set(count: std::ops::Range<usize>) -> impl Strategy<Value = Vec<(String, String)>> {
    prop::collection::vec(
        (arb_zettel_meta(), arb_body()),
        count,
    )
    .prop_map(|items| {
        items
            .into_iter()
            .enumerate()
            .map(|(i, (mut meta, body))| {
                // Ensure unique IDs by appending index
                let id = format!("2025010100{:04}", i);
                meta.id = Some(ZettelId(id.clone()));
                let md = build_zettel_markdown(&meta, &body, "");
                (format!("zettelkasten/{id}.md"), md)
            })
            .collect()
    })
}

// ---------------------------------------------------------------------------
// Indexer properties
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Index-rebuild equivalence: sequential index_zettel == full rebuild.
    #[test]
    fn indexer_equivalence(zettels in arb_zettel_set(3..10)) {
        // Build a MockSource
        let mut source = MockSource::new();
        for (path, content) in &zettels {
            source.files.insert(path.clone(), content.clone());
        }

        // Index A: sequential index_zettel calls
        let dir_a = tempfile::TempDir::new().unwrap();
        let idx_a = Index::open(&dir_a.path().join("a.db")).unwrap();
        for (path, content) in &zettels {
            let parsed = parser::parse(content, path).unwrap();
            idx_a.index_zettel(&parsed).unwrap();
        }

        // Index B: full rebuild
        let dir_b = tempfile::TempDir::new().unwrap();
        let idx_b = Index::open(&dir_b.path().join("b.db")).unwrap();
        idx_b.rebuild(&source).unwrap();

        // Compare: query each zettel by ID
        for (path, _) in &zettels {
            let id = path
                .strip_prefix("zettelkasten/")
                .unwrap()
                .strip_suffix(".md")
                .unwrap();
            let sql = format!("SELECT id, title, type, body FROM zettels WHERE id = '{id}'");
            let rows_a = idx_a.query_raw(&sql).unwrap();
            let rows_b = idx_b.query_raw(&sql).unwrap();
            prop_assert_eq!(&rows_a, &rows_b, "mismatch for zettel {}", id);
        }
    }

    /// Staleness detection: index at commit X is stale after commit Y.
    #[test]
    fn indexer_staleness(
        zettels in arb_zettel_set(1..5),
        commit_x in "[0-9a-f]{40}",
        commit_y in "[0-9a-f]{40}",
    ) {
        let mut source = MockSource::new();
        source.head = commit_x.clone();
        for (path, content) in &zettels {
            source.files.insert(path.clone(), content.clone());
        }

        let dir = tempfile::TempDir::new().unwrap();
        let idx = Index::open(&dir.path().join("idx.db")).unwrap();
        idx.rebuild(&source).unwrap();

        // Same commit → not stale
        prop_assert!(!idx.is_stale(&source).unwrap());

        // Different commit → stale
        if commit_x != commit_y {
            source.head = commit_y;
            prop_assert!(idx.is_stale(&source).unwrap());
        }
    }
}
