#![allow(unused)]
//#![feature(min_specialization)]

extern crate ofx_sys;
#[macro_use]
extern crate log;

use std::collections::HashMap;
use std::ffi::CString;
use std::fmt;
use std::fmt::Display;
use std::marker::PhantomData;

#[macro_use]
mod result;

#[macro_use]
mod suites;
#[macro_use]
mod util;
mod action;
mod enums;
mod handle;
mod plugin;
mod property;
mod types;
#[macro_use]
mod registry;
mod image;
pub use action::*;
pub use enums::*;
pub use handle::*;
pub use image::*;
pub use plugin::*;
pub use property::*;
use registry::*;
pub use result::*;
pub use types::*;
pub use util::*;

pub use ofx_sys::{OfxHost, OfxPlugin, OfxPropertySetHandle};
pub use registry::{
    Registry, init_registry, main_entry_for_plugin, set_host_for_plugin, with_registry,
};

#[macro_export]
macro_rules! register_modules {
	( $ ($module:ident), *) => {
		fn register_plugins(registry: &mut ofx::Registry) {
			$(register_plugin!(registry, $module);
			)*
		}

		build_plugin_registry!(register_plugins);
	};
}
