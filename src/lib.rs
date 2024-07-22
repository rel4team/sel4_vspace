#![no_std]
#![allow(non_snake_case)]
#![allow(internal_features)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![feature(decl_macro)]
#![feature(core_intrinsics)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_main]

mod arch;
mod asid;
mod boot;
mod pagetable;
// mod pte;
mod structures;
mod utils;

#[cfg(target_arch = "aarch64")]
pub use arch::aarch64::*;
#[cfg(target_arch = "riscv64")]
pub use arch::riscv64::*;
pub use arch::unmapPage;
pub use asid::*;
pub use boot::*;
pub use pagetable::PageTable;
// pub use pte::PTE;
pub use structures::*;
pub use utils::checkVPAlignment;
// pub use riscv::*;

#[cfg(all(test, target_arch = "aarch64"))]
pub mod trap;

#[cfg(test)]
pub mod tests {

    use core::arch::asm;
    #[cfg(target_arch = "riscv64")]
    use riscv::register::stvec;
    use sel4_common::{arch::shutdown, println};

    #[cfg(target_arch = "aarch64")]
    pub use crate::trap::*;

    #[no_mangle]
    pub fn test_runner(tests: &[&dyn Fn()]) {
        println!("Running {} tests\n", tests.len());
        for test in tests {
            test();
        }
        println!("All Test Cases(count: {}) passed!", tests.len());
        shutdown();
    }

    #[panic_handler]
    fn panic(info: &core::panic::PanicInfo) -> ! {
        println!("{}", info);
        shutdown()
    }

    #[no_mangle]
    pub fn call_test_main() {
        #[cfg(target_arch = "riscv64")]
        {
            extern "C" {
                fn trap_entry();
            }
            unsafe {
                stvec::write(trap_entry as usize, stvec::TrapMode::Direct);
            }
        }
        #[cfg(target_arch = "aarch64")]
        crate::trap::init();
        crate::test_main();
    }

    #[no_mangle]
    pub fn c_handle_syscall() {
        #[cfg(target_arch = "riscv64")]
        unsafe {
            asm!("sret");
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            asm!("eret");
        }
    }
}
