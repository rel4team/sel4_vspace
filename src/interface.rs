// use crate::{common::{sel4_config::*, structures::exception_t, utils::{convert_to_mut_type_ref, pageBitsForSize}, fault::*}, BIT, ROUND_DOWN};
use sel4_cspace::interface::{cap_t, CapTag};
use core::intrinsics::unlikely;
use sel4_common::{BIT, ROUND_DOWN};
use sel4_common::fault::lookup_fault_t;
use sel4_common::sel4_config::{KERNEL_ELF_BASE, KERNEL_ELF_PADDR_BASE, PADDR_BASE, PPTR_BASE, PPTR_BASE_OFFSET, PPTR_TOP, PT_INDEX_BITS, seL4_PageBits};
use sel4_common::structures::exception_t;
use sel4_common::utils::{convert_to_mut_type_ref, pageBitsForSize};
use super::pte::pte_t;
use super::utils::{RISCV_GET_PT_INDEX, RISCV_GET_LVL_PGSIZE, RISCV_GET_LVL_PGSIZE_BITS, kpptr_to_paddr};

use super::{satp::{setVSpaceRoot, sfence}, asid::{find_vspace_for_asid, asid_t}, utils::pptr_to_paddr, structures::{vptr_t, pptr_t}};

///页表采用`SV39`，该变量是内核使用的页表的根页表（一级页表）
#[no_mangle]
#[link_section = ".page_table"]
pub static mut kernel_root_pageTable: [pte_t; BIT!(PT_INDEX_BITS)] =
    [pte_t { words: [0] }; BIT!(PT_INDEX_BITS)];

///内核使用的二级页表
#[no_mangle]
#[link_section = ".page_table"]
pub static mut kernel_image_level2_pt: [pte_t; BIT!(PT_INDEX_BITS)] =
    [pte_t { words: [0] }; BIT!(PT_INDEX_BITS)];

/// 构建`reL4`的内核页表,主要完成了`PSpace`和`KERNEL ELF`两段虚拟地址空间的映射 
/// 
/// 其中`PSpace`是对整个物理地址空间的线性映射，`KERNEL ELF`是对内核代码的再一次映射
/// 
/// reL4的地址空间如下图所示（来源：seL4/include/arch/riscv/arch/64/mode/hardware.h），左侧为虚拟地址空间，右侧为物理地址空间：
/// 
/// ```
///                   +-----------------------------+ 2^64
///                   |        Kernel Devices       |
///                -> +-------------------KDEV_BASE-+ 2^64 - 1GiB
///                |  |         Kernel ELF          |
///            ----|  +-------------KERNEL_ELF_BASE-+ --+ 2^64 - 2GiB + (KERNEL_ELF_PADDR_BASE % 1GiB)
///            |   |  |                             |
///            |   -> +-----------------------------+ --+ 2^64 - 2GiB = (KERNEL_ELF_BASE % 1GiB)
/// Shared 1GiB|      |                             |   |
/// table entry|      |           PSpace            |   |
///            |      |  (direct kernel mappings)   |   +----+
///            ------>|                             |   |    |
///                   |                             |   |    |
///                   +-------------------PPTR_BASE-+ --+ 2^64 - 2^b
///                   |                             |        |         +-------------------------+
///                   |                             |        |         |                         |
///                   |                             |        |         |                         |
///                   |          Invalid            |        |         |                         |
///                   |                             |        |         |           not           |
///                   |                             |        |         |         kernel          |
///                   |                             |        |         |       addressable       |
///                   +--------------------USER_TOP-+  2^c   |         |                         |
///                   |                             |        |         |                         |
///                   |                             |        |         |                         |
///                   |                             |        |      +- --------------------------+  PADDR_TOP =
///                   |                             |        |      |  |                         |    PPTR_TOP - PPTR_BASE
///                   |                             |        |      |  |                         |
///                   |                             |        |      |  |                         |
///                   |            User             |        |      |  |                         |
///                   |                             |        |      |  |                         |
///                   |                             |        +------+  +-------------------------+  KDEV_BASE - KERNEL_ELF_BASE + PADDR_LOAD
///                   |                             |     kernel    |  |        Kernel ELF       |
///                   |                             |   addressable |  +-------------------------+  KERNEL_ELF_PADDR_BASE
///                   |                             |               |  |                         |
///                   |                             |               |  |                         |
///                   +-----------------------------+  0            +- +-------------------------+  0 PADDR_BASE
///
///                      virtual address space                          physical address space
/// ```
/// 
#[no_mangle]
pub fn rust_map_kernel_window() {
    // 内核地址空间中直接映射物理地址空间的起始地址
    let mut pptr = PPTR_BASE;

    // 物理地址空间的起始地址
    let mut paddr = PADDR_BASE;

    // 物理地址到内核地址空间的直接映射，用`1GB`大页的方式映射
    while pptr < PPTR_TOP {
        unsafe {
            kernel_root_pageTable[RISCV_GET_PT_INDEX(pptr, 0)] = pte_t::pte_next(paddr, true);
        }
        pptr += RISCV_GET_LVL_PGSIZE(0);
        paddr += RISCV_GET_LVL_PGSIZE(0);
    }
    pptr = ROUND_DOWN!(KERNEL_ELF_BASE, RISCV_GET_LVL_PGSIZE_BITS(0));
    paddr = ROUND_DOWN!(KERNEL_ELF_PADDR_BASE, RISCV_GET_LVL_PGSIZE_BITS(0));

    // 将根页表`KERNEL_ELF_PADDR_BASE`和`KERNEL_ELF_BASE`处的页表项改为使用`kernel_image_level2_pt`映射
    unsafe {
        kernel_root_pageTable[RISCV_GET_PT_INDEX(KERNEL_ELF_PADDR_BASE + PPTR_BASE_OFFSET, 0)] =
            pte_t::pte_next(
                kpptr_to_paddr(kernel_image_level2_pt.as_ptr() as usize),
                false,
            );
        kernel_root_pageTable[RISCV_GET_PT_INDEX(pptr, 0)] = pte_t::pte_next(
            kpptr_to_paddr(kernel_image_level2_pt.as_ptr() as usize),
            false,
        );
    }

    let mut index = 0;
    // 做了 `0xFFFF_FFFF_8400_0000(KERNEL_ELF_BASE)~0xFFFF_FFFF_C4000_0000(KDEV_BASE)`到`0x8400_0000~0xC400_0000`的地址映射。
    while pptr < PPTR_TOP + RISCV_GET_LVL_PGSIZE(0) {
        unsafe {
            kernel_image_level2_pt[index] = pte_t::pte_next(paddr, true);
        }
        pptr += RISCV_GET_LVL_PGSIZE(1);
        paddr += RISCV_GET_LVL_PGSIZE(1);
        index += 1;
    }
}

/// 激活内核页表，将`satp`的值设置为内核页表根页表地址
/// 
/// Activate kernel vspace, assign kernel root page table's value to satp.
#[inline]
pub fn activate_kernel_vspace() {
    unsafe {
        setVSpaceRoot(kpptr_to_paddr(kernel_root_pageTable.as_ptr() as usize), 0);
    }
}

/// 拷贝内核页表到新给出的页表基地址`Lvl1pt`，当创建一个进程的时候，会拷贝一个新的页表给新创建的进程，新的页表中包含内核地址空间
///
/// Copy the whole kernel page table into a new page table. 
/// when create a new process, a new page table will be alloced to the new process.
#[no_mangle]
pub fn copyGlobalMappings(Lvl1pt: usize) {
    let mut i: usize = RISCV_GET_PT_INDEX(0x80000000, 0);
    while i < BIT!(PT_INDEX_BITS) {
        unsafe {
            let newLvl1pt = (Lvl1pt + i * 8) as *mut usize;
            *newLvl1pt = kernel_root_pageTable[i].words[0];
            i += 1;
        }
    }
}

///根据给定的`vspace_root`设置相应的页表，会检查`vspace_root`是否合法，如果不合法默认设置为内核页表
/// 
/// Use page table in vspace_root to set the satp register.
pub fn set_vm_root(vspace_root: &cap_t) -> Result<(), lookup_fault_t> {
    if vspace_root.get_cap_type() != CapTag::CapPageTableCap {
        unsafe {
            setVSpaceRoot(kpptr_to_paddr(kernel_root_pageTable.as_ptr() as usize), 0);
            return Ok(());
        }
    }
    let lvl1pt = convert_to_mut_type_ref::<pte_t>(vspace_root.get_pt_base_ptr());
    let asid = vspace_root.get_pt_mapped_asid();
    let find_ret = find_vspace_for_asid(asid);
    let mut ret = Ok(());
    if unlikely(
        find_ret.status != exception_t::EXCEPTION_NONE || find_ret.vspace_root.is_none() || find_ret.vspace_root.unwrap() != lvl1pt,
    ) {
        unsafe {
            if let Some(lookup_fault) = find_ret.lookup_fault {
                ret = Err(lookup_fault);
            }
            setVSpaceRoot(kpptr_to_paddr(kernel_root_pageTable.as_ptr() as usize), 0);
        }
    }
    setVSpaceRoot(pptr_to_paddr(lvl1pt as *mut pte_t as usize), asid);
    ret
}

/// 清除页表中对应的页表项。
/// 
/// `page_size`:在页表中寻找`vptr`对应的`pte`剩余的位数
/// 
/// `vptr`:该页表项对应的应用程序访问的虚拟地址（mapped_address）
/// 
/// `pptr`:分配的页面对应的虚拟地址(frame_base_ptr)
#[no_mangle]
pub fn unmapPage(page_size: usize, asid: asid_t, vptr: vptr_t, pptr: pptr_t) -> Result<(), lookup_fault_t> {
    let find_ret = find_vspace_for_asid(asid);
    if find_ret.status != exception_t::EXCEPTION_NONE {
        return Err(find_ret.lookup_fault.unwrap());
    }

    let lu_ret = unsafe {(*find_ret.vspace_root.unwrap()).lookup_pt_slot(vptr)};

    if lu_ret.ptBitsLeft != pageBitsForSize(page_size) {
        return Ok(());
    }

    let slot = unsafe {&(*lu_ret.ptSlot)};

    if slot.get_vaild() == 0 || slot.is_pte_table() || slot.get_ppn() << seL4_PageBits != pptr_to_paddr(pptr) {
        return Ok(());
    }

    unsafe {
        let slot = lu_ret.ptSlot as *mut usize;
        *slot = 0;
        sfence();
    }
    Ok(())
}