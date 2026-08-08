//! Recovering the space that deleting and replacing left behind (Spec §4.5;
//! FR-23, FR-24, FR-25).
//!
//! One pack at a time, and that is the whole design. The live extents of a
//! single pack are copied into a new one, the index adopts them in one
//! generation step, and only then is the old pack removed. Working space is
//! therefore about one pack whatever the vault's size (FR-25), and an
//! interruption costs at most the pack in flight (FR-24).
//!
//! **Never automatic** (FR-23). Nothing in this module runs unless a caller
//! asks, and no caller may schedule it, time it, or condition it on a
//! threshold. The figures of FR-8 are what the decision rests on, and the
//! person reading them is the only one who makes it.

use std::io::{Read, Seek, SeekFrom, Write};

use crate::error::{Error, Result};
use crate::index::{EntryId, Extent};
use crate::store::{PackSink, damaged_pack, entries_in_pack, existing_pack_ids, pack_path};

use super::{Cancel, Progress, ProgressReport, Unit, Vault};

/// Bytes moved per read, and the interval at which cancellation is noticed.
const COPY_CHUNK: usize = 64 * 1024;

/// What reclaiming space recovered (FR-8, FR-32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Reclaimed {
    /// How many packs were rewritten and removed.
    pub packs_rewritten: usize,
    /// Bytes handed back to the filesystem.
    pub bytes_recovered: u64,
    /// Whether every pack worth reclaiming was reached. `false` when the
    /// caller cancelled; what was already reclaimed stays reclaimed (FR-24).
    pub complete: bool,
}

/// One pack, and what of it is still live.
struct Candidate {
    pack_id: u32,
    /// Bytes the file occupies.
    size: u64,
    /// Bytes of it the index still references.
    live: u64,
}

impl Candidate {
    fn garbage(&self) -> u64 {
        self.size.saturating_sub(self.live)
    }

    /// Garbage as a share of the pack. The selection order §4.5 fixes.
    fn ratio(&self) -> f64 {
        if self.size == 0 {
            0.0
        } else {
            self.garbage() as f64 / self.size as f64
        }
    }
}

impl Vault {
    /// Reclaims the space deleted and replaced content is still occupying
    /// (FR-23, FR-24, FR-25).
    ///
    /// Packs are taken in order of how much of them is garbage, highest share
    /// first. A pack holding no garbage is not rewritten — copying a gigabyte
    /// to recover nothing is pure cost. Every pack holding *any* garbage is,
    /// because Design §8.4 puts the reclaimable figure in the control the user
    /// presses, and an operation that recovers less than it said has made that
    /// number untrue.
    ///
    /// Stored bytes are copied exactly as they are: no decryption, no
    /// re-encryption, no new nonce, no new entry identifier. The identifier is
    /// bound into both the key wrapping and the content's associated data
    /// (§3.2, §3.3), so reissuing one would be a cryptographic fault dressed as
    /// housekeeping. Only which pack an extent lives in changes.
    ///
    /// # Errors
    ///
    /// [`Error::ReadOnly`], [`Error::ChangedOnDisk`], [`Error::Io`], or
    /// [`Error::Corrupt`] naming a pack whose bytes cannot be read in full —
    /// which is refused rather than rewritten, so damage is never copied
    /// forward into a fresh pack with the evidence deleted behind it.
    pub fn compact(&mut self, progress: &mut impl Progress, cancel: &Cancel) -> Result<Reclaimed> {
        self.begin_write()?;
        // Before anything is written, and over *every* pack rather than only
        // the ones with garbage in them. A pack that has been truncated looks
        // like a pack with no garbage — its live extents claim more than the
        // file holds, so the arithmetic says there is nothing to recover — and
        // damage that hides its own detection is the worst kind to have.
        self.refuse_unreadable_packs()?;

        let mut candidates = self.candidates()?;
        // Highest share of garbage first (§4.5). Equal shares fall back to the
        // identifier so the order is stable and a failure reproduces.
        candidates.sort_by(|a, b| {
            b.ratio()
                .partial_cmp(&a.ratio())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.pack_id.cmp(&b.pack_id))
        });

        let total: u64 = candidates.iter().map(Candidate::garbage).sum();
        let mut done = Reclaimed {
            complete: true,
            ..Reclaimed::default()
        };
        progress.report(ProgressReport {
            unit: Unit::Bytes,
            done: 0,
            total: Some(total),
        });

        for candidate in &candidates {
            // Between packs as well as within one: each pack is its own
            // transaction, so stopping here costs nothing at all (FR-24).
            if cancel.is_cancelled() {
                done.complete = false;
                break;
            }

            match self.rewrite(candidate, cancel) {
                Ok(recovered) => {
                    done.packs_rewritten += 1;
                    done.bytes_recovered += recovered;
                }
                Err(Error::Cancelled { .. }) => {
                    done.complete = false;
                    break;
                }
                Err(e) => return Err(e),
            }

            progress.report(ProgressReport {
                unit: Unit::Bytes,
                done: done.bytes_recovered,
                total: Some(total),
            });
        }

        Ok(done)
    }

    /// Every pack with garbage in it, with the figures the selection needs.
    fn candidates(&self) -> Result<Vec<Candidate>> {
        let mut candidates = Vec::new();
        for pack_id in existing_pack_ids(&self.dir)? {
            let path = pack_path(&self.dir, pack_id);
            let Ok(metadata) = std::fs::metadata(&path) else {
                continue;
            };
            let live: u64 = self
                .document
                .entries
                .iter()
                .flat_map(|e| e.extents.iter())
                .filter(|x| x.pack_id == pack_id)
                .map(|x| x.length)
                .sum();
            let candidate = Candidate {
                pack_id,
                size: metadata.len(),
                live,
            };
            if candidate.garbage() > 0 {
                candidates.push(candidate);
            }
        }
        Ok(candidates)
    }

    /// Copies one pack's live extents into a new pack, adopts them in one
    /// generation step, and removes the old pack. Returns the bytes recovered.
    ///
    /// The order is the one FR-12 fixes for every other write in this crate:
    /// the new bytes are durable, then the index names them, then what the
    /// index let go of is removed. A crash before the commit leaves the new
    /// pack unreferenced; a crash after it leaves the old one unreferenced.
    /// Either way the leftover is residue that reconciliation clears at the
    /// next open (FR-32), and nothing live is ever unreachable.
    fn rewrite(&mut self, candidate: &Candidate, cancel: &Cancel) -> Result<u64> {
        let pack_id = candidate.pack_id;

        // Which entry, which of its extents, and where it moves to.
        let moving: Vec<(EntryId, usize)> = self
            .document
            .entries
            .iter()
            .flat_map(|e| {
                e.extents
                    .iter()
                    .enumerate()
                    .filter(|(_, x)| x.pack_id == pack_id)
                    .map(move |(at, _)| (e.id, at))
            })
            .collect();

        let mut relocated: Vec<(EntryId, usize, Extent)> = Vec::with_capacity(moving.len());

        if !moving.is_empty() {
            let mut sink = PackSink::open_fresh(&self.dir, self.pack_cap)?;
            let mut source = std::fs::File::open(pack_path(&self.dir, pack_id))?;

            let outcome = (|| -> Result<()> {
                for (id, at) in &moving {
                    if cancel.is_cancelled() {
                        return Err(Error::Cancelled { rolled_back: true });
                    }
                    let from = self.extent_at(*id, *at);
                    // Each entry's run is its own extent; without this, two
                    // entries landing next to each other would merge into one.
                    sink.seal_extent();
                    let before = sink.extents().len();
                    copy_extent(&mut source, &mut sink, from, cancel)?;
                    let Some(to) = sink.extents().get(before).copied() else {
                        return Err(Error::Io {
                            kind: std::io::ErrorKind::UnexpectedEof,
                        });
                    };
                    relocated.push((*id, *at, to));
                }
                Ok(())
            })();

            if let Err(e) = outcome {
                // Nothing has been committed, so the new pack is entirely this
                // attempt's and can go. If the rollback itself fails, the
                // reason we stopped is still the one the caller needs — and
                // what it leaves behind is unreferenced, which reconciliation
                // clears at the next open (FR-32).
                let _ = sink.rollback();
                return Err(e);
            }

            // Durable before the index may name it (FR-12).
            sink.finish()?;
        }

        for (id, at, to) in relocated {
            if let Some(entry) = self.document.entries.iter_mut().find(|e| e.id == id)
                && let Some(extent) = entry.extents.get_mut(at)
            {
                *extent = to;
            }
        }

        // Removing the pack recovers exactly its garbage: the live part of it
        // was just written again elsewhere.
        let recovered = candidate.garbage();
        self.document.statistics.physical_bytes = self
            .document
            .statistics
            .physical_bytes
            .saturating_sub(recovered);
        self.document.statistics.reclaimable_bytes = self
            .document
            .statistics
            .reclaimable_bytes
            .saturating_sub(recovered);
        self.commit()?;

        crate::store::remove_pack(&self.dir, pack_id)?;
        Ok(recovered)
    }

    /// Refuses the whole operation if any referenced pack's bytes are not all
    /// there.
    ///
    /// Missing or short, the live extents cannot be copied in full, and copying
    /// what is there would produce an entry whose recorded length no longer
    /// matches its stored bytes — and would delete the original that proved
    /// what happened. Refusing costs nothing and keeps the damage where `check`
    /// can attribute it (FR-33, S-4).
    ///
    /// All of them rather than only the ones being rewritten, and before any
    /// byte is written rather than as each is reached. Reclaiming space on a
    /// damaged vault is not a request to work around the damage.
    ///
    /// A pack that is complete but whose *contents* were altered is not
    /// refused, and deliberately so: telling authentic bytes from tampered ones
    /// means decrypting them, which this operation does not do. Copying them
    /// forward loses nothing — the same bytes stay damaged in the new pack, and
    /// `check` still names the same entries.
    fn refuse_unreadable_packs(&self) -> Result<()> {
        let mut pack_ids: Vec<u32> = self
            .document
            .entries
            .iter()
            .flat_map(|e| e.extents.iter())
            .map(|x| x.pack_id)
            .collect();
        pack_ids.sort_unstable();
        pack_ids.dedup();

        for pack_id in pack_ids {
            let length = std::fs::metadata(pack_path(&self.dir, pack_id))
                .map(|m| m.len())
                .unwrap_or(0);
            let short = self
                .document
                .entries
                .iter()
                .flat_map(|e| e.extents.iter())
                .filter(|x| x.pack_id == pack_id)
                .any(|x| x.offset.saturating_add(x.length) > length);

            if short {
                return Err(damaged_pack(
                    pack_id,
                    entries_in_pack(&self.document.entries, pack_id),
                ));
            }
        }
        Ok(())
    }

    fn extent_at(&self, id: EntryId, at: usize) -> Extent {
        self.document
            .entries
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.extents.get(at).copied())
            .unwrap_or(Extent {
                pack_id: 0,
                offset: 0,
                length: 0,
            })
    }
}

/// Moves one extent's bytes, in chunks, so a cancellation is noticed part-way
/// through a large one rather than after it.
fn copy_extent(
    source: &mut std::fs::File,
    sink: &mut PackSink<'_>,
    extent: Extent,
    cancel: &Cancel,
) -> Result<()> {
    source.seek(SeekFrom::Start(extent.offset))?;
    let mut remaining = extent.length;
    let mut buffer = vec![0u8; COPY_CHUNK];

    while remaining > 0 {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled { rolled_back: true });
        }
        let want = usize::try_from(remaining)
            .unwrap_or(COPY_CHUNK)
            .min(COPY_CHUNK);
        source.read_exact(&mut buffer[..want])?;
        sink.write_all(&buffer[..want])?;
        remaining -= want as u64;
    }
    Ok(())
}
