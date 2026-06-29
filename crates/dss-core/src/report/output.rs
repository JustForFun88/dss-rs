//! Output-path machinery for Phase-8 reports (Pascal `DoExportCmd`'s filename
//! resolution + `DSS.OutputDirectory` / `DSS.CircuitName_`).
//!
//! Pascal builds the default report path as
//! `DSS.OutputDirectory + DSS.CircuitName_ + <default name>`, where
//! `CircuitName_ = <CaseName>_` (`Circuit.pas` `Set_CaseName`) and `CaseName`
//! defaults to the circuit name. An explicit trailing filename argument to
//! `Export <x> <file>` overrides it verbatim (Pascal: "should be full path name
//! to work universally").

use std::path::{Path, PathBuf};

/// Resolve the report output path (Pascal `DoExportCmd`): an explicit filename
/// wins verbatim; otherwise `<output_dir>/<circuit_name_><default_name>`.
pub fn export_path(
    output_dir: &Path,
    circuit_name_: &str,
    explicit: &str,
    default_name: &str,
) -> PathBuf {
    if explicit.is_empty() {
        output_dir.join(format!("{circuit_name_}{default_name}"))
    } else {
        // Pascal uses the StrValue verbatim (a full path is expected).
        PathBuf::from(explicit)
    }
}
