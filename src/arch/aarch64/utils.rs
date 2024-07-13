use sel4_common::{
    arch::{
        config::{KERNEL_ELF_BASE_OFFSET, PPTR_BASE_OFFSET},
        vm_rights_t,
    },
    sel4_config::*,
    utils::convert_to_mut_slice,
    MASK,
};

use crate::arch::VAddr;

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
pub struct PDE(usize);
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PTE(usize);

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
    pub const fn new(addr: usize, sign: usize) -> Self {
        Self((addr & PAGE_ADDR_MASK) | (sign & !PAGE_ADDR_MASK))
    }
});

impl_multi!(PUDE {
    #[inline]
    pub const fn new_1g(
        uxn: usize,
        page_base_address: usize,
        ng: usize,
        af: usize,
        sh: usize,
        ap: usize,
        attr_index: usize
    ) -> Self {
        Self(
            (uxn & 0x1) << 54
            | (page_base_address & 0xffffc0000000)
            | (ng & 0x1) << 11
            | (af & 0x1) << 10
            | (sh & 0x1) << 8
            | (ap & 0x1) << 6
            | (attr_index & 0x7) << 2
            | 0x1, // pude_1g_tag
        )
    }
});

impl_multi!(PDE {
    #[inline]
    pub const fn new_large(
        uxn: usize,
        page_base_address: usize,
        ng: usize,
        af: usize,
        sh: usize,
        ap: usize,
        attr_index: usize
    ) -> Self {
        Self(
            (uxn & 0x1) << 54
            | (page_base_address & 0xffffffe00000)
            | (ng & 0x1) << 11
            | (af & 0x1) << 10
            | (sh & 0x1) << 8
            | (ap & 0x1) << 6
            | (attr_index & 0x7) << 2
            | 0x1,  // pde_large_tag
        )
    }
});
