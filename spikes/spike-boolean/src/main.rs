//! Runs every engine on every case and writes a Markdown report.
//!
//! cargo run --release --manifest-path spikes/spike-boolean/Cargo.toml -- --report target/spike-boolean.md

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use spike_boolean::cases::{self, Case};
use spike_boolean::engines::{ENGINES, Engine, Outcome, run_guarded};
use spike_boolean::measure::{self, Comparison, Structure};

/// A result above this fraction of the reference area is a failure.
const AREA_TOLERANCE: f64 = 1e-3;
const TIMEOUT: Duration = Duration::from_secs(30);
/// Timing repetitions per case (median); one run if the first takes over a second.
const REPS: usize = 5;

struct Row<'a> {
    case: &'a Case,
    engine: Engine,
    status: String,
    structure: Structure,
    comparison: Comparison,
    micros: f64,
}

impl Row<'_> {
    fn ok(&self) -> bool {
        self.status == "ok"
    }

    /// Output segments per input segment: point-count growth.
    fn segment_ratio(&self) -> Option<f64> {
        let input = measure::structure(&self.case.path).segments;
        (input > 0).then(|| self.structure.segments as f64 / input as f64)
    }

    fn failures(&self) -> Vec<String> {
        let c = &self.comparison;
        let rel = |a: f64| a / c.reference_area.max(1.0);
        let mut f = Vec::new();
        if !self.ok() {
            f.push(self.status.clone());
            return f;
        }
        if [c.reference_area, c.output_area, c.xor_area]
            .iter()
            .any(|a| a.is_nan())
        {
            f.push("area error NaN".to_owned());
        }
        if self.structure.open_contours > 0 {
            f.push(format!("{} open contours", self.structure.open_contours));
        }
        if self.structure.non_finite {
            f.push("non-finite coordinates".to_owned());
        }
        if rel(c.overlap_area) > AREA_TOLERANCE {
            f.push(format!("overlap left {:.4}%", 100.0 * rel(c.overlap_area)));
        }
        if rel(c.mixed_winding_area()) > AREA_TOLERANCE {
            f.push(format!(
                "mixed winding {:.4}%",
                100.0 * rel(c.mixed_winding_area())
            ));
        }
        if c.area_error() > AREA_TOLERANCE {
            f.push(format!("area error {:.4}%", 100.0 * c.area_error()));
        }
        f
    }
}

fn run_case(case: &Case, engine: Engine) -> Row<'_> {
    let mut row = Row {
        case,
        engine,
        status: "ok".to_owned(),
        structure: Structure::default(),
        comparison: Comparison::default(),
        micros: f64::NAN,
    };
    let (out, first) = match run_guarded(engine, &case.path, TIMEOUT) {
        Outcome::Ok(out, elapsed) => (out, elapsed),
        Outcome::Error(e) => {
            row.status = format!("error: {e}");
            return row;
        }
        Outcome::Panic(p) => {
            row.status = format!("panic: {}", p.lines().next().unwrap_or_default());
            return row;
        }
        Outcome::Timeout => {
            row.status = format!("timeout (> {} s)", TIMEOUT.as_secs());
            return row;
        }
    };
    let mut times = vec![first];
    // The engines are deterministic, so a case that ran once is safe to repeat directly.
    if first < Duration::from_secs(1) {
        for _ in 1..REPS {
            let start = Instant::now();
            let _ = std::hint::black_box(engine.remove_overlaps(&case.path));
            times.push(start.elapsed());
        }
    }
    times.sort();
    row.micros = times[times.len() / 2].as_secs_f64() * 1e6;
    row.structure = measure::structure(&out);
    // Flattening a non-finite path does not terminate; such output fails anyway.
    if !row.structure.non_finite {
        row.comparison = measure::compare(&case.path, &out);
    }
    row
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn summary_line(out: &mut String, label: &str, rows: &[&Row]) {
    let count = |pred: &dyn Fn(&Row) -> bool| rows.iter().filter(|r| pred(r)).count();
    let failed =
        |needle: &str| count(&|r: &Row| r.failures().iter().any(|f| f.starts_with(needle)));
    let ok: Vec<&&Row> = rows.iter().filter(|r| r.ok()).collect();
    let worst = ok.iter().max_by(|a, b| {
        a.comparison
            .area_error()
            .total_cmp(&b.comparison.area_error())
    });
    let mut times: Vec<f64> = ok.iter().map(|r| r.micros).collect();
    times.sort_by(f64::total_cmp);
    let tiny: usize = ok.iter().map(|r| r.structure.tiny_contours).sum();
    let mut ratios: Vec<f64> = ok.iter().filter_map(|r| r.segment_ratio()).collect();
    ratios.sort_by(f64::total_cmp);
    let _ = writeln!(
        out,
        "| {label} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} ({tiny}) | {:.2} ({:.2}) | {} | {:.0} | {:.0} | {:.0} | {:.1} |",
        rows.len(),
        failed("panic"),
        failed("error"),
        failed("timeout"),
        count(&|r: &Row| r.ok() && r.structure.open_contours > 0),
        count(&|r: &Row| r.ok() && r.structure.non_finite),
        failed("overlap left"),
        failed("mixed winding"),
        failed("area error"),
        count(&|r: &Row| r.ok() && r.comparison.area_error_even_odd() > AREA_TOLERANCE),
        count(&|r: &Row| !r.failures().is_empty()),
        count(&|r: &Row| r.ok() && r.structure.tiny_contours > 0),
        percentile(&ratios, 0.5),
        ratios.last().copied().unwrap_or(f64::NAN),
        worst.map_or("—".to_owned(), |r| format!(
            "{:.4}% ({})",
            100.0 * r.comparison.area_error(),
            r.case.name
        )),
        percentile(&times, 0.5),
        percentile(&times, 0.95),
        times.last().copied().unwrap_or(f64::NAN),
        times.iter().sum::<f64>() / 1000.0,
    );
}

const SUMMARY_HEADER: &str = "| Engine | Cases | Panics | Errors | Timeouts | Open contours | Non-finite | Overlap left | Mixed winding | Area error > 0.1% | Even-odd area error > 0.1% | Cases failing | Cases with tiny contours (total) | Segments out/in, median (max) | Worst area error (case) | Median µs | p95 µs | Max µs | Total ms |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|---:|\n";

fn report(cases: &[Case], rows: &[Row]) -> String {
    let mut out = String::from("# Spike 5 results (generated by spike-boolean)\n\n");
    let groups: Vec<&str> = {
        let mut g: Vec<&str> = cases.iter().map(|c| c.group).collect();
        g.dedup();
        g
    };
    let _ = writeln!(
        out,
        "Engines: skia-pathops = skia-safe 0.153.3 `simplify` then `as_winding` (winding fill); skia-simplify-only = `simplify` alone (diagnostic); skia-union-only = `op` union with an empty path (diagnostic); linesweeper = linesweeper 0.4.0 `binary_op` union with an empty set (non-zero). {} cases: {}.\n",
        cases.len(),
        groups
            .iter()
            .map(|g| format!("{g} {}", cases.iter().filter(|c| c.group == *g).count()))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(
        out,
        "Thresholds: overlap left, mixed winding and area error fail above {}% of the reference area (floored at 1 unit²).\n",
        AREA_TOLERANCE * 100.0
    );
    out.push_str("## By engine\n\n");
    out.push_str(SUMMARY_HEADER);
    for &e in ENGINES {
        let r: Vec<&Row> = rows.iter().filter(|r| r.engine == e).collect();
        summary_line(&mut out, e.name(), &r);
    }
    out.push_str("\n## By engine and group\n\n");
    out.push_str(SUMMARY_HEADER);
    for &e in ENGINES {
        for g in &groups {
            let r: Vec<&Row> = rows
                .iter()
                .filter(|r| r.engine == e && r.case.group == *g)
                .collect();
            summary_line(&mut out, &format!("{} / {g}", e.name()), &r);
        }
    }
    out.push_str("\n## Failures\n\n| Group | Case | Engine | Failures |\n|---|---|---|---|\n");
    for r in rows.iter().filter(|r| !r.failures().is_empty()) {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            r.case.group,
            r.case.name,
            r.engine.name(),
            r.failures().join("; ")
        );
    }
    out.push_str("\n## All cases\n\nAreas in square units.\n\n| Group | Case | Engine | Status | Out contours | Tiny contours | Area error % | Even-odd area error % | Overlap left | Mixed winding | Ref area | µs |\n|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for r in rows {
        let c = &r.comparison;
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {:.5} | {:.5} | {:.3} | {:.3} | {:.1} | {:.0} |",
            r.case.group,
            r.case.name,
            r.engine.name(),
            r.status,
            r.structure.contours,
            r.structure.tiny_contours,
            100.0 * c.area_error(),
            100.0 * c.area_error_even_odd(),
            c.overlap_area,
            c.mixed_winding_area(),
            c.reference_area,
            r.micros
        );
    }
    out
}

/// Writes the input and each engine's output for one case as SVG files next to
/// the report, for inspection (`--svg <case name>`).
fn dump_svg(cases: &[Case], name: &str, report_path: &std::path::Path) {
    let Some(case) = cases.iter().find(|c| c.name == name) else {
        eprintln!("error: no case named {name}");
        std::process::exit(2);
    };
    let dir = report_path.parent().unwrap_or(std::path::Path::new("."));
    let bbox = kurbo::Shape::bounding_box(&case.path).inflate(20.0, 20.0);
    let write = |label: &str, path: &kurbo::BezPath| {
        // Flip y: fonts are y-up, SVG is y-down. Non-zero fill, as in fonts.
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\"><path transform=\"scale(1,-1)\" fill-rule=\"nonzero\" fill=\"#0006\" stroke=\"red\" stroke-width=\"1\" d=\"{}\"/></svg>\n",
            bbox.x0,
            -bbox.y1,
            bbox.width(),
            bbox.height(),
            path.to_svg()
        );
        let file = dir.join(format!("{}-{label}.svg", name.replace('/', "_")));
        match std::fs::write(&file, svg) {
            Ok(()) => eprintln!("wrote {}", file.display()),
            Err(e) => eprintln!("error: {}: {e}", file.display()),
        }
    };
    write("input", &case.path);
    for &engine in ENGINES {
        match engine.remove_overlaps(&case.path) {
            Ok(out) => write(engine.name(), &out),
            Err(e) => eprintln!("{}: {e}", engine.name()),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
    };
    let report_path =
        PathBuf::from(flag("--report").map_or("target/spike-boolean.md", String::as_str));
    let tests = flag("--tests").map_or_else(cases::default_tests_dir, PathBuf::from);

    let cases = match cases::all(&tests) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };
    if let Some(name) = flag("--svg") {
        dump_svg(&cases, name, &report_path);
        return;
    }
    eprintln!("{} cases, engines: {:?}", cases.len(), ENGINES);
    let mut rows = Vec::new();
    for (i, case) in cases.iter().enumerate() {
        for &engine in ENGINES {
            rows.push(run_case(case, engine));
        }
        if (i + 1) % 100 == 0 {
            eprintln!("{} / {}", i + 1, cases.len());
        }
    }
    let text = report(&cases, &rows);
    if let Some(dir) = report_path.parent().filter(|d| !d.as_os_str().is_empty())
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        eprintln!("error: {}: {e}", dir.display());
        std::process::exit(1);
    }
    if let Err(e) = std::fs::write(&report_path, &text) {
        eprintln!("error: {}: {e}", report_path.display());
        std::process::exit(1);
    }
    // Print the summary tables (everything before the failure list).
    print!("{}", text.split("\n## Failures").next().unwrap_or_default());
    eprintln!("report written to {}", report_path.display());
}
