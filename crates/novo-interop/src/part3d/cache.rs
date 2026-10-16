//! Simplified meshes kept per user so a model is read from its STEP file
//! once, not every time the 3D view opens.
//!
//! Files live under `%LOCALAPPDATA%\KoperCAD\cache\meshes` (no admin rights;
//! another folder for tests and portable setups). A file is named from the
//! content hash of the source model, the algorithm version, the quality and
//! the catalog size the axes were matched to, so a changed model, a better
//! algorithm or a different setting never reads a stale mesh. A failed
//! build is remembered too, so a model that cannot be read is not parsed
//! again on every start. Deleting the folder only costs rebuild time.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use super::build::StepStats;
use super::mesh::{Aabb, Fallback, MeshData, MeshOrigin, Orientation, PartGeometry, Quality};
use super::rotation::PartRotation;
use super::{MAX_STEP_BYTES, fallback_box, fallback_for, geometry_from_step_rotated};

/// Bump when tessellation, simplification or normals change: older cache
/// files are then ignored (and pruned).
pub const ALGORITHM_VERSION: u32 = 2;
const FORMAT: u32 = 2;
const MAGIC: &[u8; 4] = b"KMSH";

pub struct MeshCache {
    root: PathBuf,
}

/// What the cache knew about a key.
#[derive(Debug, PartialEq)]
pub enum Cached {
    Geometry(PartGeometry),
    /// A build failed earlier for this exact input.
    Failed(Fallback),
}

impl MeshCache {
    pub fn at(root: impl Into<PathBuf>) -> MeshCache {
        MeshCache { root: root.into() }
    }

    /// The per-user folder, or `None` when the platform has no user data
    /// folder.
    pub fn default_root() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| d.join("KoperCAD").join("cache").join("meshes"))
    }

    pub fn open_default() -> Option<MeshCache> {
        Self::default_root().map(Self::at)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path(&self, key: &str) -> PathBuf {
        self.root.join(format!("{key}.kmsh"))
    }

    pub fn get(&self, key: &str) -> Option<Cached> {
        let bytes = fs::read(self.path(key)).ok()?;
        decode(&bytes)
    }

    pub fn put(&self, key: &str, geometry: &PartGeometry) -> io::Result<()> {
        self.write(key, &encode(Some(geometry), None))
    }

    pub fn put_failure(&self, key: &str, reason: Fallback) -> io::Result<()> {
        self.write(key, &encode(None, Some(reason)))
    }

    /// Writes through a temporary file so a crash never leaves half a mesh.
    fn write(&self, key: &str, bytes: &[u8]) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        let target = self.path(key);
        let tmp = self.root.join(format!("{key}.tmp"));
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &target)
    }

    /// Deletes the oldest files until the folder is at most `max_bytes`,
    /// and every file of an older algorithm or format. Returns how many
    /// files were removed.
    pub fn prune(&self, max_bytes: u64) -> usize {
        let Ok(rd) = fs::read_dir(&self.root) else {
            return 0;
        };
        let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
        let mut removed = 0;
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_none_or(|x| x != "kmsh") {
                continue;
            }
            let stale = p
                .file_stem()
                .and_then(|s| s.to_str())
                .is_none_or(|s| !s.contains(&format!("-a{ALGORITHM_VERSION}-")));
            if stale {
                removed += usize::from(fs::remove_file(&p).is_ok());
                continue;
            }
            if let Ok(m) = e.metadata() {
                files.push((p, m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)));
            }
        }
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        files.sort_by_key(|f| f.2);
        for (p, len, _) in files {
            if total <= max_bytes {
                break;
            }
            if fs::remove_file(&p).is_ok() {
                total -= len;
                removed += 1;
            }
        }
        removed
    }
}

/// File name stem for a model: `content_id` (the SHA-256 the parts library
/// records, any identifier made of letters and digits works), algorithm
/// version, quality and the size the axes were matched to.
pub fn cache_key(content_id: &str, size_mm: Option<[f64; 3]>, quality: Quality) -> String {
    cache_key_rotated(content_id, size_mm, quality, PartRotation::NONE)
}

/// [`cache_key`] for a part with a stored rotation. Without a rotation the
/// key is the same as before, so existing cache files stay valid.
pub fn cache_key_rotated(
    content_id: &str,
    size_mm: Option<[f64; 3]>,
    quality: Quality,
    rotation: PartRotation,
) -> String {
    let id: String = content_id
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(64)
        .collect();
    let q = match quality {
        Quality::Draft => 'd',
        Quality::Normal => 'n',
        Quality::Detailed => 'h',
    };
    let dims = size_mm.map_or(0u32, |s| {
        let mut h: u32 = 0x811c_9dc5;
        for v in s {
            for b in ((v * 10.0).round() as i64).to_le_bytes() {
                h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
            }
        }
        h | 1
    });
    let turned = if rotation.is_none() {
        String::new()
    } else {
        format!("r{:02x}", rotation.code())
    };
    format!("{id}-a{ALGORITHM_VERSION}-{q}{dims:08x}{turned}")
}

/// The geometry for a model file, from the cache when it is there.
///
/// `read_step` is only called on a miss. Failures (unreadable, too large,
/// no geometry, too slow) are cached; a cancelled build is not. Disk errors
/// never fail the call: the geometry is simply not cached.
pub fn cached_part_geometry(
    cache: &MeshCache,
    content_id: &str,
    size_mm: Option<[f64; 3]>,
    quality: Quality,
    read_step: impl FnOnce() -> io::Result<Vec<u8>>,
    cancel: &AtomicBool,
    deadline: Option<Instant>,
) -> PartGeometry {
    cached_part_geometry_rotated(
        cache,
        content_id,
        size_mm,
        quality,
        PartRotation::NONE,
        read_step,
        cancel,
        deadline,
    )
}

/// [`cached_part_geometry`] for a part with a stored rotation.
#[allow(clippy::too_many_arguments)]
pub fn cached_part_geometry_rotated(
    cache: &MeshCache,
    content_id: &str,
    size_mm: Option<[f64; 3]>,
    quality: Quality,
    rotation: PartRotation,
    read_step: impl FnOnce() -> io::Result<Vec<u8>>,
    cancel: &AtomicBool,
    deadline: Option<Instant>,
) -> PartGeometry {
    let key = cache_key_rotated(content_id, size_mm, quality, rotation);
    match cache.get(&key) {
        Some(Cached::Geometry(g)) => return g,
        Some(Cached::Failed(r)) => return fallback_box(size_mm, r),
        None => {}
    }
    let bytes = match read_step() {
        Ok(b) => b,
        Err(_) => return fallback_box(size_mm, Fallback::Unreadable),
    };
    if bytes.len() > MAX_STEP_BYTES {
        let _ = cache.put_failure(&key, Fallback::TooComplex);
        return fallback_box(size_mm, Fallback::TooComplex);
    }
    match geometry_from_step_rotated(&bytes, size_mm, quality, cancel, deadline, rotation) {
        Ok(g) => {
            let _ = cache.put(&key, &g);
            g
        }
        Err(e) => {
            let r = fallback_for(e);
            if r != Fallback::Cancelled {
                let _ = cache.put_failure(&key, r);
            }
            fallback_box(size_mm, r)
        }
    }
}

// ---- file format ---------------------------------------------------------
//
// "KMSH" | u32 format | u32 algorithm | u8 kind (0 mesh, 1 failure)
// failure:  u8 reason
// geometry: u8 orientation | 7 x u32 stats | u32 levels, then per level
//           u32 vertices | u32 indices | u32 edges | positions | normals |
//           indices | edge index pairs
// then a 64-bit FNV-1a of everything before it. Little endian throughout.

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn reason_byte(r: Fallback) -> u8 {
    match r {
        Fallback::NoModel => 0,
        Fallback::Unreadable => 1,
        Fallback::NoGeometry => 2,
        Fallback::TooComplex => 3,
        Fallback::Cancelled => 4,
        Fallback::TimedOut => 5,
    }
}

fn reason_of(b: u8) -> Option<Fallback> {
    Some(match b {
        0 => Fallback::NoModel,
        1 => Fallback::Unreadable,
        2 => Fallback::NoGeometry,
        3 => Fallback::TooComplex,
        4 => Fallback::Cancelled,
        5 => Fallback::TimedOut,
        _ => return None,
    })
}

fn encode(geometry: Option<&PartGeometry>, failure: Option<Fallback>) -> Vec<u8> {
    let mut o: Vec<u8> = Vec::new();
    o.extend_from_slice(MAGIC);
    o.extend_from_slice(&FORMAT.to_le_bytes());
    o.extend_from_slice(&ALGORITHM_VERSION.to_le_bytes());
    match (geometry, failure) {
        (Some(g), _) => {
            o.push(0);
            let (orient, stats, before) = match g.origin {
                MeshOrigin::Step {
                    stats,
                    orientation,
                    before_simplify,
                } => (
                    u8::from(orientation == Orientation::Guessed),
                    stats,
                    before_simplify,
                ),
                _ => (0, StepStats::default(), 0),
            };
            o.push(orient);
            for v in [
                stats.solids,
                stats.placements,
                stats.faces,
                stats.approximated_faces,
                stats.skipped_faces,
                stats.raw_triangles,
                before,
            ] {
                o.extend_from_slice(&v.to_le_bytes());
            }
            o.extend_from_slice(&(g.lods.len() as u32).to_le_bytes());
            for m in &g.lods {
                o.extend_from_slice(&(m.positions.len() as u32).to_le_bytes());
                o.extend_from_slice(&(m.indices.len() as u32).to_le_bytes());
                o.extend_from_slice(&(m.edges.len() as u32).to_le_bytes());
                for v in m.positions.iter().chain(&m.normals).flatten() {
                    o.extend_from_slice(&v.to_le_bytes());
                }
                for i in &m.indices {
                    o.extend_from_slice(&i.to_le_bytes());
                }
                for i in m.edges.iter().flatten() {
                    o.extend_from_slice(&i.to_le_bytes());
                }
            }
        }
        (None, r) => {
            o.push(1);
            o.push(reason_byte(r.unwrap_or(Fallback::Unreadable)));
        }
    }
    let sum = fnv(&o);
    o.extend_from_slice(&sum.to_le_bytes());
    o
}

struct Rd<'a>(&'a [u8]);

impl Rd<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        if self.0.len() < n {
            return None;
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Some(a)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f32(&mut self) -> Option<f32> {
        self.u32().map(f32::from_bits)
    }
}

fn decode(bytes: &[u8]) -> Option<Cached> {
    if bytes.len() < 8 + 4 + 4 + 1 {
        return None;
    }
    let (body, sum) = bytes.split_at(bytes.len() - 8);
    if fnv(body).to_le_bytes() != sum {
        return None;
    }
    let mut r = Rd(body);
    if r.take(4)? != MAGIC || r.u32()? != FORMAT || r.u32()? != ALGORITHM_VERSION {
        return None;
    }
    if r.u8()? == 1 {
        return reason_of(r.u8()?).map(Cached::Failed);
    }
    let orientation = if r.u8()? == 1 {
        Orientation::Guessed
    } else {
        Orientation::MatchedToCatalog
    };
    let stats = StepStats {
        solids: r.u32()?,
        placements: r.u32()?,
        faces: r.u32()?,
        approximated_faces: r.u32()?,
        skipped_faces: r.u32()?,
        raw_triangles: r.u32()?,
    };
    let before_simplify = r.u32()?;
    let levels = r.u32()? as usize;
    if levels == 0 || levels > 8 {
        return None;
    }
    let mut lods = Vec::with_capacity(levels);
    for _ in 0..levels {
        let nv = r.u32()? as usize;
        let ni = r.u32()? as usize;
        let ne = r.u32()? as usize;
        // Sizes come from the file: check them against what is left before
        // allocating, so a damaged header cannot ask for gigabytes.
        let need = nv
            .checked_mul(24)?
            .checked_add(ni.checked_mul(4)?)?
            .checked_add(ne.checked_mul(8)?)?;
        if need > r.0.len() || !ni.is_multiple_of(3) {
            return None;
        }
        let mut read3 = |n: usize| -> Option<Vec<[f32; 3]>> {
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push([r.f32()?, r.f32()?, r.f32()?]);
            }
            Some(v)
        };
        let positions = read3(nv)?;
        let normals = read3(nv)?;
        let mut indices = Vec::with_capacity(ni);
        for _ in 0..ni {
            let i = r.u32()?;
            if i as usize >= nv {
                return None;
            }
            indices.push(i);
        }
        let mut edges = Vec::with_capacity(ne);
        for _ in 0..ne {
            let (a, b) = (r.u32()?, r.u32()?);
            if a as usize >= nv || b as usize >= nv {
                return None;
            }
            edges.push([a, b]);
        }
        let bounds = Aabb::of(&positions);
        lods.push(MeshData {
            positions,
            normals,
            indices,
            edges,
            bounds,
        });
    }
    if !r.0.is_empty() {
        return None;
    }
    Some(Cached::Geometry(PartGeometry {
        lods,
        origin: MeshOrigin::Step {
            stats,
            orientation,
            before_simplify,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part3d::mesh::box_mesh;

    fn sample() -> PartGeometry {
        let mut g = box_mesh([5.0, 6.0, 7.0]);
        let coarse = g.lods[0].clone();
        g.lods.push(coarse.bounding_box());
        g.origin = MeshOrigin::Step {
            stats: StepStats {
                solids: 1,
                placements: 2,
                faces: 6,
                approximated_faces: 1,
                skipped_faces: 0,
                raw_triangles: 99,
            },
            orientation: Orientation::Guessed,
            before_simplify: 77,
        };
        g
    }

    #[test]
    fn geometry_with_levels_and_edges_round_trips_exactly() {
        let g = sample();
        let bytes = encode(Some(&g), None);
        assert_eq!(decode(&bytes), Some(Cached::Geometry(g)));
    }

    #[test]
    fn damage_and_old_versions_are_misses() {
        let bytes = encode(Some(&sample()), None);
        let mut flipped = bytes.clone();
        flipped[40] ^= 1;
        assert_eq!(decode(&flipped), None);
        assert_eq!(decode(&bytes[..bytes.len() - 9]), None);
        assert_eq!(decode(&[]), None);
        let mut old = bytes.clone();
        old[8] = 0; // algorithm version
        let n = old.len() - 8;
        let sum = fnv(&old[..n]);
        old.truncate(n);
        old.extend_from_slice(&sum.to_le_bytes());
        assert_eq!(decode(&old), None);
    }

    #[test]
    fn a_header_that_claims_gigabytes_is_rejected_not_allocated() {
        let mut bytes = encode(Some(&sample()), None);
        // Vertex count of the first level: after magic(4) format(4)
        // algorithm(4) kind(1) orientation(1) stats(28) levels(4) = 46.
        bytes[46..50].copy_from_slice(&u32::MAX.to_le_bytes());
        let n = bytes.len() - 8;
        let sum = fnv(&bytes[..n]);
        bytes.truncate(n);
        bytes.extend_from_slice(&sum.to_le_bytes());
        assert_eq!(decode(&bytes), None);
    }

    #[test]
    fn failures_are_remembered() {
        for r in [Fallback::TooComplex, Fallback::TimedOut] {
            let bytes = encode(None, Some(r));
            assert_eq!(decode(&bytes), Some(Cached::Failed(r)));
        }
    }

    #[test]
    fn keys_change_with_content_quality_and_size() {
        let a = cache_key("ab12", Some([10.0, 20.0, 30.0]), Quality::Normal);
        assert!(a.starts_with(&format!("ab12-a{ALGORITHM_VERSION}-n")));
        assert_ne!(
            a,
            cache_key("ab13", Some([10.0, 20.0, 30.0]), Quality::Normal)
        );
        assert_ne!(
            a,
            cache_key("ab12", Some([10.0, 20.0, 30.0]), Quality::Draft)
        );
        assert_ne!(
            a,
            cache_key("ab12", Some([10.0, 20.5, 30.0]), Quality::Normal)
        );
        assert_ne!(a, cache_key("ab12", None, Quality::Normal));
        // A hostile id cannot leave the folder.
        let k = cache_key("../../x", None, Quality::Normal);
        assert!(!k.contains('/') && !k.contains('.'));
    }

    #[test]
    fn prune_removes_stale_files_then_the_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let c = MeshCache::at(dir.path());
        let (a, b) = (
            format!("aaaa-a{ALGORITHM_VERSION}-n00000000"),
            format!("bbbb-a{ALGORITHM_VERSION}-n00000000"),
        );
        c.put(&a, &sample()).unwrap();
        c.put(&b, &sample()).unwrap();
        fs::write(dir.path().join("old-a0-n00000000.kmsh"), b"x").unwrap();
        assert_eq!(c.prune(u64::MAX), 1);
        assert!(c.get(&a).is_some());
        assert_eq!(c.prune(0), 2);
        assert!(c.get(&a).is_none());
    }
}
