//! Time per glyph: each benchmark runs one candidate over a whole case group.
//! Only cases that every candidate completes are timed, so both engines run on
//! the same set; the report run (`cargo run`) counts the failures.

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use spike_boolean::cases::{self, Case};
use spike_boolean::engines::{CANDIDATES, Outcome, run_guarded};

const GROUPS: &[&str] = &["source-sans", "fixtures", "synthetic"];

fn completes_everywhere(case: &Case) -> bool {
    CANDIDATES.iter().all(|&e| {
        matches!(
            run_guarded(e, &case.path, Duration::from_secs(30)),
            Outcome::Ok(..)
        )
    })
}

fn bench(c: &mut Criterion) {
    let all = match cases::all(&cases::default_tests_dir()) {
        Ok(all) => all,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };
    let timed: Vec<&Case> = all.iter().filter(|c| completes_everywhere(c)).collect();
    eprintln!("timing {} of {} cases", timed.len(), all.len());
    for &engine in CANDIDATES {
        let mut group = c.benchmark_group(engine.name());
        for &name in GROUPS {
            let paths: Vec<_> = timed
                .iter()
                .filter(|c| c.group == name)
                .map(|c| &c.path)
                .collect();
            group.bench_function(format!("{name} ({} glyphs)", paths.len()), |b| {
                b.iter(|| {
                    for p in &paths {
                        let _ = black_box(engine.remove_overlaps(black_box(p)));
                    }
                });
            });
        }
        group.finish();
    }
}

criterion_group! {
    name = benches;
    // A source-sans iteration takes 0.2-1 s, so fewer, longer samples.
    config = Criterion::default().sample_size(10).measurement_time(Duration::from_secs(20));
    targets = bench
}
criterion_main!(benches);
