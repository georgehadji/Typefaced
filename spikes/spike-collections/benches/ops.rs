//! Timing benchmarks for each candidate at 1,000 and 30,000 glyphs.
//! Run: `cargo bench --manifest-path spikes/spike-collections/Cargo.toml --bench ops`

use std::hint::black_box;
use std::sync::Arc;

use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, BenchmarkId, Criterion, criterion_group, criterion_main};
use spike_collections::{
    Chunked, Glyph, GlyphTable, ImblOrdMap, ImblVector, Naive, make_edits, make_glyphs, spread_ids,
};

// 65,535 is the OpenType maximum (a u16 glyph ID).
const SIZES: [usize; 3] = [1_000, 30_000, 65_535];
const LOOKUPS: usize = 1_000;

fn run<T: GlyphTable>(group: &mut BenchmarkGroup<'_, WallTime>, glyphs: &[Arc<Glyph>]) {
    let table = T::from_glyphs(glyphs.to_vec());
    let one = make_edits(glyphs, 1);
    let hundred = make_edits(glyphs, 100);
    let ids = spread_ids(glyphs.len(), LOOKUPS);

    // A clone only bumps root reference counts, so its drop is cheap and stays in the timing.
    group.bench_function(BenchmarkId::new("clone", T::NAME), |b| {
        b.iter(|| black_box(&table).clone());
    });
    // Dropping the new snapshot is left out: history evicts old snapshots off the edit path.
    group.bench_function(BenchmarkId::new("replace_1", T::NAME), |b| {
        b.iter_with_large_drop(|| black_box(&table).replace(black_box(&one)));
    });
    group.bench_function(BenchmarkId::new("replace_100", T::NAME), |b| {
        b.iter_with_large_drop(|| black_box(&table).replace(black_box(&hundred)));
    });
    group.bench_function(BenchmarkId::new("iterate", T::NAME), |b| {
        b.iter(|| {
            black_box(&table)
                .iter()
                .map(|g| g.contours.len())
                .sum::<usize>()
        });
    });
    group.bench_function(BenchmarkId::new("lookup_1000", T::NAME), |b| {
        b.iter(|| {
            let table = black_box(&table);
            ids.iter()
                .filter_map(|&id| table.get(id))
                .map(|g| g.contours.len())
                .sum::<usize>()
        });
    });
}

fn bench(c: &mut Criterion) {
    for n in SIZES {
        let glyphs = make_glyphs(n);
        let mut group = c.benchmark_group(format!("{n}_glyphs"));
        run::<Naive>(&mut group, &glyphs);
        run::<Chunked>(&mut group, &glyphs);
        run::<ImblVector>(&mut group, &glyphs);
        run::<ImblOrdMap>(&mut group, &glyphs);
        group.finish();
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
