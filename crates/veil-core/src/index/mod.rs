//! Entry model, atomic index persistence, and statistics (Spec §4.3, §4.4).

mod byte_array;
mod document;
mod entry;
mod slots;

pub use document::{CURRENT_INDEX_VERSION, IndexDocument, IndexFormatError, Statistics};
pub use entry::{Entry, EntryId};
pub use slots::{generations, read, write};
