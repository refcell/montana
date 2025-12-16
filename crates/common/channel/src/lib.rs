//! Channel crate.

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod source;

pub use source::{BatchSource, L2BlockData, RawTransaction, SourceError};
