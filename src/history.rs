//! Iteration-lineage store: the `.imagai-history.json` sidecar.
//!
//! The sidecar lives inside the configured `output_dir` and records one entry
//! per image that lands there (D-08). Every surface — CLI, TUI and web —
//! converges on [`generate_image_core`], which calls [`record_result`] on each
//! successful save, so what is recorded cannot drift between frontends
//! (BRCH-04).
//!
//! The store is deliberately simple: read-modify-write of the whole sidecar per
//! save, no locking, no compaction (D-11). A failed write is never fatal to the
//! generation whose image is already on disk, but it is no longer invisible —
//! the write itself is atomic (temp file plus `rename`), so an interrupted
//! write leaves the previous sidecar intact rather than a truncated one, and
//! `generate_image_core` reports the failure to the operator.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// Filename of the lineage sidecar inside `output_dir` (D-08).
pub const HISTORY_FILENAME: &str = ".imagai-history.json";

/// The sibling temp file [`append_records`] writes before renaming it over the
/// sidecar. It is never served and never listed.
pub const HISTORY_TEMP_FILENAME: &str = ".imagai-history.json.tmp";

/// A tree deeper than this is not descended into by [`load_view`] or the TUI
/// flattener.
///
/// The sidecar is hand-editable by design, and a deep `parent` chain is
/// ordinary use rather than corruption — 900 sequential edits of one image is a
/// plausible history. `build_node` used to recurse once per level with no
/// bound, which aborts the process: ~900 levels kill a 2 MB tokio worker
/// (every web handler) and ~6 000 kill the 8 MB main thread (CLI, TUI). The
/// cap lives here, in the shared read path, so no surface can be killed.
///
/// Records below the cap are not dropped: the stranded pass re-roots them, so
/// every record stays visible (D-20) on a bounded stack.
pub const MAX_TREE_DEPTH: usize = 64;

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
    record_for_filename(&records, filename).map(|r| r.id.clone())
}

/// The record for `filename`, when one exists.
///
/// Filenames are not unique: a sidecar can hold several records with the same
/// name — a hand edit, or a regenerate after a delete. Resolution is
/// deterministic, and it is the **newest** match that wins: the last one in
/// sidecar (append) order, which is the node an edit of that file most
/// plausibly branched from. The first (oldest) match used to win, so lineage
/// was attached to a stale root while the newest branch went unrecorded
/// (CR-05).
fn record_for_filename<'a>(
    records: &'a [HistoryRecord],
    filename: &str,
) -> Option<&'a HistoryRecord> {
    records.iter().rev().find(|r| r.filename == filename)
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
///
/// Only `NotFound` means "no history yet". A non-UTF-8 sidecar — a hand edit, a
/// truncated write from another tool — fails `read_to_string` with
/// `InvalidData`, and reading it as empty would let the next read-modify-write
/// truncate and destroy the operator's history (CR-02).
pub fn read_records(dir: &Path) -> anyhow::Result<Vec<HistoryRecord>> {
    let path = history_path(dir);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            anyhow::bail!("Could not read history at {}: {e}", path.display());
        }
    };
    let records: Vec<HistoryRecord> = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("Could not read history at {}: {e}", path.display()))?;
    Ok(records)
}

/// The D-11 read-modify-write: read the whole sidecar, append, write it back.
/// No locking, no compaction — a locked decision.
///
/// The write goes to a temp file beside the sidecar and is then `rename`d over
/// it, which is atomic within one filesystem. `fs::write` on the sidecar itself
/// truncates before writing, so an interrupted write left a sidecar that failed
/// to parse — and once an unreadable store is an error (CR-02), every surface
/// then refused to start until an operator hand-repaired it (WR-01). A failed
/// write also never destroys the previous content.
pub fn append_records(dir: &Path, new: &[HistoryRecord]) -> anyhow::Result<()> {
    let mut records = read_records(dir)?;
    records.extend_from_slice(new);
    let json = serde_json::to_string_pretty(&records)?;
    let temp = dir.join(HISTORY_TEMP_FILENAME);
    std::fs::write(&temp, format!("{json}\n"))?;
    std::fs::rename(&temp, history_path(dir))?;
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
/// `parent` chain cannot loop. Traversal stops descending at
/// [`MAX_TREE_DEPTH`], so a deep chain in a hand-editable sidecar cannot abort
/// the process; the stranded pass re-roots what the cap skipped, so every
/// record stays visible. Children are ordered oldest-first and roots
/// newest-first, both on sidecar position — append order, and the only
/// deterministic tiebreak, because `created` has second resolution (WR-04).
pub fn load_view(dir: &Path) -> anyhow::Result<HistoryView> {
    let records = read_records(dir)?;
    let known: HashSet<&str> = records.iter().map(|r| r.id.as_str()).collect();

    let mut children_of: HashMap<&str, Vec<Indexed>> = HashMap::new();
    let mut roots: Vec<Indexed> = Vec::new();
    for (i, r) in records.iter().enumerate() {
        match r.parent.as_deref() {
            Some(p) if known.contains(p) && p != r.id.as_str() => {
                children_of.entry(p).or_default().push((i, r));
            }
            _ => roots.push((i, r)),
        }
    }
    // `created` is fixed-width ISO, so string ordering is chronological — but it
    // has second resolution, so two records written in the same second tie. The
    // sidecar position breaks the tie; relying on the random uuid made sibling
    // order arbitrary.
    roots.sort_by(|a, b| (&b.1.created, b.0).cmp(&(&a.1.created, a.0)));
    for list in children_of.values_mut() {
        list.sort_by(|a, b| (&a.1.created, a.0).cmp(&(&b.1.created, b.0)));
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
    build_roots(&roots, dir, &children_of, 0, &mut nodes, &mut seen);
    // A record stranded in a cyclic `parent` chain is reachable from no root;
    // listing it as its own root keeps every record visible (D-20) while the
    // visited set keeps derivation bounded (T-02-02). Records below the depth
    // cap arrive here too, so the cap drops nothing (CR-04).
    let mut stranded: Vec<Indexed> = records
        .iter()
        .enumerate()
        .filter(|(_, r)| !seen.contains(r.id.as_str()))
        .collect();
    stranded.sort_by(|a, b| (&b.1.created, b.0).cmp(&(&a.1.created, a.0)));
    build_roots(&stranded, dir, &children_of, 0, &mut nodes, &mut seen);

    Ok(HistoryView {
        output_dir: dir.to_string_lossy().to_string(),
        nodes: records.len(),
        missing: missing_count,
        roots: nodes,
    })
}

/// A record plus its position in the sidecar, which is append order.
type Indexed<'a> = (usize, &'a HistoryRecord);

fn build_roots(
    list: &[Indexed],
    dir: &Path,
    children_of: &HashMap<&str, Vec<Indexed>>,
    depth: usize,
    nodes: &mut Vec<HistoryNode>,
    seen: &mut HashSet<String>,
) {
    for (_, r) in list {
        if !seen.contains(r.id.as_str()) {
            nodes.push(build_node(r, dir, children_of, depth, seen));
        }
    }
}

fn build_node(
    record: &HistoryRecord,
    dir: &Path,
    children_of: &HashMap<&str, Vec<Indexed>>,
    depth: usize,
    seen: &mut HashSet<String>,
) -> HistoryNode {
    seen.insert(record.id.clone());
    // Children already reached elsewhere are skipped, so a cyclic `parent`
    // chain in a hand-edited sidecar terminates and emits nothing twice
    // (T-02-02).
    let child_records: Vec<Indexed> = children_of
        .get(record.id.as_str())
        .map(|list| {
            list.iter()
                .filter(|(_, c)| !seen.contains(c.id.as_str()))
                .copied()
                .collect()
        })
        .unwrap_or_default();
    // CR-04: the depth cap is what keeps this recursion bounded. Descending
    // stops here, and the stranded pass re-roots what was skipped.
    let children = if depth >= MAX_TREE_DEPTH {
        Vec::new()
    } else {
        child_records
            .into_iter()
            .map(|(_, c)| build_node(c, dir, children_of, depth + 1, seen))
            .collect()
    };
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
    // basename; a plain generation (no source, or no match) is a root. A
    // filename is not unique, so the newest record wins — see
    // `record_for_filename`.
    let parent = source
        .and_then(|s| Path::new(s).file_name())
        .and_then(|n| n.to_str())
        .and_then(|name| record_for_filename(&records, name))
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

/// The fixed 32-character prompt field: the first 29 characters plus `...`
/// when longer, otherwise padded with trailing spaces. A lazily adopted file
/// with no PNG lineage renders `(unknown prompt)` — never a blank column.
pub fn prompt_field(prompt: &str) -> String {
    if prompt.is_empty() {
        return format!("{:<32}", "(unknown prompt)");
    }
    if prompt.chars().count() > 32 {
        let clipped: String = prompt.chars().take(29).collect();
        format!("{clipped}...")
    } else {
        format!("{:<32}", prompt)
    }
}

/// `2026-10-09T11:40:00` -> `2026-10-09 11:40`.
pub fn timestamp_field(created: &str) -> String {
    match (created.get(..10), created.get(11..16)) {
        (Some(date), Some(time)) => format!("{date} {time}"),
        _ => created.to_string(),
    }
}

/// One row of the tree in the grammar every surface renders: three spaces of
/// indent per ancestor level, then the connector (`+- ` for a non-last child,
/// `` `- `` for the last, none at depth 0), then `✗ ` for a missing file, then
/// the id8, two spaces, the prompt field, two spaces, the filename, two spaces
/// and the timestamp. One implementation, three surfaces (BRCH-04).
pub fn history_row_text(node: &HistoryNode, depth: usize, is_last: bool) -> String {
    let connector = if depth == 0 {
        ""
    } else if is_last {
        "`- "
    } else {
        "+- "
    };
    let indent = "   ".repeat(depth);
    let marker = if node.missing { "✗ " } else { "" };
    format!(
        "{indent}{connector}{marker}{}  {}  {}  {}",
        node.id,
        prompt_field(&node.prompt),
        node.filename,
        timestamp_field(&node.created)
    )
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
/// 3. an existing file — a separator-carrying path used literally and left
///    untracked (D-22), or a bare filename resolved inside `dir` and
///    auto-adopted as a root node (D-12). A bare name is never resolved
///    against the process CWD (CR-01);
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

    // (2) exact filename. Filenames are not unique, so the newest record
    // matching the name wins — deterministic, and the branch an edit of that
    // file most plausibly came from (CR-05).
    if let Some(record) = record_for_filename(&records, from) {
        return node_source(dir, record);
    }

    // (3) an existing file. A value carrying a separator is the literal path
    // the operator named: it stays out of the store (D-22). A bare filename is
    // resolved *inside `dir` only* — never against the process working
    // directory. The web surface binds 0.0.0.0 with `allow_origin(Any)`, so a
    // CWD-relative resolution would hand a server-side file to whichever
    // browser asked for it (CR-01). It is also the only way the `recorded:
    // true` adoption branch is reachable when the CWD *is* `dir` (CR-03).
    let literal = Path::new(from);
    if from.contains('/') || from.contains('\\') {
        if literal.is_file() {
            return Ok(ForkSource::File {
                path: literal.to_path_buf(),
                recorded: false,
            });
        }
    } else {
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
    fn unreadable_sidecar_is_an_error_not_an_empty_store() {
        // CR-02: a non-UTF-8 sidecar used to read as `Ok(vec![])`, because every
        // `read_to_string` failure was mapped to an empty store. `read_to_string`
        // fails with `InvalidData`, not `NotFound`, so only the missing file is
        // the "no history yet" case.
        let dir = TempDir::new().unwrap();
        std::fs::write(history_path(dir.path()), b"\xff\xfe\x00{\"")
            .expect("write non-utf8 sidecar");
        let err = read_records(dir.path()).expect_err("unreadable is an error");
        let text = err.to_string();
        assert!(
            text.contains("Could not read history at"),
            "contract sentence: {text}"
        );
        assert!(text.contains(HISTORY_FILENAME), "names the path: {text}");
        assert!(load_view(dir.path()).is_err(), "never a silent empty tree");
    }

    #[test]
    fn an_unreadable_sidecar_is_not_overwritten_by_the_next_record() {
        // CR-02: the hazard the empty read created — the next read-modify-write
        // truncated the file and destroyed the operator's history.
        let dir = TempDir::new().unwrap();
        let broken = b"\xff\xfe\x00{\"";
        std::fs::write(history_path(dir.path()), broken).expect("write non-utf8 sidecar");
        let outcome = record_result(dir.path(), &dir.path().join("new.png"), "x", "m", None);
        assert!(
            outcome.is_err(),
            "recording must refuse rather than clobber an unreadable store"
        );
        let after = std::fs::read(history_path(dir.path())).expect("sidecar still there");
        assert_eq!(after, broken, "the operator's bytes are untouched");
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
    fn load_view_survives_a_deep_chain_on_a_small_stack() {
        // CR-04: `build_node` recursed once per level with no bound, so a deep
        // `parent` chain aborted the process. The tokio worker that serves
        // every web handler runs on a 2 MB stack and dies at roughly 900
        // levels; a 512 KiB one makes the pre-fix overflow unmistakable.
        let dir = TempDir::new().unwrap();
        let chain = 900;
        let mut store = Vec::with_capacity(chain);
        for i in 0..chain {
            store.push(HistoryRecord {
                id: format!("{i:08x}"),
                parent: (i > 0).then(|| format!("{:08x}", i - 1)),
                filename: format!("img{i}.png"),
                prompt: String::new(),
                model: "dall-e-3".to_string(),
                created: "2026-10-09T10:00:00".to_string(),
            });
        }
        write_store(dir.path(), &store);

        let path = dir.path().to_path_buf();
        let handle = std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(move || load_view(&path).expect("a deep chain must not abort"))
            .expect("spawn");
        let view = handle.join().expect("no stack overflow");

        assert_eq!(view.nodes, chain, "every record is counted");
        // D-20: the depth cap re-roots what it skipped, so nothing is dropped.
        let mut ids = Vec::new();
        collect_ids(&view.roots, &mut ids);
        assert_eq!(ids.len(), chain, "every record stays visible");
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
    fn a_duplicate_filename_resolves_to_the_newest_record() {
        // CR-05: both parent lookups took the first (oldest) match, so with two
        // records sharing a filename lineage was attached to a stale root while
        // the newest branch went unrecorded. Resolution is now the newest
        // record — deterministic, and the branch the edit plausibly came from.
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("cat.png"), b"x").expect("file");
        write_store(
            dir.path(),
            &[
                record("aaa11111", None, "cat.png", "2026-10-09T10:00:00"),
                record("bbb22222", None, "cat.png", "2026-10-09T12:00:00"),
            ],
        );

        assert_eq!(
            node_id_for(dir.path(), "cat.png").as_deref(),
            Some("bbb22222"),
            "the newest record wins"
        );
        match resolve_fork_source(dir.path(), "cat.png").expect("resolves") {
            ForkSource::Node { id, .. } => assert_eq!(id, "bbb22222"),
            _ => panic!("expected a node source"),
        }

        // And an edit whose source is that filename links to the same node.
        let child = dir.path().join("cat-edit.png");
        record_result(
            dir.path(),
            &child,
            "make it sunset",
            "gpt-image-1",
            Some(dir.path().join("cat.png").to_str().expect("utf-8")),
        )
        .expect("recorded");
        let records = read_records(dir.path()).expect("sidecar");
        assert_eq!(records.len(), 3);
        assert_eq!(
            records[2].parent.as_deref(),
            Some("bbb22222"),
            "CR-05: the child links to the newest record, not the stale one"
        );
    }

    #[test]
    fn append_records_is_atomic_and_leaves_no_temp_file() {
        // WR-01: the sidecar was written in place with `fs::write`, which
        // truncates before it writes — an interrupted write left a sidecar that
        // failed to parse. The write now goes to a sibling temp file and is
        // renamed over the sidecar.
        let dir = TempDir::new().unwrap();
        let first = record("aaa11111", None, "a.png", "2026-10-09T10:00:00");
        append_records(dir.path(), std::slice::from_ref(&first)).expect("first append");
        let second = record("bbb22222", Some("aaa11111"), "b.png", "2026-10-09T11:00:00");
        append_records(dir.path(), std::slice::from_ref(&second)).expect("second append");

        let records = read_records(dir.path()).expect("sidecar");
        assert_eq!(records.len(), 2, "the earlier record survives the append");
        assert!(
            !dir.path().join(HISTORY_TEMP_FILENAME).exists(),
            "no temp file is left behind"
        );
    }

    #[test]
    fn a_failed_sidecar_write_keeps_the_previous_content() {
        // WR-01, the property that matters: the live sidecar is never opened for
        // writing, so a write that fails cannot destroy the operator's history.
        let dir = TempDir::new().unwrap();
        let first = record("aaa11111", None, "a.png", "2026-10-09T10:00:00");
        append_records(dir.path(), std::slice::from_ref(&first)).expect("first append");
        let before = std::fs::read(history_path(dir.path())).expect("bytes before");

        // Occupying the temp path makes the write itself fail.
        std::fs::create_dir(dir.path().join(HISTORY_TEMP_FILENAME)).expect("occupy the temp path");
        let second = record("bbb22222", Some("aaa11111"), "b.png", "2026-10-09T11:00:00");
        assert!(
            append_records(dir.path(), std::slice::from_ref(&second)).is_err(),
            "the failed write is reported, not swallowed"
        );
        assert_eq!(
            std::fs::read(history_path(dir.path())).expect("bytes after"),
            before,
            "the live sidecar is untouched"
        );
    }

    #[test]
    fn load_view_orders_same_second_records_by_sidecar_position() {
        // WR-04: `created` has second resolution, so any two records written in
        // the same second tie and then fell through to the random uuid,
        // contradicting the documented children-oldest-first /
        // roots-newest-first order. Append position is the tiebreak.
        //
        // The ids are chosen so the uuid order is the *reverse* of append
        // order, which is what made the old behaviour a defect rather than a
        // coincidence.
        let dir = TempDir::new().unwrap();
        write_store(
            dir.path(),
            &[
                // Children: "zzz99999" is written first, "aaa11111" second.
                record("root0001", None, "root.png", "2026-10-09T10:00:00"),
                record(
                    "zzz99999",
                    Some("root0001"),
                    "first.png",
                    "2026-10-09T11:00:00",
                ),
                record(
                    "aaa11111",
                    Some("root0001"),
                    "second.png",
                    "2026-10-09T11:00:00",
                ),
                // Roots sharing the root's timestamp.
                record("aaa11112", None, "third.png", "2026-10-09T10:00:00"),
                record("ccc33333", None, "fourth.png", "2026-10-09T10:00:00"),
            ],
        );
        let view = load_view(dir.path()).expect("view");
        assert_eq!(view.nodes, 5);

        let kids: Vec<&str> = view
            .roots
            .iter()
            .find(|r| r.id == "root0001")
            .expect("the root")
            .children
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(
            kids,
            vec!["zzz99999", "aaa11111"],
            "same-second children are oldest-first by append order, not by uuid"
        );

        let roots: Vec<&str> = view.roots.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(
            roots,
            vec!["ccc33333", "aaa11112", "root0001"],
            "same-second roots are newest-append-first, not by uuid"
        );
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
