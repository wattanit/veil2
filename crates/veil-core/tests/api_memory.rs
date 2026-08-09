//! Phase 2 test case T2.18 — peak memory does not scale with entry size
//! through the public API (FR-20, S-1, A-2).
//!
//! T1.11 measured the cryptographic layer in isolation. This measures the whole
//! path a frontend drives — pack sink, extents, index bookkeeping and all —
//! because S-1 is a promise about the product, not about one module.
//!
//! This binary installs a counting global allocator, so it holds exactly one
//! test. A second test running concurrently would attribute its allocations to
//! this one.
//!
//! *Scope note:* this establishes the shape of the curve at modest sizes. The
//! claim at C-2's 64 GiB maximum is a scale test scheduled in Phase 5 (Plan
//! P5.5), and this case is not evidence for it.

// A counting allocator cannot be written in safe Rust. The library forbids
// unsafe outright; this is a measurement harness, not library code.
#![allow(unsafe_code)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

use veil_core::crypto::{CHUNK_LEN, KdfParams, Password};
use veil_core::vault::Vault;
use veil_core::{Cancel, NoProgress};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(live, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// A source that yields `len` bytes without ever holding them.
///
/// A `Vec` of the plaintext would dominate the measurement and turn this into a
/// test of the fixture.
struct Generated {
    remaining: usize,
}

impl Read for Generated {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().min(self.remaining);
        for (i, slot) in buf[..n].iter_mut().enumerate() {
            *slot = (i % 251) as u8;
        }
        self.remaining -= n;
        Ok(n)
    }
}

/// A destination that counts bytes and keeps none.
struct Counted(u64);

impl Write for Counted {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn peak_for(len: usize) -> usize {
    let dir = std::env::temp_dir().join(format!("veil2-t2-18-{len}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let password = Password::new("a sufficiently long password".to_owned());
    let mut vault = Vault::create(&dir, &password, KdfParams::for_tests()).unwrap();

    PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
    let before = PEAK.load(Ordering::Relaxed);

    let id = vault
        .add(
            "measured.bin",
            "d",
            &mut Generated { remaining: len },
            &mut NoProgress,
            &Cancel::new(),
        )
        .unwrap();

    let mut counted = Counted(0);
    vault
        .extract(id, &mut counted, &mut NoProgress, &Cancel::new())
        .unwrap();
    assert_eq!(counted.0, len as u64);

    let peak = PEAK.load(Ordering::Relaxed).saturating_sub(before);
    drop(vault);
    let _ = std::fs::remove_dir_all(&dir);
    peak
}

/// T2.18 — peak allocation differs by a bounded constant, not by the ratio of
/// the entry sizes (FR-20, S-1).
#[test]
fn t2_18_peak_memory_does_not_scale_with_entry_size() {
    let small = CHUNK_LEN * 2;
    let large = CHUNK_LEN * 32;
    assert_eq!(large / small, 16, "the fixture sizes must differ sharply");

    // Warm up: first-call allocations belong to neither measurement.
    let _ = peak_for(CHUNK_LEN);

    let peak_small = peak_for(small);
    let peak_large = peak_for(large);

    // Sixteen times the content. If peak memory followed entry size, the second
    // figure would be roughly sixteen times the first. C-2 permits 64 GiB; an
    // implementation that buffers is unusable at the size the product exists
    // for.
    assert!(
        peak_large < peak_small * 2,
        "peak memory followed entry size: {peak_small} bytes for {small}, \
         {peak_large} bytes for {large} — a {}x input gave a {:.1}x peak",
        large / small,
        peak_large as f64 / peak_small as f64
    );

    assert!(
        peak_large < CHUNK_LEN * 8,
        "peak memory is {peak_large} bytes, more than eight chunks"
    );
}
