//! Entry file read and write (Spec §4.5).

mod entry_file;

pub use entry_file::{ENTRIES_DIR, EntryWriter, entry_path, exists, open_for_read, remove};
