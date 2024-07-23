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

// #[cfg(all(test, target_arch = "aarch64"))]
// pub mod trap;

#[cfg(target_arch = "aarch64")]
mod trap;

#[cfg(target_arch = "aarch64")]
pub use trap::*;

#[cfg(test)]
pub mod tests {

    use core::arch::{asm, global_asm};
    #[cfg(target_arch = "riscv64")]
    use riscv::register::{stvec, utvec::TrapMode};
    use sel4_common::{
        arch::shutdown,
        println,
        sel4_config::{asidHighBits, asidLowBits},
        structures::exception_t,
        utils::ptr_to_mut,
        BIT, MASK,
    };
    use sel4_cspace::arch::cap_t;
    global_asm!(include_str!("entry.asm"));

    #[cfg(target_arch = "aarch64")]
    pub use crate::trap::*;
    use crate::{
        asid_pool_t, delete_asid, find_vspace_for_asid, paddr_to_pptr, set_asid_pool_by_index, PTE,
    };

    #[cfg(target_arch = "aarch64")]
    use crate::{asid_map_t, find_map_for_asid};

    #[no_mangle]
    pub fn test_runner(tests: &[&dyn Fn()]) {
        println!("Running {} tests\n", tests.len());
        for test in tests {
            test();
        }
        println!("All Test Cases(count: {}) passed!", tests.len());
        shutdown();
    }

    #[cfg(target_arch = "aarch64")]
    #[test_case]
    pub fn asid_relevant_tests() {
        println!(">>>>>>>>>>>> Entering asid_relevant_tests...");

        let asid: usize = 0x1234;
        let vspace_root: usize = paddr_to_pptr(0x987654321);

        let mut pool = asid_pool_t([asid_map_t::new_none(); BIT!(asidLowBits)]);
        #[cfg(target_arch = "riscv64")]
        let mut pool = asid_pool_t {
            array: [0 as *mut PTE; BIT!(asidLowBits)],
        };
        let asid_cap = cap_t::new_asid_pool_cap(asid, pool.as_ptr() as usize);
        pool[asid & MASK!(asidLowBits)] = asid_map_t::new_vspace(vspace_root);
        set_asid_pool_by_index(asid >> asidLowBits, pool.as_ptr() as usize);

        // test find_map_for_asid
        let find_ret = find_map_for_asid(asid_cap.get_asid_base());
        if let Some(asid_map) = find_ret {
            assert_eq!(asid_map.get_vspace_root(), vspace_root);
            log::info!("Successfully find right vspace_root");
        } else {
            assert!(false);
        }

        // test find_vspace_for_asid
        let vroot = find_vspace_for_asid(asid);
        assert_eq!(vroot.status, exception_t::EXCEPTION_NONE);
        assert_eq!(vroot.vspace_root.unwrap() as usize, vspace_root);

        let invalid_vroot = find_vspace_for_asid(0);
        assert_eq!(invalid_vroot.status, exception_t::EXCEPTION_LOOKUP_FAULT);

        println!("Test asid_relevant_tests passed!<<<<<<<<<<<<\n");
    }

    #[cfg(target_arch = "aarch64")]
    #[test_case]
    fn boot_relevant_tests() {
        use crate::rust_map_kernel_window;

        println!(">>>>>>>>>>>> Entering boot_relevant_tests...");
        rust_map_kernel_window();
        println!("Test boot_relevant_tests passed!<<<<<<<<<<<<\n");
    }

    // #[cfg(target_arch="aarch64")]
    // #[test_case]
    // fn _relevant_tests(){
    //     println!(">>>>>>>>>>>> Entering boot_relevant_tests...");

    //     println!("Test boot_relevant_tests passed!<<<<<<<<<<<<\n");
    // }

    #[cfg(target_arch = "riscv64")]
    #[test_case]
    pub fn asid_relevant_tests() {
        println!(">>>>>>>>>>>> Entering asid_relevant_tests...");
        let asid: usize = 0x1234;
        let vspace_root: usize = paddr_to_pptr(0x987654321);

        #[cfg(target_arch = "riscv64")]
        let mut pool = asid_pool_t {
            array: [0 as *mut PTE; BIT!(asidLowBits)],
        };
        let asid_cap = cap_t::new_asid_pool_cap(asid, pool.array.as_ptr() as usize);
        pool.array[asid & MASK!(asidLowBits)] = vspace_root as *mut PTE;
        set_asid_pool_by_index(asid >> asidLowBits, pool.array.as_ptr() as usize);

        // test find_vspace_for_asid
        let vroot = find_vspace_for_asid(asid);
        assert_eq!(vroot.status, exception_t::EXCEPTION_NONE);
        assert_eq!(vroot.vspace_root.unwrap() as usize, vspace_root);

        let invalid_vroot = find_vspace_for_asid(0);
        assert_eq!(invalid_vroot.status, exception_t::EXCEPTION_LOOKUP_FAULT);

        // delete_asid(asid, vspace_root as *mut PTE, &asid_cap);
        println!("Test asid_relevant_tests passed!<<<<<<<<<<<<\n");
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
