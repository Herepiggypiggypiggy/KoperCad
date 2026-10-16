//! Part geometry for the 3D cabinet view (see `docs/design/part-geometry.md`
//! and the contract in `docs/design/3d-view.md`).
//!
//! A part is drawn from a [`PartGeometry`]: levels of detail made from a
//! STEP file the user imported (never bundled), or a plain box from width,
//! height and depth when there is no model or it cannot be used.
//!
//! - [`parse`], [`brep`], [`tess`], [`build`]: STEP text to a triangle soup,
//!   pure Rust. Only the entities a B-rep needs are parsed; assemblies reuse
//!   one tessellation per distinct solid.
//! - [`decimate`], [`mesh`]: weld, simplify to a triangle budget, smooth
//!   normals and feature edges, part-space axes, levels of detail.
//! - [`cache`]: the simplified geometry, kept per user on the device. It is
//!   a cache, never the source of truth: delete the folder and it is
//!   rebuilt from the imported files.
//! - [`service`]: background loading behind the `GeometryProvider` shape the
//!   view asks (`None`, `Pending`, `Ready`, `Failed`), with a time limit.
//! - [`rotation`]: a stored per-part rotation (quarter turns) that fixes a
//!   part that came out turned wrongly.
//! - [`generated`]: parts that have no model but a known shape (DIN rails,
//!   wiring ducts) drawn from their dimensions.
//! - [`instances`]: groups placements of identical parts for instanced
//!   drawing and picks levels of detail against a scene triangle budget.
//!
//! Nothing here touches the GPU, the project file or the network.

pub mod brep;
pub mod build;
pub mod cache;
pub mod decimate;
pub mod generated;
pub mod instances;
pub mod math;
pub mod mesh;
pub mod nurbs;
pub mod parse;
pub mod pins;
pub mod rotation;
pub mod service;
pub mod tess;

use std::sync::atomic::AtomicBool;
use std::time::Instant;

pub use build::{BuildError, StepOptions, StepStats};
pub use mesh::{
    Aabb, FIT_TOLERANCE_MM, Fallback, Fit, MeshBudget, MeshData, MeshOrigin, Orientation,
    PartGeometry, Quality, box_mesh, chord_tolerance, fit_to_footprint,
};
pub use rotation::{PartRotation, geometry_key_rotated};

/// Box used when a part has neither a model nor a size, in millimetres.
pub const DEFAULT_BOX_MM: [f64; 3] = [18.0, 90.0, 60.0];

/// Larger STEP files are not read for a preview (bytes).
pub const MAX_STEP_BYTES: usize = 96 * 1024 * 1024;

/// What to draw for one part.
#[derive(Debug, Clone, Copy)]
pub struct MeshRequest<'a> {
    /// The STEP file's bytes, if the part has one.
    pub step: Option<&'a [u8]>,
    /// Width, height, depth from the catalog, if known.
    pub size_mm: Option<[f64; 3]>,
    pub quality: Quality,
}

/// The geometry for a part. Never fails: a missing or unusable model gives
/// a box (of the catalog size, else [`DEFAULT_BOX_MM`]) whose `origin` says
/// why.
pub fn part_geometry(req: &MeshRequest, cancel: &AtomicBool) -> PartGeometry {
    let Some(bytes) = req.step else {
        return box_mesh(req.size_mm.unwrap_or(DEFAULT_BOX_MM));
    };
    match geometry_from_step(bytes, req.size_mm, req.quality, cancel, None) {
        Ok(m) => m,
        Err(e) => fallback_box(req.size_mm, fallback_for(e)),
    }
}

/// A box that records why the model was not used.
pub fn fallback_box(size_mm: Option<[f64; 3]>, reason: Fallback) -> PartGeometry {
    let mut m = box_mesh(size_mm.unwrap_or(DEFAULT_BOX_MM));
    m.origin = MeshOrigin::Fallback(reason);
    m
}

pub(crate) fn fallback_for(e: BuildError) -> Fallback {
    match e {
        BuildError::NotStep => Fallback::Unreadable,
        BuildError::NoGeometry => Fallback::NoGeometry,
        BuildError::TooComplex => Fallback::TooComplex,
        BuildError::TimedOut => Fallback::TimedOut,
        BuildError::Cancelled => Fallback::Cancelled,
    }
}

/// STEP bytes to levels of detail in part space.
pub fn geometry_from_step(
    bytes: &[u8],
    size_mm: Option<[f64; 3]>,
    quality: Quality,
    cancel: &AtomicBool,
    deadline: Option<Instant>,
) -> Result<PartGeometry, BuildError> {
    geometry_from_step_rotated(
        bytes,
        size_mm,
        quality,
        cancel,
        deadline,
        PartRotation::NONE,
    )
}

/// [`geometry_from_step`] with the part's stored rotation (see
/// [`PartRotation`]): the model is built as usual, then turned by whole
/// quarter turns about its own axes and seated again.
pub fn geometry_from_step_rotated(
    bytes: &[u8],
    size_mm: Option<[f64; 3]>,
    quality: Quality,
    cancel: &AtomicBool,
    deadline: Option<Instant>,
    rotation: PartRotation,
) -> Result<PartGeometry, BuildError> {
    if bytes.len() > MAX_STEP_BYTES {
        return Err(BuildError::TooComplex);
    }
    let text = String::from_utf8_lossy(bytes);
    // A first pass at a tolerance that suits the catalog size (or a medium
    // part); the real size is known only after the geometry is read.
    let guess = size_mm.map_or(60.0, |s| s.iter().cloned().fold(0.0, f64::max));
    let opts = StepOptions {
        chord_tol_mm: chord_tolerance(guess),
        deadline,
        ..StepOptions::default()
    };
    let (soup, stats) = build::step_soup(&text, &opts, cancel)?;
    let (lo, hi) = soup
        .pos
        .iter()
        .fold(([f64::MAX; 3], [f64::MIN; 3]), |(lo, hi), p| {
            (
                [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
                [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
            )
        });
    let longest = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max);
    let budget = MeshBudget::for_size(longest, quality);
    let (lods, orientation, before) = mesh::finish_rotated(soup, size_mm, budget, rotation);
    if lods[0].indices.is_empty() {
        return Err(BuildError::NoGeometry);
    }
    Ok(PartGeometry {
        lods,
        origin: MeshOrigin::Step {
            stats,
            orientation,
            before_simplify: before,
        },
    })
}

/// Stable key of a part's geometry: the part's id and the content hash of
/// its model file. The same inputs give the same key on every run and
/// machine (FNV-1a over both strings), so it can name cache entries.
pub fn geometry_key(part_id: &str, content_id: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in part_id
        .bytes()
        .chain(std::iter::once(0))
        .chain(content_id.bytes())
    {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
