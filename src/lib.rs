#![no_std]
#![allow(non_snake_case)]
#![allow(internal_features)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![feature(decl_macro)]
#![feature(core_intrinsics)]
mod arch;
mod asid;
mod boot;
pub mod interface;
mod pagetable;
// mod pte;
mod structures;
mod utils;

#[cfg(target_arch = "aarch64")]
pub use arch::aarch64::*;
#[cfg(target_arch = "riscv64")]
pub use arch::riscv64::*;
pub use asid::*;
pub use boot::*;
pub use interface::unmapPage;
pub use pagetable::PageTable;
// pub use pte::PTE;
pub use structures::*;
pub use utils::checkVPAlignment;
// pub use riscv::*;
