//! The git-level rules in temporary repositories the tests create, plus the
//! one check over the real repository that holds in any checkout and clone
//! depth: HEAD against itself is `None`. No test reads the real working tree
//! or a commit other than HEAD.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use gate_kind::{ALWAYS_CODE, GateKind, classify_diff, classify_diff_report};

/// The identity and EOL settings of every git command a test issues.
const GIT_CONFIG: [&str; 6] = [
    "-c",
    "user.name=gk",
    "-c",
    "user.email=gk@invalid",
    "-c",
    "core.autocrlf=false",
];

/// A throwaway repository under the system temp directory, removed on drop.
struct TempRepo {
    dir: PathBuf,
}

impl TempRepo {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir =
            std::env::temp_dir().join(format!("gate-kind-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the temporary repository");
        let repo = TempRepo { dir };
        repo.git(&["-c", "init.defaultBranch=main", "init", "-q"]);
        // The library's own git calls read the repository's config, so the
        // EOL setting is written there as well.
        repo.git(&["config", "core.autocrlf", "false"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo
    }

    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.dir)
            .args(GIT_CONFIG)
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.dir.join(rel);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("the parent directory");
        std::fs::write(path, text).expect("the file is written");
    }

    /// Stages everything and commits; returns the commit id.
    fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    fn kind(&self, base: &str, head: Option<&str>) -> GateKind {
        classify_diff(&self.dir, base, head)
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

const LIB: &str = "pub fn f() -> u32 {\n    // the answer\n    42\n}\n";
const LIB_COMMENT: &str = "pub fn f() -> u32 {\n    // the answer, reworded\n    42\n}\n";
const LIB_CODE: &str = "pub fn f() -> u32 {\n    // the answer\n    43\n}\n";

/// A repository with `src/lib.rs` and `README.md` committed; returns it and
/// the base commit.
fn seeded(tag: &str) -> (TempRepo, String) {
    let repo = TempRepo::new(tag);
    repo.write("src/lib.rs", LIB);
    repo.write("README.md", "# readme\n");
    let base = repo.commit("base");
    (repo, base)
}

#[test]
fn a_committed_comment_only_range_is_comments_and_a_code_edit_is_code() {
    let (repo, base) = seeded("range");
    repo.write("src/lib.rs", LIB_COMMENT);
    let comment = repo.commit("comment");
    assert_eq!(repo.kind(&base, Some(&comment)), GateKind::Comments);
    repo.write("src/lib.rs", LIB_CODE);
    let code = repo.commit("code");
    assert_eq!(repo.kind(&comment, Some(&code)), GateKind::Code);
    assert_eq!(repo.kind(&base, Some(&code)), GateKind::Code);
}

#[test]
fn the_working_tree_form_grades_uncommitted_and_staged_edits() {
    let (repo, base) = seeded("worktree");
    assert_eq!(repo.kind(&base, None), GateKind::None, "a clean tree");
    repo.write("src/lib.rs", LIB_COMMENT);
    assert_eq!(repo.kind(&base, None), GateKind::Comments, "unstaged");
    assert_eq!(
        repo.kind("HEAD", None),
        GateKind::Comments,
        "a symbolic base"
    );
    repo.git(&["add", "src/lib.rs"]);
    assert_eq!(repo.kind(&base, None), GateKind::Comments, "staged");
    repo.write("src/lib.rs", LIB_CODE);
    assert_eq!(repo.kind(&base, None), GateKind::Code, "a code edit");
    // The committed range still sees nothing: the edits are not committed.
    assert_eq!(repo.kind(&base, Some("HEAD")), GateKind::None);
}

#[test]
fn an_untracked_rs_beside_a_comment_only_edit_is_code() {
    let (repo, base) = seeded("untracked");
    repo.write("src/lib.rs", LIB_COMMENT);
    assert_eq!(repo.kind(&base, None), GateKind::Comments);
    repo.write("src/new.rs", "pub fn g() {}\n");
    let report = classify_diff_report(&repo.dir, &base, None).expect("a report");
    assert_eq!(report.kind(), GateKind::Code, "{:?}", report.files);
    assert!(
        report
            .files
            .iter()
            .any(|f| f.path == "src/new.rs" && f.kind == GateKind::Code),
        "{:?}",
        report.files
    );
    // An untracked but ignored file is not listed.
    std::fs::remove_file(repo.dir.join("src/new.rs")).expect("removed");
    repo.write(".git/info/exclude", "scratch/\n");
    repo.write("scratch/probe.rs", "pub fn h() {}\n");
    assert_eq!(repo.kind(&base, None), GateKind::Comments);
}

#[test]
fn a_rename_from_rs_to_md_is_code() {
    let (repo, base) = seeded("rename");
    repo.git(&["mv", "src/lib.rs", "src/lib.md"]);
    let head = repo.commit("rename");
    let report = classify_diff_report(&repo.dir, &base, Some(&head)).expect("a report");
    assert_eq!(report.kind(), GateKind::Code, "{:?}", report.files);
    assert_eq!(
        report.files.len(),
        1,
        "one rename entry: {:?}",
        report.files
    );
    assert_eq!(report.files[0].path, "src/lib.rs -> src/lib.md");
    // The same rename of a document is a document change.
    let (repo, base) = seeded("rename-md");
    repo.git(&["mv", "README.md", "GUIDE.md"]);
    let head = repo.commit("rename");
    assert_eq!(repo.kind(&base, Some(&head)), GateKind::Docs);
}

#[test]
fn a_mode_change_of_an_rs_is_code() {
    let (repo, base) = seeded("mode");
    repo.git(&["update-index", "--chmod=+x", "src/lib.rs"]);
    repo.git(&["commit", "-q", "-m", "mode"]);
    let head = repo.git(&["rev-parse", "HEAD"]);
    assert_eq!(repo.kind(&base, Some(&head)), GateKind::Code);
}

#[test]
fn a_cargo_lock_diff_is_code() {
    let (repo, base) = seeded("lock");
    repo.write("Cargo.lock", "version = 4\n");
    let head = repo.commit("lock");
    assert_eq!(repo.kind(&base, Some(&head)), GateKind::Code);
}

#[test]
fn a_comment_only_edit_under_the_tool_is_code() {
    let repo = TempRepo::new("tool");
    repo.write("tools/gate-kind/src/lib.rs", LIB);
    repo.write("tools/gate-kind/README.md", "# a\n");
    let base = repo.commit("base");
    repo.write("tools/gate-kind/src/lib.rs", LIB_COMMENT);
    let head = repo.commit("comment");
    assert_eq!(repo.kind(&base, Some(&head)), GateKind::Code);
    repo.write("tools/gate-kind/README.md", "# b\n");
    let doc = repo.commit("doc");
    assert_eq!(
        repo.kind(&head, Some(&doc)),
        GateKind::Code,
        "a document under the tool"
    );
}

/// A `.rs` side that is not UTF-8 is `Code`, never decoded lossily (a Latin-1
/// byte gained by a comment would otherwise grade `Comments`).
#[test]
fn a_comment_gaining_a_non_utf8_byte_is_code() {
    let repo = TempRepo::new("latin1");
    let lib = repo.dir.join("src/lib.rs");
    std::fs::create_dir_all(repo.dir.join("src")).expect("the src directory");
    std::fs::write(&lib, b"// plain\npub fn f() {}\n").expect("the base is written");
    let base = repo.commit("base");
    std::fs::write(&lib, b"// caf\xe9 (latin-1)\npub fn f() {}\n").expect("the head is written");
    let head = repo.commit("latin1");
    assert_eq!(repo.kind(&base, Some(&head)), GateKind::Code);
    assert_eq!(
        repo.kind(&base, None),
        GateKind::Code,
        "the working-tree form"
    );
}

/// A side that is a symlink (mode 120000) is `Code`, a `.md` included. The
/// entry goes straight into the index, so no filesystem symlink is needed.
#[test]
fn a_document_turned_into_a_symlink_is_code() {
    let (repo, base) = seeded("symlink");
    repo.write("target.txt", "README.md\n");
    let sha = repo.git(&["hash-object", "-w", "target.txt"]);
    let entry = format!("120000,{sha},README.md");
    repo.git(&["update-index", "--cacheinfo", &entry]);
    repo.git(&["commit", "-q", "-m", "link"]);
    let head = repo.git(&["rev-parse", "HEAD"]);
    assert_eq!(repo.kind(&base, Some(&head)), GateKind::Code);
}

#[test]
fn a_comment_only_edit_on_the_always_code_list_is_code_and_off_it_comments() {
    let listed = ALWAYS_CODE[0];
    let neighbour = "crates/dss-core/src/exec/neighbour.rs";
    assert!(!ALWAYS_CODE.contains(&neighbour));
    let repo = TempRepo::new("always");
    repo.write(listed, LIB);
    repo.write(neighbour, LIB);
    let base = repo.commit("base");
    repo.write(neighbour, LIB_COMMENT);
    let off_list = repo.commit("neighbour");
    assert_eq!(repo.kind(&base, Some(&off_list)), GateKind::Comments);
    repo.write(listed, LIB_COMMENT);
    let on_list = repo.commit("listed");
    assert_eq!(repo.kind(&off_list, Some(&on_list)), GateKind::Code);
}

#[test]
fn a_document_plus_comments_is_comments_and_a_document_alone_docs() {
    let (repo, base) = seeded("mixed");
    repo.write("README.md", "# readme\n\nMore.\n");
    let docs = repo.commit("docs");
    assert_eq!(repo.kind(&base, Some(&docs)), GateKind::Docs);
    repo.write("src/lib.rs", LIB_COMMENT);
    let comments = repo.commit("comments");
    assert_eq!(repo.kind(&docs, Some(&comments)), GateKind::Comments);
    assert_eq!(repo.kind(&base, Some(&comments)), GateKind::Comments);
}

#[test]
fn an_added_document_is_docs_and_a_deleted_rs_code() {
    let (repo, base) = seeded("add-delete");
    repo.write("docs/new.md", "new\n");
    let added = repo.commit("added");
    assert_eq!(repo.kind(&base, Some(&added)), GateKind::Docs);
    repo.git(&["rm", "-q", "src/lib.rs"]);
    let deleted = repo.commit("deleted");
    assert_eq!(repo.kind(&added, Some(&deleted)), GateKind::Code);
}

#[test]
fn a_file_neither_md_nor_rs_is_code() {
    let (repo, base) = seeded("other");
    repo.write("decks/case.dss", "clear\n");
    let head = repo.commit("deck");
    assert_eq!(repo.kind(&base, Some(&head)), GateKind::Code);
}

#[test]
fn an_unknown_rev_is_code() {
    let (repo, base) = seeded("unknown");
    assert_eq!(repo.kind("no-such-rev", None), GateKind::Code);
    assert_eq!(repo.kind(&base, Some("no-such-rev")), GateKind::Code);
    let err = classify_diff_report(&repo.dir, "no-such-rev", None).expect_err("an error");
    assert!(err.contains("no-such-rev"), "{err}");
    // A rev that names a path is never read as the path.
    assert_eq!(repo.kind("README.md", None), GateKind::Code);
}

#[test]
fn a_repository_git_cannot_enter_is_code() {
    let dir = std::env::temp_dir().join(format!(
        "gate-kind-missing-{}/never-created",
        std::process::id()
    ));
    assert!(!dir.exists());
    assert_eq!(classify_diff(&dir, "HEAD", None), GateKind::Code);
    assert_eq!(classify_diff(&dir, "HEAD", Some("HEAD")), GateKind::Code);
}

/// True in any checkout and clone depth (CI clones at depth 1).
#[test]
fn the_real_repository_head_against_itself_is_none() {
    let root: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect();
    assert!(Path::new(&root).join("Cargo.toml").is_file());
    assert_eq!(classify_diff(&root, "HEAD", Some("HEAD")), GateKind::None);
}

/// The binary in `dir`: its stdout lines and whether it exited 0.
fn run_cli(dir: &Path, args: &[&str]) -> (Vec<String>, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_gate-kind"))
        .current_dir(dir)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("the binary runs");
    let lines = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    (lines, out.status.success())
}

/// One `<kind>\t<path>\t<reason>` line per file, `Comments` naming doc or
/// plain, then `GATE_KIND=<kind>`; exit 0.
#[test]
fn the_binary_prints_one_line_per_file_and_the_gate_kind() {
    let (repo, base) = seeded("cli");
    repo.write("src/lib.rs", LIB_COMMENT);
    repo.write("README.md", "# readme, longer\n");
    let head = repo.commit("comment and doc");
    let (lines, ok) = run_cli(&repo.dir, &["--base", &base, "--head", &head]);
    assert!(ok, "{lines:?}");
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(
        lines.contains(&"Docs\tREADME.md\ta document".to_string()),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("Comments(plain)\tsrc/lib.rs\t")),
        "{lines:?}"
    );
    assert_eq!(lines.last().map(String::as_str), Some("GATE_KIND=Comments"));

    // The working-tree form, `--base=<rev>` spelling, a doc comment.
    repo.write("src/lib.rs", &format!("/// f\n{LIB_COMMENT}"));
    let (lines, ok) = run_cli(&repo.dir, &[&format!("--base={head}")]);
    assert!(ok);
    assert!(
        lines[0].starts_with("Comments(doc)\tsrc/lib.rs\t"),
        "{lines:?}"
    );
    assert_eq!(lines.last().map(String::as_str), Some("GATE_KIND=Comments"));

    // Nothing changed: only the final line.
    let (lines, ok) = run_cli(&repo.dir, &["--base", &head, "--head", &head]);
    assert!(ok);
    assert_eq!(lines, ["GATE_KIND=None"]);
}

/// Every error prints its reason and `GATE_KIND=Code`, and exits 0.
#[test]
fn the_binary_grades_every_error_code_and_exits_zero() {
    let (repo, base) = seeded("cli-errors");
    for args in [
        vec!["--base", "no-such-rev"],
        vec!["--base", base.as_str(), "--head", "no-such-rev"],
        vec![],
        vec!["--head", base.as_str()],
        vec!["--base"],
        vec!["--base", base.as_str(), "--bogus"],
        vec!["--base", base.as_str(), "--base", base.as_str()],
    ] {
        let (lines, ok) = run_cli(&repo.dir, &args);
        assert!(ok, "{args:?}: {lines:?}");
        assert_eq!(lines.len(), 2, "{args:?}: {lines:?}");
        assert!(lines[0].starts_with("error: "), "{args:?}: {lines:?}");
        assert_eq!(lines[1], "GATE_KIND=Code", "{args:?}");
    }
}
