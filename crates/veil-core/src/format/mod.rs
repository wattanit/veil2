//! On-disk layout, header, serialisation, and version dispatch (Spec §4.1–§4.3).

mod header;
mod unlock;

pub use header::{
    CURRENT_FORMAT_VERSION, HEADER_LEN, HEADER_PREFIX_LEN, Header, HeaderError, MAGIC,
    OLDEST_SUPPORTED_FORMAT_VERSION, SALT_LEN,
};
pub use unlock::unlock;
