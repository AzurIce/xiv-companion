//! Vendored third-party code, maintained locally inside this crate.
//!
//! `physis_havok` is derived from physis (https://github.com/AzurIce/physis), MIT license.
//! Upstream keeps `mod havok` private; it is vendored here so the Havok tagfile v3
//! reader (skeleton extraction + spline-compressed animation sampling) can evolve
//! with this project.

pub mod physis_havok;
