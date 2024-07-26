use super::machine::mair_types;
use super::structures::{
    lookupFrame_ret_t, lookupPDSlot_ret_t, lookupPGDSlot_ret_t, lookupPTSlot_ret_t,
    lookupPUDSlot_ret_t,
};
use crate::arch::VAddr;
use crate::vptr_t;
use sel4_common::{
    arch::{
        config::{KERNEL_ELF_BASE_OFFSET, PPTR_BASE_OFFSET},
        vm_rights_t,
    },
    fault::lookup_fault_t,
    ffi_addr,
    sel4_config::*,
    structures::exception_t,
    utils::{convert_to_mut_slice, convert_to_type_ref},
    MASK,
};

pub const KPT_LEVELS: usize = 4;
pub const seL4_VSpaceIndexBits: usize = 9;
pub(self) const PAGE_ADDR_MASK: usize = MASK!(48) & !0xfff;
#[inline]
pub fn GET_PT_INDEX(addr: usize) -> usize {
    (addr >> PT_INDEX_OFFSET) & MASK!(PT_INDEX_BITS)
}
#[inline]
pub fn GET_PD_INDEX(addr: usize) -> usize {
    (addr >> PD_INDEX_OFFSET) & MASK!(PD_INDEX_BITS)
}
#[inline]
pub fn GET_UPUD_INDEX(addr: usize) -> usize {
    (addr >> PUD_INDEX_OFFSET) & MASK!(UPUD_INDEX_BITS)
}
#[inline]
pub fn GET_PUD_INDEX(addr: usize) -> usize {
    (addr >> PUD_INDEX_OFFSET) & MASK!(PUD_INDEX_BITS)
}
#[inline]
pub fn GET_PGD_INDEX(addr: usize) -> usize {
    (addr >> PGD_INDEX_OFFSET) & MASK!(PGD_INDEX_BITS)
}

#[inline]
pub fn GET_KPT_INDEX(addr: usize, n: usize) -> usize {
    ((addr) >> (((PT_INDEX_BITS) * (((KPT_LEVELS) - 1) - (n))) + seL4_PageBits))
        & MASK!(PT_INDEX_BITS)
}

#[inline]
pub fn GET_UPT_INDEX(addr: usize, n: usize) -> usize {
    ((addr) >> (((PT_INDEX_BITS) * (((KPT_LEVELS) - 1) - (n))) + seL4_PageBits))
        & MASK!(PT_INDEX_BITS)
}

#[inline]
pub fn kpptr_to_paddr(x: usize) -> usize {
    x - KERNEL_ELF_BASE_OFFSET
}

///计算以`PPTR_BASE`作为偏移的指针虚拟地址对应的物理地址
#[inline]
pub const fn pptr_to_paddr(x: usize) -> usize {
    x - PPTR_BASE_OFFSET
}

///计算物理地址对应的虚拟地址，以`PPTR_BASE`作为偏移
#[inline]
pub fn paddr_to_pptr(x: usize) -> usize {
    x + PPTR_BASE_OFFSET
}

impl VAddr {
    /// Get the index of the pt(last level, bit 12..20)
    pub(super) const fn pt_index(&self) -> usize {
        (self.0 >> 12) & 0x1ff
    }

    /// Get the index of the pd(third level, bit 21..29)
    pub(super) const fn pd_index(&self) -> usize {
        (self.0 >> 21) & 0x1ff
    }

    /// Get the index of the pud(second level, bit 30..38)
    pub(super) const fn pud_index(&self) -> usize {
        (self.0 >> 30) & 0x1ff
    }

    /// Get the index of the pgd(first level, bit 39..47)
    pub(super) const fn pgd_index(&self) -> usize {
        (self.0 >> 39) & 0x1ff
    }
}

/// Get the slice of the page_table items
///
/// Addr should be virtual address.
pub(super) fn page_slice<T>(addr: usize) -> &'static mut [T] {
    assert!(addr >= KERNEL_ELF_BASE_OFFSET);
    // The size of the page_table is 4K
    // The size of the item is sizeof::<usize>() bytes
    // 4096 / sizeof::<usize>() == 512
    // So the len is 512
    convert_to_mut_slice::<T>(addr, 0x200)
}

pub fn ap_from_vm_rights(rights: vm_rights_t) -> usize {
    // match rights {
    //     vm_rights_t::VMKernelOnly => 0,
    //     vm_rights_t::VMReadWrite => 1,
    //     vm_rights_t::VMReadOnly => 3,
    // }
    rights as usize
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PGDE(usize);
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PUDE(usize);
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PDE(pub usize);
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PTE(pub usize);

#[repr(C)]
#[derive(Debug, Clone)]
pub struct ASID(usize);

enum pude_tag_t {
    pude_invalid = 0,
    pude_1g = 1,
    pude_pd = 3,
}

enum pde_tag_t {
    pde_large = 1,
    pde_small = 3,
}

/// Implemente generic function for given Ident
#[macro_export]
macro_rules! impl_multi {
    ($($t:ident),* {$($block:item)*}) => {
        macro_rules! methods {
            () => {
                $($block)*
            };
        }
        $(
            impl $t {
                methods!();
            }
        )*
    }
}

// Implemente generic function for PGDE PUDE PDE
impl_multi!(PGDE, PUDE, PDE {
    /// Get the slice of the next level page.
    ///
    /// PGDE -> PUDE[PAGE_ITEMS]
    #[inline]
    pub fn next_level_slice<T>(&self) -> &'static mut [T] {
        page_slice(paddr_to_pptr(self.next_level_paddr()))
    }
});

impl_multi!(PGDE, PUDE, PDE, PTE {
    #[inline]
    pub fn get_ptr(&self) -> usize {
        self as *const Self as usize
    }

    /// Get the next level paddr
    #[inline]
    pub const fn next_level_paddr(&self) -> usize {
        self.0 & PAGE_ADDR_MASK
    }
    /// Set the next level paddr
    ///
    /// If It is PT or HUGE_PAGE, it will set the maped physical address
    /// Else it is the page to contains list
    #[inline]
    pub fn set_next_level_paddr(&mut self, value: usize) {
        self.0 = (self.0 & !PAGE_ADDR_MASK) | (value & PAGE_ADDR_MASK);
    }
    /// Set Page Attribute
    #[inline]
    pub fn set_attr(&mut self, value: usize) {
        self.0 = (self.0 & PAGE_ADDR_MASK) | (value & !PAGE_ADDR_MASK);
    }
    /// Get the address of the self.
    #[inline]
    pub fn self_addr(&self) -> usize {
        self as *const _ as _
    }
    /// Get the attribute of self
    #[inline]
    pub const fn attr(&self) -> usize {
        self.0 & !PAGE_ADDR_MASK
    }
    /// Create self through addr and attributes.
    #[inline]
    pub const fn new_page(addr: usize, sign: usize) -> Self {
        Self((addr & PAGE_ADDR_MASK) | (sign & !PAGE_ADDR_MASK))
    }

    /// Get the page's type info
    #[inline]
    pub const fn get_type(&self) -> usize {
        self.0 & 0x3
    }

    #[inline]
    pub const fn new_from_pte(word: usize) -> Self {
        Self(word)
    }

    #[inline]
    pub fn invalidate(&mut self) {
        self.0 = 0;
    }

    #[inline]
    pub fn new_invalid() -> Self {
        Self::new_page(0, 0)
    }
});

impl PGDE {
    #[inline]
    pub const fn get_pgde_type(&self) -> usize {
        self.0 & 0x3
    }

    #[inline]
    pub const fn get_present(&self) -> bool {
        self.get_pgde_type() == 3 // pgde_pgde_pud
    }

    #[inline]
    pub const fn get_pud_base_address(&self) -> usize {
        self.0 & 0xfffffffff000
    }

    ///用于记录某个虚拟地址`vptr`对应的pte表项在内存中的位置
    pub fn lookup_pt_slot(&self, vptr: vptr_t) -> lookupPTSlot_ret_t {
        let pdSlot = self.lookup_pd_slot(vptr);
        if pdSlot.status != exception_t::EXCEPTION_NONE {
            let ret = lookupPTSlot_ret_t {
                status: pdSlot.status,
                ptSlot: 0 as *mut PTE,
            };
            return ret;
        }
        unsafe {
            if (*pdSlot.pdSlot).get_present() == false {
                *get_current_lookup_fault() =
                    lookup_fault_t::new_missing_cap(seL4_PageBits + PT_INDEX_BITS);

                let ret = lookupPTSlot_ret_t {
                    status: exception_t::EXCEPTION_LOOKUP_FAULT,
                    ptSlot: 0 as *mut PTE,
                };
                return ret;
            }
        }
        let ptIndex = GET_PT_INDEX(vptr);
        let pt = unsafe { paddr_to_pptr((*pdSlot.pdSlot).get_pt_base_address()) as *mut PTE };

        let ret = lookupPTSlot_ret_t {
            status: exception_t::EXCEPTION_NONE,
            ptSlot: unsafe { pt.add(ptIndex) },
        };
        ret
    }

    // acturally the lookup pd slot can only be seen under aarch64 and x86 in sel4
    // and in sel4, it should be the impl function of vspace_root_t
    // but as it define the pde_t as vspace_root_t and define PTE as vspace_root_t
    // so I think it is reasonable here to let those functions as a member funcion of PTE
    // commented by ZhiyuanSue
    pub fn lookup_pd_slot(&self, vptr: vptr_t) -> lookupPDSlot_ret_t {
        let pudSlot: lookupPUDSlot_ret_t = self.lookup_pud_slot(vptr);
        if pudSlot.status != exception_t::EXCEPTION_NONE {
            let ret = lookupPDSlot_ret_t {
                status: pudSlot.status,
                pdSlot: 0 as *mut PDE,
            };
            return ret;
        }
        unsafe {
            if (*pudSlot.pudSlot).get_present() == false {
                *get_current_lookup_fault() =
                    lookup_fault_t::new_missing_cap(seL4_PageBits + PT_INDEX_BITS + PD_INDEX_BITS);

                let ret = lookupPDSlot_ret_t {
                    status: exception_t::EXCEPTION_LOOKUP_FAULT,
                    pdSlot: 0 as *mut PDE,
                };
                return ret;
            }
        }
        let pdIndex = GET_PD_INDEX(vptr);
        let pd = unsafe { paddr_to_pptr((*pudSlot.pudSlot).get_pd_base_address()) as *mut PDE };

        let ret = lookupPDSlot_ret_t {
            status: exception_t::EXCEPTION_NONE,
            pdSlot: unsafe { pd.add(pdIndex) },
        };
        ret
    }

    pub fn lookup_pud_slot(&self, vptr: vptr_t) -> lookupPUDSlot_ret_t {
        let pgdSlot = self.lookup_pgd_slot(vptr);
        unsafe {
            if (*pgdSlot.pgdSlot).get_present() == false {
                *get_current_lookup_fault() = lookup_fault_t::new_missing_cap(
                    seL4_PageBits + PT_INDEX_BITS + PD_INDEX_BITS + PUD_INDEX_BITS,
                );
                let ret = lookupPUDSlot_ret_t {
                    status: exception_t::EXCEPTION_LOOKUP_FAULT,
                    pudSlot: 0 as *mut PUDE,
                };
                return ret;
            }
        }
        let pudIndex = GET_UPUD_INDEX(vptr);
        let pud = unsafe { paddr_to_pptr((*pgdSlot.pgdSlot).get_pud_base_address()) as *mut PUDE };
        let ret = lookupPUDSlot_ret_t {
            status: exception_t::EXCEPTION_NONE,
            pudSlot: unsafe { pud.add(pudIndex) },
        };
        ret
    }

    pub fn lookup_pgd_slot(&self, vptr: vptr_t) -> lookupPGDSlot_ret_t {
        let pgdIndex = GET_PGD_INDEX(vptr);
        let ret = lookupPGDSlot_ret_t {
            status: exception_t::EXCEPTION_NONE,
            pgdSlot: unsafe { (self.0 as *mut PGDE).add(pgdIndex) },
        };
        ret
    }
    pub fn lookup_frame(&self, vptr: vptr_t) -> lookupFrame_ret_t {
        let mut ret = lookupFrame_ret_t {
            valid: false,
            frameBase: 0,
            frameSize: 0,
        };
        let pudSlot = self.lookup_pud_slot(vptr);
        if pudSlot.status != exception_t::EXCEPTION_NONE {
            ret.valid = false;
            return ret;
        }
        let pudSlot = convert_to_type_ref::<PUDE>(pudSlot.pudSlot as usize);
        unsafe {
            match core::mem::transmute::<u8, pude_tag_t>(pudSlot.get_type() as _) {
                pude_tag_t::pude_1g => {
                    ret.frameBase = pudSlot.pude_1g_ptr_get_page_base_address();
                    ret.frameSize = ARM_Huge_Page;
                    ret.valid = true;
                    return ret;
                }
                pude_tag_t::pude_pd => {
                    // TODO: check if below code from sel4 is work
                    //         pde_t *pd = paddr_to_pptr(pude_pude_pd_ptr_get_pd_base_address(pudSlot.pudSlot));
                    //         pde_t *pdSlot = pd + GET_PD_INDEX(vptr);
                    let pdSlot: &PDE = pudSlot.next_level_slice()[GET_PD_INDEX(vptr)];

                    if pdSlot.get_type() == pde_tag_t::pde_large as usize {
                        ret.frameBase = pdSlot.pde_large_ptr_get_page_base_address();
                        ret.frameSize = ARM_Large_Page;
                        ret.valid = true;
                        return ret;
                    }

                    if pdSlot.get_type() == pde_tag_t::pde_small as usize {
                        let ptSlot: &PTE = pdSlot.next_level_slice()[GET_PT_INDEX(vptr)];
                        if ptSlot.pte_table_get_present() {
                            ret.frameBase = ptSlot.pte_ptr_get_page_base_address();
                            ret.frameSize = ARM_Small_Page;
                            ret.valid = true;
                            return ret;
                        }
                    }
                }
                _ => panic!("invalid pt slot type:{}", pudSlot.get_type()),
            }
        }
        ret
    }
}

impl PUDE {
    #[inline]
    pub const fn new_1g(
        uxn: bool,
        page_base_address: usize,
        ng: usize,
        af: usize,
        sh: usize,
        ap: usize,
        attr_index: mair_types,
    ) -> Self {
        Self(
            (uxn as usize & 0x1) << 54
                | (page_base_address & 0xffffc0000000)
                | (ng & 0x1) << 11
                | (af & 0x1) << 10
                | (sh & 0x1) << 8
                | (ap & 0x1) << 6
                | (attr_index as usize & 0x7) << 2
                | 0x1, // pude_1g_tag
        )
    }

    #[inline]
    pub const fn get_pude_type(&self) -> usize {
        self.0 & 0x3
    }

    // Check whether the pude is a 1g huge page.
    #[inline]
    pub const fn is_1g_page(&self) -> bool {
        self.0 & 0x3 == 1
    }

    #[inline]
    pub const fn get_present(&self) -> bool {
        self.get_pude_type() == 3 // pude_pude_pd
    }

    #[inline]
    pub fn pude_1g_ptr_get_page_base_address(&self) -> usize {
        self.0 & 0xffffc0000000
    }
    #[inline]
    pub fn get_pd_base_address(&self) -> usize {
        self.0 & 0xfffffffff000
    }
}

impl PDE {
    #[inline]
    pub const fn new_large(
        uxn: bool,
        page_base_address: usize,
        ng: usize,
        af: usize,
        sh: usize,
        ap: usize,
        attr_index: mair_types,
    ) -> Self {
        Self(
            (uxn as usize & 0x1) << 54
                | (page_base_address & 0xffffffe00000)
                | (ng & 0x1) << 11
                | (af & 0x1) << 10
                | (sh & 0x1) << 8
                | (ap & 0x1) << 6
                | (attr_index as usize & 0x7) << 2
                | 0x1, // pde_large_tag
        )
    }

    #[inline]
    pub const fn new_small(pt_base_address: usize) -> Self {
        Self((pt_base_address & 0xfffffffff000) | (pde_tag_t::pde_small as usize & 0x3))
    }

    #[inline]
    pub const fn get_pde_type(&self) -> usize {
        self.0 & 0x3
    }

    #[inline]
    pub const fn get_present(&self) -> bool {
        self.get_pde_type() == 3 // pde_pde_small
    }

    /// Check whether it is a 2M huge page.
    #[inline]
    pub const fn is_larger_page(&self) -> bool {
        self.get_pde_type() == 3
    }

    #[inline]
    pub const fn pde_large_ptr_get_page_base_address(&self) -> usize {
        self.0 & 0xffffffe00000
    }

    // TODO: Rename get_pt_base_address to get_base_address
    #[inline]
    pub const fn get_pt_base_address(&self) -> usize {
        self.0 & 0xfffffffff000
    }
}

impl PTE {
    #[inline]
    pub const fn get_reserved(&self) -> usize {
        self.0 & 0x3
    }

    // TODO: Rename to is_present()
    #[inline]
    pub const fn pte_ptr_get_present(&self) -> bool {
        self.get_reserved() == 0x3
    }

    #[inline]
    pub const fn pte_ptr_get_page_base_address(&self) -> usize {
        self.0 & 0xfffffffff000
    }
}

/// Get current lookup fault object.
pub(super) fn get_current_lookup_fault() -> &'static mut lookup_fault_t {
    unsafe {
        (ffi_addr!(current_lookup_fault) as *mut lookup_fault_t)
            .as_mut()
            .unwrap()
    }
}
