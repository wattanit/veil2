//! Pack files, extent allocation, space accounting, and compaction (Spec §4.5).
//!
//! Compaction and reconciliation are Phase 4; this module carries pack write
//! and read, which is what Phase 1 needs to prove damage locality (S-4).

mod pack;

pub use pack::{
    DEFAULT_PACK_CAP, PACKS_DIR, PackSink, PackSource, damaged_pack, entries_in_pack,
    existing_pack_ids, pack_path, total_pack_bytes,
};
