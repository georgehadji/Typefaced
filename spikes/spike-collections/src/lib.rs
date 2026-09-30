//! Spike 6 (ADR-017): persistent glyph tables for document snapshots.
//!
//! One small interface, [`GlyphTable`], with four implementations, so the benchmarks
//! compare like with like:
//! - [`Naive`]: `Arc<Vec<Arc<Glyph>>>`, the baseline. Every edit copies the whole vector.
//! - [`Chunked`]: in-house `Arc<[Arc<Chunk>]>` with 64 glyphs per chunk. An edit copies
//!   the spine (one pointer per chunk) and the chunks it touches.
//! - [`ImblVector`]: `imbl::Vector<Arc<Glyph>>` (RRB tree, dense index).
//! - [`ImblOrdMap`]: `imbl::OrdMap<GlyphId, Arc<Glyph>>` (B+ tree, sparse keys).
//!
//! `GlyphId` is a dense index here; the map variant shows what sparse IDs would cost.

use std::sync::Arc;

pub type GlyphId = u32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointKind {
    Move,
    Line,
    Curve,
    QCurve,
    OffCurve,
}

/// 24 bytes, like the `Point` sketched in implementation plan §5.1 minus the optional name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub kind: PointKind,
    pub smooth: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub name: String,
    pub contours: Vec<Vec<Point>>,
}

pub const CONTOURS_PER_GLYPH: usize = 4;
pub const POINTS_PER_CONTOUR: usize = 25;
/// Glyphs per chunk in [`Chunked`] (implementation plan §5.1).
pub const CHUNK: usize = 64;
/// Prime stride that spreads edits and lookups over the whole table.
const STRIDE: usize = 7_919;

/// A glyph with `CONTOURS_PER_GLYPH * POINTS_PER_CONTOUR` (100) points.
pub fn make_glyph(id: GlyphId) -> Glyph {
    let kinds = [
        PointKind::Line,
        PointKind::OffCurve,
        PointKind::OffCurve,
        PointKind::Curve,
    ];
    let contours = (0..CONTOURS_PER_GLYPH)
        .map(|c| {
            (0..POINTS_PER_CONTOUR)
                .map(|p| Point {
                    x: f64::from(id) + p as f64,
                    y: (c * 100 + p) as f64,
                    kind: if p == 0 {
                        PointKind::Move
                    } else {
                        kinds[p % kinds.len()]
                    },
                    smooth: p % 3 == 0,
                })
                .collect()
        })
        .collect();
    Glyph {
        name: format!("glyph{id}"),
        contours,
    }
}

pub fn make_glyphs(n: usize) -> Vec<Arc<Glyph>> {
    (0..).take(n).map(|id| Arc::new(make_glyph(id))).collect()
}

/// A copy of `glyph` with every point nudged one unit right: a typical single-glyph edit.
pub fn edited(glyph: &Glyph) -> Glyph {
    let mut out = glyph.clone();
    out.contours.iter_mut().flatten().for_each(|p| p.x += 1.0);
    out
}

/// `count` distinct IDs spread over a table of `n` glyphs (`count <= n`).
pub fn spread_ids(n: usize, count: usize) -> Vec<GlyphId> {
    assert!(count <= n, "cannot pick {count} distinct IDs from {n}");
    (0..count)
        .map(|k| GlyphId::try_from(k * STRIDE % n).expect("glyph count fits in u32"))
        .collect()
}

/// One transaction that replaces `count` spread-out glyphs with edited copies.
pub fn make_edits(glyphs: &[Arc<Glyph>], count: usize) -> Vec<(GlyphId, Arc<Glyph>)> {
    spread_ids(glyphs.len(), count)
        .into_iter()
        .map(|id| (id, Arc::new(edited(&glyphs[id as usize]))))
        .collect()
}

/// A persistent glyph table: cloning is a snapshot, and `replace` leaves `self` untouched.
pub trait GlyphTable: Clone {
    const NAME: &'static str;
    fn from_glyphs(glyphs: Vec<Arc<Glyph>>) -> Self;
    fn get(&self, id: GlyphId) -> Option<&Arc<Glyph>>;
    /// Applies all edits as one transaction and returns the new snapshot.
    /// Panics if an ID is not in the table.
    fn replace(&self, edits: &[(GlyphId, Arc<Glyph>)]) -> Self;
    /// All glyphs in ID order.
    fn iter(&self) -> impl Iterator<Item = &Arc<Glyph>>;
}

#[derive(Clone)]
pub struct Naive(Arc<Vec<Arc<Glyph>>>);

impl GlyphTable for Naive {
    const NAME: &'static str = "naive";

    fn from_glyphs(glyphs: Vec<Arc<Glyph>>) -> Self {
        Self(Arc::new(glyphs))
    }

    fn get(&self, id: GlyphId) -> Option<&Arc<Glyph>> {
        self.0.get(id as usize)
    }

    fn replace(&self, edits: &[(GlyphId, Arc<Glyph>)]) -> Self {
        let mut glyphs = Vec::clone(&self.0);
        for (id, glyph) in edits {
            glyphs[*id as usize] = Arc::clone(glyph);
        }
        Self(Arc::new(glyphs))
    }

    fn iter(&self) -> impl Iterator<Item = &Arc<Glyph>> {
        self.0.iter()
    }
}

pub type Chunk = [Arc<Glyph>];

#[derive(Clone)]
pub struct Chunked(Arc<[Arc<Chunk>]>);

impl GlyphTable for Chunked {
    const NAME: &'static str = "chunked";

    fn from_glyphs(glyphs: Vec<Arc<Glyph>>) -> Self {
        Self(glyphs.chunks(CHUNK).map(Arc::from).collect())
    }

    fn get(&self, id: GlyphId) -> Option<&Arc<Glyph>> {
        let i = id as usize;
        self.0.get(i / CHUNK)?.get(i % CHUNK)
    }

    fn replace(&self, edits: &[(GlyphId, Arc<Glyph>)]) -> Self {
        let mut spine = Arc::clone(&self.0);
        // `make_mut` copies whatever is still shared with `self`: the spine once, and each
        // chunk on its first edit. Later edits to the same chunk write in place.
        let chunks = Arc::make_mut(&mut spine);
        for (id, glyph) in edits {
            let i = *id as usize;
            Arc::make_mut(&mut chunks[i / CHUNK])[i % CHUNK] = Arc::clone(glyph);
        }
        Self(spine)
    }

    fn iter(&self) -> impl Iterator<Item = &Arc<Glyph>> {
        self.0.iter().flat_map(|chunk| chunk.iter())
    }
}

#[derive(Clone)]
pub struct ImblVector(imbl::Vector<Arc<Glyph>>);

impl GlyphTable for ImblVector {
    const NAME: &'static str = "imbl-vector";

    fn from_glyphs(glyphs: Vec<Arc<Glyph>>) -> Self {
        Self(glyphs.into())
    }

    fn get(&self, id: GlyphId) -> Option<&Arc<Glyph>> {
        self.0.get(id as usize)
    }

    fn replace(&self, edits: &[(GlyphId, Arc<Glyph>)]) -> Self {
        let mut glyphs = self.0.clone();
        for (id, glyph) in edits {
            glyphs.set(*id as usize, Arc::clone(glyph));
        }
        Self(glyphs)
    }

    fn iter(&self) -> impl Iterator<Item = &Arc<Glyph>> {
        self.0.iter()
    }
}

#[derive(Clone)]
pub struct ImblOrdMap(imbl::OrdMap<GlyphId, Arc<Glyph>>);

impl GlyphTable for ImblOrdMap {
    const NAME: &'static str = "imbl-ordmap";

    fn from_glyphs(glyphs: Vec<Arc<Glyph>>) -> Self {
        Self((GlyphId::MIN..).zip(glyphs).collect())
    }

    fn get(&self, id: GlyphId) -> Option<&Arc<Glyph>> {
        self.0.get(&id)
    }

    fn replace(&self, edits: &[(GlyphId, Arc<Glyph>)]) -> Self {
        let mut glyphs = self.0.clone();
        for (id, glyph) in edits {
            let old = glyphs.insert(*id, Arc::clone(glyph));
            assert!(old.is_some(), "glyph {id} is not in the table");
        }
        Self(glyphs)
    }

    fn iter(&self) -> impl Iterator<Item = &Arc<Glyph>> {
        self.0.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1,000 is not a multiple of `CHUNK`, so the last chunk is partial.
    const N: usize = 1_000;

    fn ptrs<T: GlyphTable>(table: &T) -> Vec<*const Glyph> {
        table.iter().map(Arc::as_ptr).collect()
    }

    /// Builds a table, runs a 100-glyph transaction and four single edits (at chunk
    /// edges and on the last glyph), checks persistence and `get`/`iter` agreement, and
    /// returns the final glyphs by value.
    fn scenario<T: GlyphTable>(glyphs: &[Arc<Glyph>]) -> Vec<Glyph> {
        let base = T::from_glyphs(glyphs.to_vec());
        let before = ptrs(&base);
        assert_eq!(before, glyphs.iter().map(Arc::as_ptr).collect::<Vec<_>>());
        let mut table = base.replace(&make_edits(glyphs, 100));
        for id in [0, 63, 64, N as GlyphId - 1] {
            let glyph = Arc::new(edited(table.get(id).expect("id in range")));
            table = table.replace(&[(id, glyph)]);
        }
        assert_eq!(table.iter().count(), N, "{}", T::NAME);
        assert_eq!(ptrs(&base), before, "{}: base snapshot changed", T::NAME);
        for (id, glyph) in (0..).zip(table.iter()) {
            let got = table.get(id).expect("id in range");
            assert!(
                Arc::ptr_eq(got, glyph),
                "{}: get and iter disagree at {id}",
                T::NAME
            );
        }
        assert!(table.get(N as GlyphId).is_none(), "{}", T::NAME);
        table.iter().map(|glyph| Glyph::clone(glyph)).collect()
    }

    #[test]
    fn all_candidates_agree() {
        let glyphs = make_glyphs(N);
        let expected = scenario::<Naive>(&glyphs);
        let changed = (0..N).filter(|&i| expected[i] != *glyphs[i]).count();
        assert_eq!(
            changed, 103,
            "100 spread edits plus 63, 64 and 999 (0 is in both)"
        );
        assert_eq!(scenario::<Chunked>(&glyphs), expected);
        assert_eq!(scenario::<ImblVector>(&glyphs), expected);
        assert_eq!(scenario::<ImblOrdMap>(&glyphs), expected);
    }

    #[test]
    fn edits_touch_distinct_glyphs() {
        let mut ids = spread_ids(30_000, 200);
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 200);
    }

    #[test]
    fn glyphs_have_100_points_of_24_bytes() {
        let glyph = make_glyph(7);
        assert_eq!(glyph.contours.iter().map(Vec::len).sum::<usize>(), 100);
        assert_eq!(size_of::<Point>(), 24);
        assert_ne!(edited(&glyph), glyph);
    }

    #[test]
    #[should_panic(expected = "not in the table")]
    fn ordmap_rejects_unknown_ids() {
        let table = ImblOrdMap::from_glyphs(make_glyphs(3));
        let _ = table.replace(&[(3, Arc::new(make_glyph(3)))]);
    }
}
