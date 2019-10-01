// Copyright(c) 2019 Pierre Krieger

#![feature(never_type)]
#![warn(missing_docs)]
#![deny(unsafe_code)]
#![deny(intra_doc_link_resolution_failure)]
#![allow(dead_code)] // TODO: temporary during development

// TODO: futures and std::error::Error don't work in #![no_std] :-/
// #![no_std]

extern crate alloc;

pub mod module;
pub mod scheduler;
pub mod signature;
pub mod system;
