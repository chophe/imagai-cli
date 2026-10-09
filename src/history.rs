//! Iteration-lineage store: the `.imagai-history.json` sidecar.
//!
//! The sidecar lives inside the configured `output_dir` and records one entry
//! per image that lands there (D-08). Every surface — CLI, TUI and web —
//! converges on [`generate_image_core`], which calls [`record_result`] on each
//! successful save, so what is recorded cannot drift between frontends
//! (BRCH-04).
//!
//! The store is deliberately simple: read-modify-write of the whole sidecar per
//! save, no locking, no journal, no compaction (D-11). It is a single-user
//! local tool, and a failed write is never fatal — lazy adoption (D-10)
//! recovers the node on the next read.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// Filename of the lineage sidecar inside `output_dir` (D-08).
pub const HISTORY_FILENAME: &str = ".imagai-history.json";

/// One persisted lineage record. Exactly what the sidecar holds — no derived
/// state. `id` and `filename` are the identity of a record and carry no
/// `#[serde(default)]`, so a hand-truncated sidecar still loads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub id: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub filename: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub created: String,
}

/// One node in the read view: a record plus state derived at read time.
#[derive(Debug, Clone, Serialize)]
pub struct HistoryNode {
    pub id: String,
    pub parent: Option<String>,
    pub filename: String,
    pub prompt: String,
    pub model: String,
    pub created: String,
    /// True when the node's file is absent from `output_dir` right now (D-20).
    /// Derived on every read, never persisted, so the sidecar stays truthful.
    pub missing: bool,
    pub children: Vec<HistoryNode>,
}

/// The read view shared by every surface. Deriving `Serialize` is what makes
/// the CLI's `--json` and the web API's `roots` byte-identical in shape — one
/// serializer, two callers (D-19, BRCH-04).
#[derive(Debug, Clone, Serialize)]
pub struct HistoryView {
    pub output_dir: String,
    pub nodes: usize,
    pub missing: usize,
    pub roots: Vec<HistoryNode>,
}

/// The node id recorded for `filename`, if any. Lets a surface that resolved a
/// source by name still name the node it belongs to.
pub fn node_id_for(dir: &Path, filename: &str) -> Option<String> {
    let records = read_records(dir).ok()?;
    records
        .iter()
        .find(|r| r.filename == filename)
        .map(|r| r.id.clone())
}

/// A resolved fork source, in the order `resolve_fork_source` tries (D-12).
#[derive(Debug)]
pub enum ForkSource {
    /// A recorded history node.
    Node { id: String, filename: String },
    /// A file on disk. `recorded` is true when the file was auto-adopted as a
    /// root node during resolution (D-12); false when it lives outside
    /// `output_dir` and therefore stays untracked (D-22).
    File { path: PathBuf, recorded: bool },
}

/// Path of the sidecar inside `dir`.
pub fn history_path(dir: &Path) -> PathBuf {
    dir.join(HISTORY_FILENAME)
}

/// Read the sidecar. A missing sidecar reads as an empty store; a sidecar that
/// exists but cannot be read or parsed is an error naming the path and the
/// reason — never a silent empty tree (the Copywriting Contract's
/// unreadable-store sentence).
pub fn read_records(dir: &Path) -> anyhow::Result<Vec<HistoryRecord>> {
    let path = history_path(dir);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    let records: Vec<HistoryRecord> = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("Could not read history at {}: {e}", path.display()))?;
    Ok(records)
}

/// The D-11 read-modify-write: read the whole sidecar, append, write it back.
/// No locking, no journal, no compaction — a locked decision.
pub fn append_records(dir: &Path, new: &[HistoryRecord]) -> anyhow::Result<()> {
    let mut records = read_records(dir)?;
    records.extend_from_slice(new);
    let json = serde_json::to_string_pretty(&records)?;
    std::fs::write(history_path(dir), format!("{json}\n"))?;
    Ok(())
}

/// A fresh 8-hex node id, regenerated while it collides with `existing` (D-17).
pub fn new_node_id(existing: &[String]) -> String {
    loop {
        let candidate = uuid::Uuid::new_v4().to_string()[..8].to_string();
        if !existing.contains(&candidate) {
            return candidate;
        }
    }
}

/// Reduce a record `filename` to a path inside `dir`.
///
/// Every consumer that turns a record filename into a path goes through this
/// reduction — the same one `web::output_dir_file` performs — so a hand-edited
/// sidecar carrying a `../` component cannot read or write outside `output_dir`
/// (T-02-01). A value with no usable final component yields `None` and is never
/// joined.
pub fn node_path(dir: &Path, filename: &str) -> Option<PathBuf> {
    let name = Path::new(filename).file_name().and_then(|n| n.to_str())?;
    if name.is_empty() {
        return None;
    }
    Some(dir.join(name))
}

/// Read the store and derive the tree.
///
/// A record whose `parent` is absent or names an unknown id becomes a root, so
/// a hand-edited sidecar cannot attach a subtree to nothing (T-02-02).
/// Derivation indexes the records by parent in a single pass rather than
/// recursing over them, and the builder carries a visited set, so a cyclic
/// `parent` chain cannot loop. Children are ordered oldest-first by
/// `(created, id)`; roots newest-first, descending.
pub fn load_view(dir: &Path) -> anyhow::Result<HistoryView> {
    let records = read_records(dir)?;
    let known: HashSet<&str> = records.iter().map(|r| r.id.as_str()).collect();

    let mut children_of: HashMap<&str, Vec<&HistoryRecord>> = HashMap::new();
    let mut roots: Vec<&HistoryRecord> = Vec::new();
    for r in &records {
        match r.parent.as_deref() {
            Some(p) if known.contains(p) && p != r.id.as_str() => {
                children_of.entry(p).or_default().push(r);
            }
            _ => roots.push(r),
        }
    }
    // `created` is fixed-width ISO, so string ordering is chronological.
    roots.sort_by(|a, b| (&b.created, &b.id).cmp(&(&a.created, &a.id)));
    for list in children_of.values_mut() {
        list.sort_by(|a, b| (&a.created, &a.id).cmp(&(&b.created, &b.id)));
    }

    let missing_count = records
        .iter()
        .filter(|r| match node_path(dir, &r.filename) {
            Some(p) => !p.is_file(),
            None => true,
        })
        .count();

    let mut seen: HashSet<String> = HashSet::new();
    let mut nodes: Vec<HistoryNode> = Vec::new();
    build_roots(&roots, dir, &children_of, &mut nodes, &mut seen);
    // A record stranded in a cyclic `parent` chain is reachable from no root;
    // listing it as its own root keeps every record visible (D-20) while the
    // visited set keeps derivation bounded (T-02-02).
    let mut stranded: Vec<&HistoryRecord> = records
        .iter()
        .filter(|r| !seen.contains(r.id.as_str()))
        .collect();
    stranded.sort_by(|a, b| (&b.created, &b.id).cmp(&(&a.created, &a.id)));
    build_roots(&stranded, dir, &children_of, &mut nodes, &mut seen);

    Ok(HistoryView {
        output_dir: dir.to_string_lossy().to_string(),
        nodes: records.len(),
        missing: missing_count,
        roots: nodes,
    })
}

fn build_roots(
    list: &[&HistoryRecord],
    dir: &Path,
    children_of: &HashMap<&str, Vec<&HistoryRecord>>,
    nodes: &mut Vec<HistoryNode>,
    seen: &mut HashSet<String>,
) {
    for r in list {
        if !seen.contains(r.id.as_str()) {
            nodes.push(build_node(r, dir, children_of, seen));
        }
    }
}

fn build_node(
    record: &HistoryRecord,
    dir: &Path,
    children_of: &HashMap<&str, Vec<&HistoryRecord>>,
    seen: &mut HashSet<String>,
) -> HistoryNode {
    seen.insert(record.id.clone());
    // Children already reached elsewhere are skipped, so a cyclic `parent`
    // chain in a hand-edited sidecar terminates and emits nothing twice
    // (T-02-02).
    let child_records: Vec<&HistoryRecord> = children_of
        .get(record.id.as_str())
        .map(|list| {
            list.iter()
                .filter(|c| !seen.contains(c.id.as_str()))
                .copied()
                .collect()
        })
        .unwrap_or_default();
    let children = child_records
        .into_iter()
        .map(|c| build_node(c, dir, children_of, seen))
        .collect();
    let missing = match node_path(dir, &record.filename) {
        Some(p) => !p.is_file(),
        None => true,
    };
    HistoryNode {
        id: record.id.clone(),
        parent: record.parent.clone(),
        filename: record.filename.clone(),
        prompt: record.prompt.clone(),
        model: record.model.clone(),
        created: record.created.clone(),
        missing,
        children,
    }
}

/// Record one saved result and return the new node's id.
///
/// The D-22 guard comes first: only a result whose parent directory is exactly
/// `dir` is recorded — `generate_image_core` builds the path with
/// `output_dir.join(...)`, so comparing `Path` values directly is enough and
/// canonicalisation would only break byte-identical paths. When the guard
/// fails, nothing is written and the empty id comes back.
pub fn record_result(
    dir: &Path,
    saved_path: &Path,
    prompt: &str,
    model: &str,
    source: Option<&str>,
) -> anyhow::Result<String> {
    if saved_path.parent() != Some(dir) {
        return Ok(String::new());
    }
    let Some(filename) = saved_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
    else {
        return Ok(String::new());
    };

    let records = read_records(dir)?;
    let existing: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    let id = new_node_id(&existing);
    // D-09: the parent is the node whose filename matches the source's own
    // basename; a plain generation (no source, or no match) is a root.
    let parent = source
        .and_then(|s| Path::new(s).file_name())
        .and_then(|n| n.to_str())
        .and_then(|name| records.iter().find(|r| r.filename == name))
        .map(|r| r.id.clone());

    let record = HistoryRecord {
        id: id.clone(),
        parent,
        filename,
        prompt: prompt.to_string(),
        model: model.to_string(),
        created: crate::utils::datetime_iso(SystemTime::now()),
    };
    append_records(dir, std::slice::from_ref(&record))?;
    Ok(id)
}

/// True when `s` is shaped like a node id or an id prefix: at most the 8
/// characters an id holds, and only lowercase hex. This keeps an ordinary
/// filename from being mistaken for an id in step (1) of the resolution.
fn is_id_shaped(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 8
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Adopt an untracked file as a root node and return its id (D-12). A file
/// with no PNG lineage has no prompt or model to recover, so both stay empty
/// and the renderers print `(unknown prompt)` / `(unknown model)`.
fn adopt_root(dir: &Path, path: &Path) -> anyhow::Result<String> {
    let Some(filename) = path.file_name().and_then(|n| n.to_str()).map(String::from) else {
        anyhow::bail!("Could not adopt '{}' — it has no filename.", path.display());
    };
    let records = read_records(dir)?;
    let existing: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    let id = new_node_id(&existing);
    append_records(
        dir,
        &[HistoryRecord {
            id: id.clone(),
            parent: None,
            filename,
            prompt: String::new(),
            model: String::new(),
            created: crate::utils::datetime_iso(SystemTime::now()),
        }],
    )?;
    Ok(id)
}

/// Resolve `from` to something forkable, in exactly this order (D-12):
///
/// 1. a unique id-prefix match against record ids;
/// 2. an exact `filename` match against a record;
/// 3. an existing file — auto-adopted as a root node when it sits in `dir`,
///    otherwise handed back untracked (D-22);
/// 4. otherwise a bail naming what is missing.
///
/// The three error sentences below are the Copywriting Contract's verbatim
/// strings and the only place they are produced: every surface formats them,
/// none rewords them.
pub fn resolve_fork_source(dir: &Path, from: &str) -> anyhow::Result<ForkSource> {
    let records = read_records(dir)?;

    // (1) id prefix
    if is_id_shaped(from) {
        let matches: Vec<&HistoryRecord> =
            records.iter().filter(|r| r.id.starts_with(from)).collect();
        match matches.as_slice() {
            [] => {}
            [only] => {
                return node_source(dir, only);
            }
            many => {
                let candidates = many
                    .iter()
                    .map(|r| r.id.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow::bail!("Ambiguous node id prefix '{from}' — candidates: {candidates}");
            }
        }
    }

    // (2) exact filename
    if let Some(record) = records.iter().find(|r| r.filename == from) {
        return node_source(dir, record);
    }

    // (3) an existing file: the literal path the operator named, or — when the
    // name is a bare filename — that same name inside `dir`. A value carrying a
    // separator is never joined onto `dir`: `is_plain_filename` refuses it on
    // the web surface, and a traversal-looking name is not silently reduced
    // here either.
    let literal = Path::new(from);
    if literal.is_file() {
        if literal.parent() == Some(dir) {
            adopt_root(dir, literal)?;
            return Ok(ForkSource::File {
                path: literal.to_path_buf(),
                recorded: true,
            });
        }
        return Ok(ForkSource::File {
            path: literal.to_path_buf(),
            recorded: false,
        });
    }
    if !from.contains('/') && !from.contains('\\') {
        let in_dir = dir.join(from);
        if in_dir.is_file() {
            adopt_root(dir, &in_dir)?;
            return Ok(ForkSource::File {
                path: in_dir,
                recorded: true,
            });
        }
    }

    // (4) nothing matched
    anyhow::bail!(
        "No history node or image named '{from}' in {} — run 'imagai history' to see what is available.",
        dir.to_string_lossy()
    )
}

/// A recorded node, refused when its file is gone. D-20 keeps the node in the
/// tree, but a fork cannot start from a file that is not there.
fn node_source(dir: &Path, record: &HistoryRecord) -> anyhow::Result<ForkSource> {
    let present = node_path(dir, &record.filename)
        .map(|p| p.is_file())
        .unwrap_or(false);
    if !present {
        anyhow::bail!(
            "History node {} points at '{}', which is no longer in {}.",
            record.id,
            record.filename,
            dir.to_string_lossy()
        );
    }
    Ok(ForkSource::Node {
        id: record.id.clone(),
        filename: record.filename.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn record(id: &str, parent: Option<&str>, filename: &str, created: &str) -> HistoryRecord {
        HistoryRecord {
            id: id.to_string(),
            parent: parent.map(String::from),
            filename: filename.to_string(),
            prompt: format!("prompt {id}"),
            model: "dall-e-3".to_string(),
            created: created.to_string(),
        }
    }

    fn write_store(dir: &Path, records: &[HistoryRecord]) {
        let json = serde_json::to_string_pretty(records).expect("serialize");
        std::fs::write(history_path(dir), json).expect("write sidecar");
    }

    #[test]
    fn absent_sidecar_reads_as_empty() {
        let dir = TempDir::new().unwrap();
        let records = read_records(dir.path()).expect("no sidecar is not an error");
        assert!(records.is_empty());
        let view = load_view(dir.path()).expect("empty view");
        assert_eq!(view.nodes, 0);
        assert_eq!(view.missing, 0);
        assert!(view.roots.is_empty());
    }

    #[test]
    fn malformed_sidecar_returns_the_named_error() {
        let dir = TempDir::new().unwrap();
        std::fs::write(history_path(dir.path()), b"{not json").expect("write corrupt sidecar");
        let err = read_records(dir.path()).expect_err("malformed sidecar is an error");
        let text = err.to_string();
        assert!(
            text.contains("Could not read history at"),
            "contract sentence: {text}"
        );
        assert!(text.contains(HISTORY_FILENAME), "names the path: {text}");
        assert!(load_view(dir.path()).is_err(), "never a silent empty tree");
    }

    #[test]
    fn new_node_id_is_eight_lowercase_hex_and_avoids_collisions() {
        let first = new_node_id(&[]);
        assert_eq!(first.len(), 8);
        assert!(first
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        assert_ne!(
            new_node_id(std::slice::from_ref(&first)),
            first,
            "collision regenerates"
        );
        let mut taken = vec![first];
        for _ in 0..16 {
            let candidate = new_node_id(&taken);
            assert!(
                !taken.contains(&candidate),
                "no collision with existing ids"
            );
            taken.push(candidate);
        }
    }

    #[test]
    fn load_view_orders_roots_newest_first_and_children_oldest_first() {
        let dir = TempDir::new().unwrap();
        write_store(
            dir.path(),
            &[
                record("aaa11111", None, "old.png", "2026-10-09T10:00:00"),
                record("bbb22222", None, "new.png", "2026-10-09T12:00:00"),
                record(
                    "ccc33333",
                    Some("bbb22222"),
                    "kid2.png",
                    "2026-10-09T13:00:00",
                ),
                record(
                    "ddd44444",
                    Some("bbb22222"),
                    "kid1.png",
                    "2026-10-09T11:30:00",
                ),
            ],
        );
        let view = load_view(dir.path()).expect("view");
        assert_eq!(view.nodes, 4);
        let roots: Vec<&str> = view.roots.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(roots, vec!["bbb22222", "aaa11111"], "roots newest-first");
        let kids: Vec<&str> = view.roots[0]
            .children
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(kids, vec!["ddd44444", "ccc33333"], "children oldest-first");
    }

    #[test]
    fn load_view_marks_absent_files_missing_and_keeps_them() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("present.png"), b"x").expect("present file");
        write_store(
            dir.path(),
            &[
                record("aaa11111", None, "present.png", "2026-10-09T10:00:00"),
                record("bbb22222", None, "gone.png", "2026-10-09T11:00:00"),
            ],
        );
        let view = load_view(dir.path()).expect("view");
        assert_eq!(view.roots.len(), 2, "D-20: a missing node is never dropped");
        assert_eq!(view.missing, 1);
        // Roots are newest-first, so the absent file (11:00) sorts first.
        assert!(view.roots[0].missing, "the absent file is marked");
        assert!(
            !view.roots[1].missing,
            "the file that exists is not missing"
        );
    }

    #[test]
    fn load_view_reduces_a_filename_that_carries_a_parent_component() {
        let dir = TempDir::new().unwrap();
        // T-02-01: an injected separator must resolve inside output_dir only.
        let mut injected = record("aaa11111", None, "secrets.env", "2026-10-09T10:00:00");
        injected.filename = "../../secrets.env".to_string();
        std::fs::write(dir.path().join("secrets.env"), b"x").expect("file in dir");
        write_store(dir.path(), std::slice::from_ref(&injected));

        let view = load_view(dir.path()).expect("view");
        assert_eq!(view.roots.len(), 1);
        assert!(
            !view.roots[0].missing,
            "the reduced path resolves inside output_dir"
        );
        assert_eq!(
            node_path(dir.path(), &injected.filename),
            Some(dir.path().join("secrets.env")),
            "reduction keeps the final component only"
        );
    }

    #[test]
    fn load_view_treats_unknown_parent_as_a_root_and_survives_a_cycle() {
        let dir = TempDir::new().unwrap();
        write_store(
            dir.path(),
            &[
                record("aaa11111", Some("nope"), "a.png", "2026-10-09T10:00:00"),
                record("bbb22222", Some("ccc33333"), "b.png", "2026-10-09T11:00:00"),
                record("ccc33333", Some("bbb22222"), "c.png", "2026-10-09T12:00:00"),
            ],
        );
        let view = load_view(dir.path()).expect("a cycle cannot loop derivation");
        assert_eq!(view.nodes, 3, "every record stays visible");
        assert_eq!(
            view.roots.len(),
            2,
            "unknown parent becomes a root; the cycle gets its own"
        );
        let cycle = view
            .roots
            .iter()
            .find(|r| r.id == "ccc33333")
            .expect("the cycle's newest record roots its subtree");
        assert_eq!(
            cycle.children.len(),
            1,
            "derivation terminates on a hand-edited cycle"
        );
        // The whole view holds each id exactly once.
        let mut seen = Vec::new();
        collect_ids(&view.roots, &mut seen);
        seen.sort();
        assert_eq!(seen, vec!["aaa11111", "bbb22222", "ccc33333"]);
    }

    fn collect_ids(nodes: &[HistoryNode], out: &mut Vec<String>) {
        for n in nodes {
            out.push(n.id.clone());
            collect_ids(&n.children, out);
        }
    }

    #[test]
    fn resolve_fork_source_resolves_a_unique_id_prefix() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.png"), b"x").expect("file");
        write_store(
            dir.path(),
            &[record("abc12345", None, "a.png", "2026-10-09T10:00:00")],
        );
        match resolve_fork_source(dir.path(), "abc").expect("resolves") {
            ForkSource::Node { id, filename } => {
                assert_eq!(id, "abc12345");
                assert_eq!(filename, "a.png");
            }
            _ => panic!("expected a node source"),
        }
    }

    #[test]
    fn resolve_fork_source_reports_an_ambiguous_prefix_with_candidates() {
        let dir = TempDir::new().unwrap();
        write_store(
            dir.path(),
            &[
                record("abc12345", None, "a.png", "2026-10-09T10:00:00"),
                record("abc99999", None, "b.png", "2026-10-09T11:00:00"),
            ],
        );
        let err = resolve_fork_source(dir.path(), "abc").expect_err("ambiguous");
        let text = err.to_string();
        assert!(
            text.contains("Ambiguous node id prefix 'abc' — candidates: abc12345, abc99999"),
            "contract sentence: {text}"
        );
    }

    #[test]
    fn resolve_fork_source_resolves_an_exact_filename() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.png"), b"x").expect("file");
        write_store(
            dir.path(),
            &[record("abc12345", None, "a.png", "2026-10-09T10:00:00")],
        );
        match resolve_fork_source(dir.path(), "a.png").expect("resolves") {
            ForkSource::Node { id, filename } => {
                assert_eq!(id, "abc12345");
                assert_eq!(filename, "a.png");
            }
            _ => panic!("expected a node source"),
        }
    }

    #[test]
    fn resolve_fork_source_adopts_an_untracked_file_as_a_root() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("loose.png"), b"x").expect("file");
        match resolve_fork_source(dir.path(), "loose.png").expect("adopted, never an error") {
            ForkSource::File { path, recorded } => {
                assert_eq!(path, dir.path().join("loose.png"));
                assert!(recorded, "D-12 auto-adoption");
            }
            _ => panic!("expected a file source"),
        }
        let records = read_records(dir.path()).expect("sidecar");
        assert_eq!(records.len(), 1);
        assert!(records[0].parent.is_none(), "adopted as a root");
        assert_eq!(records[0].filename, "loose.png");
        // Lazily adopted, so no lineage was recovered.
        assert!(records[0].prompt.is_empty());
    }

    #[test]
    fn resolve_fork_source_leaves_an_out_of_dir_file_untracked() {
        let dir = TempDir::new().unwrap();
        let outside = dir.path().parent().expect("parent").join("outside.png");
        std::fs::write(&outside, b"x").expect("file");
        let source = resolve_fork_source(dir.path(), outside.to_str().expect("path"))
            .expect("an out-of-dir source is not an error");
        match source {
            ForkSource::File { path, recorded } => {
                assert_eq!(path, outside);
                assert!(!recorded, "D-22: out-of-dir stays untracked");
            }
            _ => panic!("expected a file source"),
        }
        assert!(
            read_records(dir.path()).expect("empty store").is_empty(),
            "nothing was written"
        );
        let _ = std::fs::remove_file(&outside);
    }

    #[test]
    fn resolve_fork_source_reports_unknown_names() {
        let dir = TempDir::new().unwrap();
        let err = resolve_fork_source(dir.path(), "nope.png").expect_err("unknown");
        let text = err.to_string();
        assert!(
            text.contains("No history node or image named 'nope.png' in"),
            "contract sentence: {text}"
        );
        assert!(
            text.contains("run 'imagai history' to see what is available."),
            "contract sentence: {text}"
        );
    }

    #[test]
    fn resolve_fork_source_refuses_a_node_whose_file_is_gone() {
        let dir = TempDir::new().unwrap();
        write_store(
            dir.path(),
            &[record("abc12345", None, "gone.png", "2026-10-09T10:00:00")],
        );
        let err = resolve_fork_source(dir.path(), "abc12345").expect_err("file deleted");
        let text = err.to_string();
        assert!(
            text.contains("History node abc12345 points at 'gone.png', which is no longer in"),
            "contract sentence: {text}"
        );
    }

    #[test]
    fn record_result_appends_and_links_the_parent() {
        let dir = TempDir::new().unwrap();
        let first = dir.path().join("first.png");
        let id = record_result(dir.path(), &first, "a cat", "dall-e-3", None).expect("recorded");
        assert_eq!(id.len(), 8);

        let child = dir.path().join("first-edit.png");
        let child_id = record_result(
            dir.path(),
            &child,
            "make it sunset",
            "gpt-image-1",
            Some(first.to_str().expect("utf-8 path")),
        )
        .expect("recorded");
        let records = read_records(dir.path()).expect("sidecar");
        assert_eq!(records.len(), 2);
        assert!(records[0].parent.is_none(), "a generation is a root");
        assert_eq!(records[1].id, child_id);
        assert_eq!(records[1].parent.as_deref(), Some(id.as_str()));
        assert_eq!(records[1].filename, "first-edit.png");
    }

    #[test]
    fn record_result_ignores_results_outside_the_directory() {
        let dir = TempDir::new().unwrap();
        let escaped = dir.path().parent().expect("parent").join("escaped.png");
        let id = record_result(dir.path(), &escaped, "x", "m", None).expect("no error");
        assert!(id.is_empty(), "D-22: nothing recorded");
        assert!(read_records(dir.path())
            .expect("no sidecar is fine")
            .is_empty());
    }

    #[test]
    fn a_fresh_id_never_repeats_an_existing_one() {
        let taken: Vec<String> = vec![
            record("abc12345", None, "a.png", "2026-10-09T10:00:00").id,
            record("def67890", None, "b.png", "2026-10-09T11:00:00").id,
        ];
        let mut all = taken;
        for _ in 0..16 {
            let candidate = new_node_id(&all);
            assert!(!all.contains(&candidate), "no collision with existing ids");
            all.push(candidate);
        }
    }
}
