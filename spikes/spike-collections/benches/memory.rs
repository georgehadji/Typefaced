//! Memory held by 200 undo snapshots with one single-glyph edit each, counted with a
//! counting global allocator. Deterministic, so one run is enough.
//! Run: `cargo bench --manifest-path spikes/spike-collections/Cargo.toml --bench memory`

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use spike_collections::{
    Chunked, GlyphTable, ImblOrdMap, ImblVector, Naive, edited, make_glyphs, spread_ids,
};

// 65,535 is the OpenType maximum (a u16 glyph ID).
const SIZES: [usize; 3] = [1_000, 30_000, 65_535];
const SNAPSHOTS: usize = 200;
const MIB: f64 = 1024.0 * 1024.0;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

/// Forwards to the system allocator and counts live bytes and allocation calls.
/// `realloc` and `alloc_zeroed` keep their default implementations, which call
/// `alloc` and `dealloc` below, so the counts stay exact.
struct Counting;

// SAFETY: every call is forwarded unchanged to `System`, which upholds the
// `GlobalAlloc` contract; the counters are plain atomics and never touch memory.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: same `layout` the caller passed us, under the same contract.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Relaxed);
            ALLOCATIONS.fetch_add(1, Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `alloc` above with this `layout`.
        unsafe { System.dealloc(ptr, layout) };
        LIVE_BYTES.fetch_sub(layout.size(), Relaxed);
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn live() -> i64 {
    LIVE_BYTES.load(Relaxed) as i64
}

fn measure<T: GlyphTable>(n: usize) {
    let start = live();
    let base = T::from_glyphs(make_glyphs(n));
    let base_bytes = live() - start;

    let ids = spread_ids(n, SNAPSHOTS);
    let mut history: Vec<T> = Vec::with_capacity(SNAPSHOTS);
    let before = live();
    let allocations = ALLOCATIONS.load(Relaxed);
    let mut edited_bytes = 0;
    for &id in &ids {
        let current = history.last().unwrap_or(&base);
        let glyph_start = live();
        let glyph = Arc::new(edited(current.get(id).expect("id in range")));
        edited_bytes += live() - glyph_start;
        history.push(current.replace(&[(id, glyph)]));
    }
    let held = live() - before;
    let allocations = ALLOCATIONS.load(Relaxed) - allocations;

    let pct = |bytes: i64| 100.0 * bytes as f64 / base_bytes as f64;
    println!(
        "| {} | {n} | {:.2} | {:.2} | {:.1}% | {:.2}% | {allocations} |",
        T::NAME,
        base_bytes as f64 / MIB,
        held as f64 / MIB,
        pct(held),
        pct(held - edited_bytes),
    );
}

fn main() {
    println!("Memory held by {SNAPSHOTS} snapshots, one single-glyph edit each");
    println!();
    println!(
        "| Candidate | Glyphs | Base (MiB) | Held by snapshots (MiB) | Growth vs. base | Growth excl. edited glyphs | Allocations |"
    );
    println!("|---|---:|---:|---:|---:|---:|---:|");
    for n in SIZES {
        measure::<Naive>(n);
        measure::<Chunked>(n);
        measure::<ImblVector>(n);
        measure::<ImblOrdMap>(n);
    }
}
