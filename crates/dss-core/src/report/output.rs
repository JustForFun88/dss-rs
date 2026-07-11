//! Output-path machinery for Phase-8 reports (Pascal `DoExportCmd`'s filename
//! resolution + `DSS.OutputDirectory` / `DSS.CircuitName_`).
//!
//! Pascal builds the default report path as
//! `DSS.OutputDirectory + DSS.CircuitName_ + <default name>`, where
//! `CircuitName_ = <CaseName>_` (`Circuit.pas` `Set_CaseName`) and `CaseName`
//! defaults to the circuit name. An explicit trailing filename argument to
//! `Export <x> <file>` is used as given — but a *relative* one opens relative
//! to the process cwd, which in Pascal tracks `CurrentDSSDir` (so after a
//! `Compile` it means the deck's directory; oracle-verified). Rust models that
//! cwd virtually in `current_dir`, hence the join below.

use std::path::{Path, PathBuf};

/// Resolve the report output path (Pascal `DoExportCmd`): an explicit filename
/// resolves against `current_dir` (Pascal opens it relative to the process cwd
/// = `CurrentDSSDir`; an absolute path wins the join verbatim); otherwise
/// `<output_dir>/<circuit_name_><default_name>`.
pub fn export_path(
    output_dir: &Path,
    current_dir: &Path,
    circuit_name_: &str,
    explicit: &str,
    default_name: &str,
) -> PathBuf {
    if explicit.is_empty() {
        output_dir.join(format!("{circuit_name_}{default_name}"))
    } else {
        current_dir.join(explicit)
    }
}
