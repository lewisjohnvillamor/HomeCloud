//! Image derivatives.
//!
//! Everything here treats its input as hostile: a photo library is full
//! of files other people produced, and an image decoder is one of the
//! easiest places to spend all of a server's memory. Decoding therefore
//! happens under explicit limits, by content rather than by file name,
//! and never on an async executor.

// Application crates have no need for `unsafe`; an exception requires an ADR.
#![forbid(unsafe_code)]

pub mod thumbnail;

pub use thumbnail::{
    generate_thumbnail, MediaError, ThumbnailSize, MAX_SOURCE_BYTES, MAX_SOURCE_PIXELS,
};
