//! `Export Counts` — Pascal `ExportResults.pas:2965` `ExportCounts`: a text dump
//! of every DSS class and its instance count, in class-registration order
//! (Pascal walks `DSS.DSSClassList`). Not CSV despite the `.csv` default name;
//! the format header is literal.

/// Build the `Export Counts` text. `classes` is `(class_name, instance_count)`
/// in registration order. Mirrors Pascal:
/// ```text
/// Format: DSS Class Name = Instance Count
///
/// <Name> = <Count>
/// ...
/// ```
/// (each line `FSWriteln`, i.e. one trailing newline; Pascal's `Format('%s = %d')`).
pub fn export_counts(classes: &[(String, usize)]) -> String {
    let mut out = String::from("Format: DSS Class Name = Instance Count\n\n");
    for (name, count) in classes {
        out.push_str(&format!("{name} = {count}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_then_one_line_per_class() {
        let s = export_counts(&[("Line".into(), 2), ("Load".into(), 1)]);
        assert_eq!(
            s,
            "Format: DSS Class Name = Instance Count\n\nLine = 2\nLoad = 1\n"
        );
    }
}
