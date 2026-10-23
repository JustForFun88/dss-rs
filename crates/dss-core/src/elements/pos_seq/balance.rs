use crate::exec::Dss;
use num_complex::Complex64;

/// The relative bound of a balance pin: headroom over the 1.6e-16 to
/// 2e-11 the pins measure (2026-10-04). The upper end is the unbalanced
/// elements, which unbalance the source bus a little.
pub(crate) const BALANCE_REL: f64 = 1e-9;

/// A circuit whose source bus `b` holds 12.47 kV line to line at 60 Hz
/// behind a 1e12 MVA short-circuit level, so an element on `b` sees one
/// balanced voltage before and after the reduction. An unbalanced element
/// still unbalances the bus by about `|I| |Zs| / |V|`, which shows as up to
/// 2e-11 in the balance pins (1e-9 at 1e10 MVA).
pub(crate) fn stiff(name: &str) -> Dss {
    stiff_at(name, 12.47, 60.0)
}

/// [`stiff`] at `kv` line to line after `set defaultbasefrequency={hz}`.
pub(crate) fn stiff_at(name: &str, kv: f64, hz: f64) -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("set defaultbasefrequency={hz}"));
    dss.command(&format!(
        "new circuit.{name} basekv={kv} pu=1.0 phases=3 bus1=b mvasc3=1e12 mvasc1=1e12"
    ));
    dss
}

/// [`stiff`] with a line from `b` to a 100 kW load on bus `c`, and a monitor
/// and an energy meter on the line: the readers a report walks.
pub(crate) fn stiff_metered(name: &str) -> Dss {
    let mut dss = stiff(name);
    dss.command("new line.feed bus1=b bus2=c phases=3 r1=0.1 x1=0.3 length=1");
    dss.command("new load.ld bus1=c phases=3 kv=12.47 kw=100");
    dss.command("new monitor.mon element=line.feed terminal=1");
    dss.command("new energymeter.em element=line.feed terminal=1");
    dss
}

/// The solve error (code 482) of a Y build an element refuses.
pub(crate) const REFUSED_SOLVE: &str =
    "Error Encountered in Solve: an element has no defined primitive admittance";

/// After a refused solve, `summary`, `show currents`, `export summary` and
/// `sample` each finish without an error of their own, the summary reads
/// the circuit as not solved, and `export y` and `show y` answer that no Y
/// matrix is built. The next solve builds the Y matrix again and refuses
/// again: it logs the element's `message` once more beside
/// [`REFUSED_SOLVE`].
pub(crate) fn reports_after_refusal(dss: &mut Dss, message: &str, what: &str) {
    let dir = std::env::temp_dir().join(format!("dss_refused_{}_{what}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dss.command(&format!("set datapath=\"{}\"", dir.display()));
    for cmd in ["summary", "show currents", "export summary", "sample"] {
        let n = dss.error_texts().len();
        dss.command(cmd);
        let errors = dss.error_texts();
        assert_eq!(
            errors.len(),
            n,
            "{what}: `{cmd}` after the refusal: {:?}",
            &errors[n..]
        );
        if cmd == "summary" {
            assert!(
                dss.result().contains("Status = NOT Solved"),
                "{what}: {}",
                dss.result()
            );
        }
    }
    for cmd in ["export y", "show y"] {
        let n = dss.error_texts().len();
        dss.command(cmd);
        let errors = dss.error_texts();
        assert!(
            errors[n..]
                .iter()
                .any(|t| t.contains("Y Matrix not Built.")),
            "{what}: `{cmd}` after the refusal: {:?}",
            &errors[n..]
        );
    }
    assert!(!dss.circuit().unwrap().is_solved, "{what}: solved");

    let n = dss.errors().len();
    dss.command("solve");
    let errors = &dss.errors()[n..];
    assert!(
        errors.iter().any(|d| d.message.contains(message)),
        "{what}: the second solve does not name the element: {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|d| d.code == Some(482) && d.message == REFUSED_SOLVE),
        "{what}: the second solve did not fail with 482: {errors:?}"
    );
    assert!(
        !dss.circuit().unwrap().is_solved,
        "{what}: solved the second time"
    );
    // Drop the report files, keep the data path valid for the caller.
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
}

/// Set the 12.47 kV base and solve, asserting a clean, solved circuit.
pub(crate) fn solve_clean(dss: &mut Dss, when: &str) {
    solve_clean_at(dss, &[12.47], when);
}

/// Set the voltage bases `bases` (kV line to line) and solve, asserting a
/// clean, solved circuit.
pub(crate) fn solve_clean_at(dss: &mut Dss, bases: &[f64], when: &str) {
    let list: Vec<String> = bases.iter().map(f64::to_string).collect();
    dss.command(&format!("set voltagebases=[{}]", list.join(", ")));
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{when}: {:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved, "{when}: not solved");
}

/// `|S_red - S_full| / |S_full|` of the [`source_power`]: `deck` compiled line
/// by line in a fresh [`Dss`] and solved on `bases` gives `S_full`, the
/// commands of `reduce` in order and a solve give `S_red`. A
/// positive-sequence circuit reports three times the one phase, so the two
/// are one quantity. Every command must leave no error.
pub(crate) fn ds_over_s(deck: &str, reduce: &[&str], bases: &[f64]) -> f64 {
    let mut dss = Dss::new();
    for line in deck.lines().map(str::trim).filter(|l| !l.is_empty()) {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}`: {:?}", dss.errors());
    }
    solve_clean_at(&mut dss, bases, "before the reduction");
    let full = source_power(&mut dss);
    for cmd in reduce {
        dss.command(cmd);
        assert!(dss.errors().is_empty(), "`{cmd}`: {:?}", dss.errors());
    }
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "after the reduction: {:?}",
        dss.errors()
    );
    assert!(
        dss.circuit().unwrap().is_solved,
        "after the reduction: not solved"
    );
    let red = source_power(&mut dss);
    (red - full).norm() / full.norm()
}

/// The circuit as `save circuit` writes it: every file of the save, sorted by
/// its path relative to the save directory, as `== <path>` and its text. The
/// directory (under the temp dir, named by `tag` and the process id) is
/// cleared before the save and removed after the read. The save must add no
/// error and write a `Master.dss` that defines the circuit, so a save that
/// writes nothing never reads as an unchanged circuit.
pub(crate) fn saved_circuit(dss: &mut Dss, tag: &str) -> String {
    let dir = std::env::temp_dir()
        .join("dss_posseq_saved")
        .join(format!("{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("save dir");
    let n = dss.errors().len();
    let path = dir.to_string_lossy().replace('\\', "/");
    dss.command(&format!("save circuit dir=\"{path}\""));
    assert_eq!(
        dss.errors().len(),
        n,
        "{tag}: the save added an error: {:?}",
        &dss.errors()[n..]
    );
    let mut files = Vec::new();
    let mut pending = vec![dir.clone()];
    while let Some(d) = pending.pop() {
        for entry in std::fs::read_dir(&d).expect("read the save dir") {
            let p = entry.expect("a save dir entry").path();
            if p.is_dir() {
                pending.push(p);
            } else {
                let rel = p.strip_prefix(&dir).expect("under the save dir");
                let rel = rel.to_string_lossy().replace('\\', "/");
                let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{rel}: {e}"));
                files.push((rel, text));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    files.sort();
    let (_, master) = files
        .iter()
        .find(|(rel, _)| rel == "Master.dss")
        .unwrap_or_else(|| {
            let names: Vec<&str> = files.iter().map(|(rel, _)| rel.as_str()).collect();
            panic!("{tag}: the save wrote no Master.dss: {names:?}")
        });
    assert!(
        master.lines().any(|l| l
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("new circuit.")),
        "{tag}: Master.dss defines no circuit:\n{master}"
    );
    files
        .into_iter()
        .map(|(rel, text)| format!("== {rel}\n{text}"))
        .collect()
}

/// Solve after a refused command: the solve adds no error to the ones
/// already there and solves, and the [`source_power`] is returned.
pub(crate) fn solve_after_refusal(dss: &mut Dss, when: &str) -> Complex64 {
    let n = dss.errors().len();
    dss.command("solve");
    assert_eq!(
        dss.errors().len(),
        n,
        "{when}: the solve added an error: {:?}",
        &dss.errors()[n..]
    );
    assert!(dss.circuit().unwrap().is_solved, "{when}: not solved");
    source_power(dss)
}

/// The complex power (kW + j kvar) into `vsource.source`, read as minus the
/// power every other element draws, which conservation of complex power makes
/// the same number. The source's own terminal current is the difference of two
/// terms near `Y V`, which a 1e12 MVA source leaves about 1e-2 A off: on the
/// reduction of the `ds_over_s` pin's circuit that reading moves by 2.1e-4 and
/// this one by 1.2e-13 (2026-10-17).
pub(crate) fn source_power(dss: &mut Dss) -> Complex64 {
    let snap = dss.snapshot_elements();
    let is_source = |name: &str| name.eq_ignore_ascii_case("vsource.source");
    assert!(
        snap.iter().any(|e| is_source(&e.name)),
        "vsource.source not in the snapshot"
    );
    -snap
        .iter()
        .filter(|e| !is_source(&e.name))
        .flat_map(|e| e.powers.iter())
        .sum::<Complex64>()
}

/// The power (kW + j kvar) of `full` at its first terminal, the sum over
/// its conductors. A positive-sequence circuit reports three times the
/// one phase, so the reading before and after a reduction is one quantity.
pub(crate) fn power(dss: &mut Dss, full: &str) -> Complex64 {
    let snap = dss.snapshot_elements();
    let e = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(full))
        .unwrap_or_else(|| panic!("{full} not in the snapshot"));
    e.powers[..e.n_conds].iter().sum()
}

/// The loss (W + j var) of `full`.
pub(crate) fn loss(dss: &mut Dss, full: &str) -> Complex64 {
    let snap = dss.snapshot_elements();
    let e = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(full))
        .unwrap_or_else(|| panic!("{full} not in the snapshot"));
    Complex64::new(e.loss_w.0, e.loss_w.1)
}

/// Solve, read the powers of `names`, `makeposseq`, solve again and read
/// them once more: `(before, after)` per element.
pub(crate) fn reduce(dss: &mut Dss, names: &[&str]) -> Vec<(Complex64, Complex64)> {
    solve_clean(dss, "before makeposseq");
    let pre: Vec<Complex64> = names.iter().map(|n| power(dss, n)).collect();
    dss.command("makeposseq");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "after makeposseq: {:?}",
        dss.errors()
    );
    assert!(
        dss.circuit().unwrap().is_solved,
        "after makeposseq: not solved"
    );
    let post: Vec<Complex64> = names.iter().map(|n| power(dss, n)).collect();
    pre.into_iter().zip(post).collect()
}

/// `after` equals `before` within [`BALANCE_REL`].
pub(crate) fn assert_kept(before: Complex64, after: Complex64, what: &str) {
    assert!(
        after.re.is_finite() && after.im.is_finite(),
        "{what}: {after} is not finite"
    );
    assert!(
        (after - before).norm() <= BALANCE_REL * before.norm(),
        "{what}: {before} before makeposseq, {after} after (rel {:.3e})",
        (after - before).norm() / before.norm()
    );
}

/// The solved magnitude (V) of node 1 of bus `bus`.
pub(crate) fn v1(dss: &Dss, bus: &str) -> f64 {
    dss.bus_voltages(bus)
        .unwrap_or_else(|| panic!("bus {bus}"))
        .vmag_angle[0]
        .0
}

/// The property `obj.prop` as one number (`[ 12]` reads 12).
pub(crate) fn query_f64(dss: &mut Dss, what: &str) -> f64 {
    dss.command(&format!("? {what}"));
    let r = dss.result().to_string();
    r.trim_matches(|c: char| c == '[' || c == ']' || c.is_whitespace())
        .parse()
        .unwrap_or_else(|e| panic!("{what} = {r:?}: {e}"))
}

/// `a` within `rel` of `b`, relative to `|b|`.
pub(crate) fn assert_rel(a: f64, b: f64, rel: f64, what: &str) {
    assert!(
        (a - b).abs() <= rel * b.abs(),
        "{what}: {a} vs {b} (rel {:.3e})",
        (a - b).abs() / b.abs()
    );
}

/// `(1/3)·v1ᴴ M v1` of the `n`-conductor matrix `m` (row-major) on phases
/// `0..n`, with `v1 = [1, a², a]` and `a = e^{j2π/3}`: the balanced
/// positive-sequence self term, computed from its definition.
pub(crate) fn seq11(m: &[Complex64], n: usize) -> Complex64 {
    let a = Complex64::from_polar(1.0, 2.0 * std::f64::consts::PI / 3.0);
    let mut s = Complex64::ZERO;
    for i in 0..n {
        for j in 0..n {
            s += m[i * n + j] * a.powi(i as i32 - j as i32);
        }
    }
    s / 3.0
}

/// The inverse of the `n × n` complex matrix `m` (row-major), Gauss-Jordan
/// with partial pivoting.
pub(crate) fn inverse(m: &[Complex64], n: usize) -> Vec<Complex64> {
    let mut a = m.to_vec();
    let mut inv: Vec<Complex64> = (0..n * n)
        .map(|k| {
            if k / n == k % n {
                Complex64::ONE
            } else {
                Complex64::ZERO
            }
        })
        .collect();
    for col in 0..n {
        let piv = (col..n)
            .max_by(|&p, &q| a[p * n + col].norm().total_cmp(&a[q * n + col].norm()))
            .unwrap();
        for k in 0..n {
            a.swap(col * n + k, piv * n + k);
            inv.swap(col * n + k, piv * n + k);
        }
        let d = a[col * n + col];
        for k in 0..n {
            a[col * n + k] /= d;
            inv[col * n + k] /= d;
        }
        for row in 0..n {
            if row != col {
                let f = a[row * n + col];
                for k in 0..n {
                    let (ack, ick) = (a[col * n + k], inv[col * n + k]);
                    a[row * n + k] -= f * ack;
                    inv[row * n + k] -= f * ick;
                }
            }
        }
    }
    inv
}

/// The matrix property `key` of `full` as the JSON export writes it, from
/// its key to the end of its rows (lower case).
pub(crate) fn json_matrix(dss: &Dss, full: &str, key: &str) -> String {
    let json = dss
        .obj_to_json(full, crate::report::export::json::JsonOpts::NONE)
        .unwrap_or_else(|| panic!("{full}: no JSON"))
        .to_ascii_lowercase();
    let at = json
        .find(&format!("\"{key}\":"))
        .unwrap_or_else(|| panic!("{key} in {json}"));
    let end = json[at..]
        .find("]]")
        .unwrap_or_else(|| panic!("{key} rows in {json}"));
    json[at..at + end + 2].to_string()
}

/// The row-major complex matrix `r + j x`.
pub(crate) fn complex(r: &[f64], x: &[f64]) -> Vec<Complex64> {
    r.iter()
        .zip(x)
        .map(|(&r, &x)| Complex64::new(r, x))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        BALANCE_REL, assert_kept, assert_rel, ds_over_s, power, query_f64, saved_circuit,
        solve_after_refusal, solve_clean_at, source_power, stiff_at, v1,
    };
    use crate::exec::Dss;

    /// A fresh [`Dss`] with `deck` compiled line by line.
    fn compiled(deck: &str) -> Dss {
        let mut dss = Dss::new();
        for line in deck.lines() {
            dss.command(line);
        }
        dss
    }

    /// A balanced 12.47 kV circuit: a line, a constant-impedance load and a
    /// capacitor on its far bus.
    const DSOS: &str = "clear
set defaultbasefrequency=60
new circuit.dsos basekv=12.47 pu=1.0 phases=3 bus1=b mvasc3=1e12 mvasc1=1e12
new line.l bus1=b bus2=c phases=3 r1=0.2 x1=0.5 length=1 units=km
new load.ld bus1=c phases=3 kv=12.47 kw=400 pf=0.95 model=2
new capacitor.c bus1=c phases=3 kvar=300 kv=12.47";

    /// A balanced circuit reduces exactly, and a twin that writes the
    /// unreduced load power back after the reduction draws about three times
    /// the load from the source (dS/S 1.94 on 2026-10-17).
    #[test]
    fn ds_over_s_holds_a_reduction_and_sees_a_wrong_one() {
        let exact = ds_over_s(DSOS, &["makeposseq"], &[12.47]);
        assert!(exact <= BALANCE_REL, "the reduction: dS/S = {exact:e}");
        let twin = ds_over_s(DSOS, &["makeposseq", "edit load.ld kw=400"], &[12.47]);
        assert!(twin > 0.5, "the twin: dS/S = {twin}");
    }

    /// Two saves with nothing between read the same text, and a solve of the
    /// unchanged circuit returns the source power read before it. After an
    /// edit with no solve of its own, the helper solves the edited circuit:
    /// it returns the source power of the deck compiled with the edit.
    #[test]
    fn saved_circuit_and_solve_after_refusal_read_an_unchanged_circuit() {
        let mut dss = compiled(DSOS);
        solve_clean_at(&mut dss, &[12.47], "dsos");
        dss.command("solve");
        let before = source_power(&mut dss);
        let first = saved_circuit(&mut dss, "dsos");
        let second = saved_circuit(&mut dss, "dsos");
        assert!(
            first.contains("== Master.dss\n") && first.to_ascii_lowercase().contains("load.ld"),
            "{first}"
        );
        assert_eq!(first, second, "two saves with nothing between");
        let after = solve_after_refusal(&mut dss, "dsos");
        assert_kept(before, after, "dsos");

        dss.command("edit load.ld kw=200");
        let edited = solve_after_refusal(&mut dss, "dsos at 200 kW");
        let mut half = compiled(&DSOS.replace("kw=400", "kw=200"));
        solve_clean_at(&mut half, &[12.47], "dsos compiled at 200 kW");
        let want = source_power(&mut half);
        assert!(
            (want - before).norm() > 0.3 * before.norm(),
            "the edit moves the source power: {before} and {want}"
        );
        assert_kept(want, edited, "dsos at 200 kW");
    }

    /// [`source_power`] is the power into the source's own terminal, which a
    /// 100 MVA source reads with no cancellation, before and after the
    /// reduction.
    #[test]
    fn source_power_is_the_power_into_the_source_terminal() {
        let weak = DSOS.replace("mvasc3=1e12 mvasc1=1e12", "mvasc3=100 mvasc1=100");
        let mut dss = compiled(&weak);
        for when in ["before makeposseq", "after makeposseq"] {
            if when == "after makeposseq" {
                dss.command("makeposseq");
            }
            solve_clean_at(&mut dss, &[12.47], when);
            let terminal = power(&mut dss, "vsource.source");
            assert!(terminal.norm() > 100.0, "{when}: {terminal}");
            assert_kept(terminal, source_power(&mut dss), when);
        }
        assert!(dss.circuit().unwrap().positive_sequence);
    }

    /// [`stiff_at`] puts its source at `kv` line to line and `hz`: the
    /// unloaded bus `b` holds the line-to-neutral voltage of `kv`, and the
    /// source reads `hz` as its base frequency.
    #[test]
    fn stiff_at_sets_the_voltage_and_the_frequency() {
        let mut dss = stiff_at("sa", 24.94, 50.0);
        solve_clean_at(&mut dss, &[24.94], "sa");
        let v_ln = 24940.0 / 3.0_f64.sqrt();
        assert_rel(v1(&dss, "b"), v_ln, 1e-9, "the voltage of b");
        let hz = query_f64(&mut dss, "vsource.source.basefreq");
        assert_eq!(hz, 50.0, "the base frequency of the source");
    }
}
