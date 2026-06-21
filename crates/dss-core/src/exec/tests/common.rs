use crate::exec::*;

pub(super) fn query(dss: &mut Dss, what: &str) -> String {
    dss.command(&format!("? {what}"));
    dss.result().to_string()
}

/// `Edit`/`~`/`?` are circuit-gated in `ProcessCommand` (error 301), so
/// even DSS_OBJECT tests need a circuit.
pub(super) fn dss_with_circuit() -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.testckt");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// Parse the single number a `?` scalar query returns.
pub(super) fn query_f64(dss: &mut Dss, what: &str) -> f64 {
    query(dss, what).parse().expect("numeric query result")
}
