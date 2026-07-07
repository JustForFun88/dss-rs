use crate::exec::*;

/// WP5.8 step 6: time-option round trips, all values transcribed from the
/// pinned oracle.
#[test]
fn time_options_round_trip_matches_oracle() {
    let query = |dss: &mut Dss, what: &str| -> String {
        dss.command(&format!("get {what}"));
        dss.result().to_string()
    };
    let mut dss = Dss::new();
    dss.command("new circuit.t2");
    dss.command("set stepsize=15m");
    assert_eq!(query(&mut dss, "stepsize"), "900");
    dss.command("set hour=5");
    assert_eq!(query(&mut dss, "hour"), "5");
    dss.command("set sec=120.5");
    assert_eq!(query(&mut dss, "sec"), "120.5");
    dss.command("set time=(2, 1800)");
    assert_eq!(query(&mut dss, "time"), "[ 2, 1800 ] !... 2.5 (hours)");
    assert_eq!(query(&mut dss, "hour"), "2");
    assert_eq!(query(&mut dss, "sec"), "1800");
    dss.command("set mode=daily");
    assert_eq!(query(&mut dss, "number"), "24");
    assert_eq!(query(&mut dss, "stepsize"), "3600");
    // DUTYCYCLE forces TIMEDRIVEN control mode and h = 1 s.
    dss.command("set mode=duty");
    assert_eq!(query(&mut dss, "controlmode"), "Time");
    assert_eq!(query(&mut dss, "stepsize"), "1");
    // Circuit defaults resolve to the built-in `loadshape.default`.
    assert_eq!(query(&mut dss, "defaultdaily"), "default");
    assert_eq!(query(&mut dss, "defaultyearly"), "default");
    assert_eq!(query(&mut dss, "pricesignal"), "25");
    assert_eq!(query(&mut dss, "pricecurve"), "");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
}

/// WPG.2: `Set`/`Get LoadShapeClass=` round trip + the `LoadShapeClassEnum`
/// abbreviation behaviour (min_match=1), all transcribed from the pinned
/// oracle (dss-python 0.15.7). Note `d`/`D` are AMBIGUOUS (Daily vs Duty) and
/// the enum falls back to its DefaultValue `None`, while the 2-char `da`/`du`
/// disambiguate and the unique first letters `n`/`y` resolve at one char.
#[test]
fn loadshapeclass_set_get_round_trip_matches_oracle() {
    let query = |dss: &mut Dss, what: &str| -> String {
        dss.command(&format!("get {what}"));
        dss.result().to_string()
    };
    let mut dss = Dss::new();
    dss.command("new circuit.lsc basekv=12.47 bus1=b1 phases=3");
    // Default (unset) is None.
    assert_eq!(query(&mut dss, "loadshapeclass"), "None");
    for (set, get) in [
        ("None", "None"),
        ("Daily", "Daily"),
        ("Yearly", "Yearly"),
        ("Duty", "Duty"),
        ("n", "None"),
        ("y", "Yearly"),
        ("da", "Daily"),
        ("du", "Duty"),
        ("d", "None"), // ambiguous 1-char -> DefaultValue
        ("D", "None"), // case-insensitive, still ambiguous
        ("YEAR", "Yearly"),
        ("dut", "Duty"),
    ] {
        dss.command(&format!("set loadshapeclass={set}"));
        assert!(dss.errors().is_empty(), "set {set}: {:?}", dss.errors());
        assert_eq!(query(&mut dss, "loadshapeclass"), get, "set {set}");
    }
}

/// WP5.8: a daily-mode solve steps the clock through `number` steps.
#[test]
fn daily_mode_advances_the_clock_and_solves() {
    let mut dss = Dss::new();
    dss.command("New circuit.d1 basekv=12.47 pu=1.0 phases=3 mvasc3=2000");
    dss.command("New loadshape.two npts=2 interval=1 mult=(0.5 1.0)");
    dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.2 length=1");
    dss.command("New load.ld bus1=b2 phases=3 kv=12.47 kw=500 pf=0.95 daily=two");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Set mode=daily stepsize=1h number=2");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved);
    // IncrementTime runs before each step: after 2 steps dblHour = 2.0
    // (oracle probe: dblHour 2.0, Hour 2, Seconds 0.0).
    assert_eq!(ckt.solution.int_hour, 2);
    assert_eq!(ckt.solution.dbl_hour, 2.0);
    assert_eq!(ckt.solution.t, 0.0);
    // The built-in default daily shape drove DefaultHourMult (the value
    // itself is the WP5.2 oracle-pinned GetMultAtHour).
    let expected = ckt
        .default_daily_shape_obj
        .clone()
        .expect("default shape resolved")
        .get_mult_at_hour(2.0);
    assert_eq!(ckt.default_hour_mult, expected);
}

/// WP5.8 step 5: `BusCoords` reads `bus, x, y` rows, skipping unknown
/// buses silently; coordinates survive on existing buses.
#[test]
fn bus_coords_sets_coordinates_on_existing_buses() {
    let dir = std::env::temp_dir().join("dss_rs_buscoords_test");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("coords.csv");
    std::fs::write(&file, "sourcebus, 10.5, -3\nnosuchbus, 1, 2\nb2 7 8\n").unwrap();

    let mut dss = Dss::new();
    dss.command("New circuit.bc basekv=12.47 pu=1.0 phases=3");
    dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.2 length=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases"); // builds the bus list
    dss.command(&format!(
        "BusCoords \"{}\"",
        file.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    let sb = ckt.bus_list.find("sourcebus").unwrap();
    assert!(ckt.buses[sb].coord_defined);
    assert_eq!((ckt.buses[sb].x, ckt.buses[sb].y), (10.5, -3.0));
    let b2 = ckt.bus_list.find("b2").unwrap();
    assert_eq!((ckt.buses[b2].x, ckt.buses[b2].y), (7.0, 8.0));
    std::fs::remove_file(&file).ok();
}
