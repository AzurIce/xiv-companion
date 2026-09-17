//! Material data pipeline: bind groups + GPU texture upload, uniform
//! value builders, texture creation helpers, and uniform layouts.

mod bind_groups;
mod params;
mod textures;
mod uniforms;

use super::*;

pub(crate) use bind_groups::*;
pub(crate) use params::*;
pub(crate) use textures::*;
pub(crate) use uniforms::*;
