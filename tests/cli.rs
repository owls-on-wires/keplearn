use std::io::Write;
use std::process::{Command, Stdio};

fn wide_table() -> String {
    let mut s = 7u64;
    let mut next = || {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        1.0 + 9.0 * ((s >> 11) as f64 / (1u64 << 53) as f64)
    };
    let mut t = (1..=30).map(|i| format!("c{}\t", i)).collect::<String>() + "y\n";
    for _ in 0..200 {
        let x: Vec<f64> = (0..30).map(|_| next()).collect();
        t += &(x.iter().map(|v| format!("{}\t", v)).collect::<String>()
            + &format!("{}\n", x[6] * x[18].sqrt()));
    }
    t
}

fn run(args: &[&str], input: &str) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_keplearn"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    c.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let o = c.wait_with_output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn the_search_stops_at_the_soft_memory_limit_and_writes_a_result() {
    let (code, out, _) = run(
        &["--target", "y", "--timeout", "0", "--max-memory", "200"],
        &wide_table(),
    );
    assert_eq!(code, 0);
    assert!(out.contains("\"stopped_by\":\"memory\""), "{}", out);
}

#[test]
fn reaching_the_memory_limit_exits_with_status_3() {
    let (code, out, err) = run(
        &["--target", "y", "--timeout", "0", "--max-memory", "1"],
        &wide_table(),
    );
    assert_eq!(code, 3, "{}", err);
    assert!(out.is_empty());
    assert!(err.contains("--max-memory 1 MB"), "{}", err);
}

#[test]
fn every_warning_of_the_result_is_printed_on_stderr() {
    let table = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/planets_2_digits.tsv"
    ))
    .unwrap();
    let (code, out, err) = run(&["--target", "T_years"], &table);
    assert_eq!(code, 0);
    let note = "the exact verdict takes the feature columns as rounded";
    assert!(out.contains(note), "{}", out);
    assert!(err.contains(&format!("keplearn: {}", note)), "{}", err);
}
