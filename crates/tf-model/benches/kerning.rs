//! Kerning map benchmark (ADR-0017 amendment): 50,000 pairs; set one pair, look one up and
//! iterate all, for `imbl::OrdMap` (the model's [`Kerning`]) and `imbl::HashMap`.
//!
//! Each `set` sample clones the map first (an undo snapshot keeps the old one alive). Exits
//! with status 1 when the model's map has a `set` or `lookup` p95 >= 50 µs. Iterating all
//! pairs is printed, not gated. Run with `cargo bench -p tf-model --bench kerning`.

use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;

use tf_model::{Kerning, ModelError, Name};

const FIRSTS: usize = 250;
const SECONDS: usize = 200; // 250 x 200 = 50,000 pairs
const SAMPLES: usize = 1_000;
const ITERATE_SAMPLES: usize = 100;
const BUDGET_US: f64 = 50.0;
/// Prime stride that spreads the sampled pairs over the whole map.
const STRIDE: usize = 7_919;

type Pair = (Name, Name);
type HashKerning = imbl::HashMap<Pair, f64>;

/// The operations both maps share.
trait PairMap: Clone {
    fn set(&mut self, pair: Pair, value: f64);
    fn lookup(&self, pair: &Pair) -> Option<f64>;
    fn sum(&self) -> f64;
}

impl PairMap for Kerning {
    fn set(&mut self, pair: Pair, value: f64) {
        self.insert(pair, value);
    }
    fn lookup(&self, pair: &Pair) -> Option<f64> {
        self.get(pair).copied()
    }
    fn sum(&self) -> f64 {
        self.values().sum()
    }
}

impl PairMap for HashKerning {
    fn set(&mut self, pair: Pair, value: f64) {
        self.insert(pair, value);
    }
    fn lookup(&self, pair: &Pair) -> Option<f64> {
        self.get(pair).copied()
    }
    fn sum(&self) -> f64 {
        self.values().sum()
    }
}

fn pairs() -> Result<Vec<Pair>, ModelError> {
    let firsts: Vec<Name> = (0..FIRSTS)
        .map(|i| Name::new(&format!("public.kern1.g{i}")))
        .collect::<Result<_, _>>()?;
    let seconds: Vec<Name> = (0..SECONDS)
        .map(|i| Name::new(&format!("g{i}")))
        .collect::<Result<_, _>>()?;
    Ok(firsts
        .iter()
        .flat_map(|a| seconds.iter().map(move |b| (a.clone(), b.clone())))
        .collect())
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

/// Prints one line and returns p95 in µs.
fn report(name: &str, mut samples_us: Vec<f64>) -> f64 {
    samples_us.sort_by(f64::total_cmp);
    let p95 = percentile(&samples_us, 95.0);
    println!(
        "{name}: n={} p50={:.2} µs p95={p95:.2} µs p99={:.2} µs max={:.2} µs",
        samples_us.len(),
        percentile(&samples_us, 50.0),
        percentile(&samples_us, 99.0),
        samples_us[samples_us.len() - 1],
    );
    p95
}

/// Returns the p95 of `set` and `lookup`.
fn bench<M: PairMap>(label: &str, map: &M, pairs: &[Pair]) -> (f64, f64) {
    let pick = |k: usize| &pairs[k * STRIDE % pairs.len()];
    let set = (0..SAMPLES)
        .map(|k| {
            let pair = pick(k).clone();
            let start = Instant::now();
            let mut next = black_box(map).clone();
            next.set(pair, -50.0);
            let elapsed = start.elapsed().as_secs_f64() * 1e6;
            drop(black_box(next));
            elapsed
        })
        .collect();
    let lookup = (0..SAMPLES)
        .map(|k| {
            let pair = pick(k);
            let start = Instant::now();
            black_box(map.lookup(black_box(pair)));
            start.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    let iterate = (0..ITERATE_SAMPLES)
        .map(|_| {
            let start = Instant::now();
            black_box(map.sum());
            start.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    let set = report(&format!("{label}: set one pair"), set);
    let lookup = report(&format!("{label}: look one up"), lookup);
    report(&format!("{label}: iterate {} pairs", pairs.len()), iterate);
    (set, lookup)
}

fn run() -> Result<bool, ModelError> {
    let pairs = pairs()?;
    let ord: Kerning = pairs.iter().map(|p| (p.clone(), -10.0)).collect();
    let hash: HashKerning = pairs.iter().map(|p| (p.clone(), -10.0)).collect();
    let (set, lookup) = bench("imbl::OrdMap (model)", &ord, &pairs);
    bench("imbl::HashMap", &hash, &pairs);
    let ok = set < BUDGET_US && lookup < BUDGET_US;
    if !ok {
        eprintln!(
            "FAIL: the model's kerning map misses {BUDGET_US} µs: set p95 {set:.2} µs, lookup p95 {lookup:.2} µs"
        );
    }
    Ok(ok)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("benchmark setup failed: {error}");
            ExitCode::FAILURE
        }
    }
}
