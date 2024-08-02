use crate::{arch::aarch64::machine::clean_by_va_pou, vm_attributes_t, PDE, PGDE, PTE, PUDE};

use super::utils::paddr_to_pptr;
use sel4_common::{
    arch::vm_rights_t,
    sel4_config::seL4_PageTableBits,
    utils::{convert_ref_type_to_usize, convert_to_mut_type_ref},
    BIT,
};

#[allow(unused)]
pub enum VMPageSize {
    ARMSmallPage = 0,
    ARMLargePage,
    ARMHugePage,
}

#[allow(unused)]
impl VMPageSize {
    /// Get VMPageSize from usize
    pub fn try_from_usize(value: usize) -> Option<VMPageSize> {
        match value {
            0 => Some(VMPageSize::ARMSmallPage),
            1 => Some(VMPageSize::ARMLargePage),
            2 => Some(VMPageSize::ARMHugePage),
            _ => None,
        }
    }
}

#[allow(unused)]
enum pte_tag_t {
    pte_table = 3,
    pte_page = 1,
    pte_4k_page = 7,
    pte_invalid = 0,
}

#[allow(unused)]
enum pude_tag_t {
    pude_invalid = 0,
    pude_1g = 1,
    pude_pd = 3,
}

#[allow(unused)]
enum pde_tag_t {
    pde_large = 1,
    pde_small = 3,
}

bitflags::bitflags! {
    /// Possible flags for a page table entry.
    pub struct PTEFlags: usize {
        // Attribute fields in stage 1 VMSAv8-64 Block and Page descriptors:
        /// Whether the descriptor is valid.
        const VALID =       BIT!(0);
        /// The descriptor gives the address of the next level of translation table or 4KB page.
        /// (not a 2M, 1G block)
        const NON_BLOCK =   BIT!(1);
        /// Memory attributes index field.
        const ATTR_INDX =   0b111 << 2;
        const NORMAL_NONCACHE = 0b010 << 2;
        const NORMAL =      0b100 << 2;
        /// Non-secure bit. For memory accesses from Secure state, specifies whether the output
        /// address is in Secure or Non-secure memory.
        const NS =          BIT!(5);
        /// Access permission: accessable at EL0.
        const AP_EL0 =      BIT!(6);
        /// Access permission: read-only.
        const AP_RO =       BIT!(7);
        /// Shareability: Inner Shareable (otherwise Outer Shareable).
        const INNER =       BIT!(8);
        /// Shareability: Inner or Outer Shareable (otherwise Non-shareable).
        const SHAREABLE =   BIT!(9);
        /// The Access flag.
        const AF =          BIT!(10);
        /// The not global bit.
        const NG =          BIT!(11);
        /// Indicates that 16 adjacent translation table entries point to contiguous memory regions.
        const CONTIGUOUS =  BIT!(52);
        /// The Privileged execute-never field.
        const PXN =         BIT!(53);
        /// The Execute-never or Unprivileged execute-never field.
        const UXN =         BIT!(54);

        // Next-level attributes in stage 1 VMSAv8-64 Table descriptors:

        /// PXN limit for subsequent levels of lookup.
        const PXN_TABLE =           BIT!(59);
        /// XN limit for subsequent levels of lookup.
        const XN_TABLE =            BIT!(60);
        /// Access permissions limit for subsequent levels of lookup: access at EL0 not permitted.
        const AP_NO_EL0_TABLE =     BIT!(61);
        /// Access permissions limit for subsequent levels of lookup: write access not permitted.
        const AP_NO_WRITE_TABLE =   BIT!(62);
        /// For memory accesses from Secure state, specifies the Security state for subsequent
        /// levels of lookup.
        const NS_TABLE =            BIT!(63);

    }
}

impl PTE {
    pub fn new(addr: usize, flags: PTEFlags) -> Self {
        Self((addr & 0xfffffffff000) | flags.bits())
    }
    pub fn pte_next_table(addr: usize, _: bool) -> Self {
        Self::new(addr, PTEFlags::VALID | PTEFlags::NON_BLOCK)
    }
    fn new_4k_page(addr: usize, flags: PTEFlags) -> Self {
        Self((addr & 0xfffffffff000) | flags.bits() | 0x400000000000003)
    }

    pub fn get_pte_from_ppn_mut(&self) -> &mut PTE {
        convert_to_mut_type_ref::<PTE>(paddr_to_pptr(self.get_ppn() << seL4_PageTableBits))
    }

    pub fn get_ppn(&self) -> usize {
        (self.0 & 0xfffffffff000) >> 10
    }

    pub fn as_pgde(&self) -> PGDE {
        PGDE::new_from_pte(self.0)
    }

    pub fn as_pude(&self) -> PUDE {
        PUDE::new_from_pte(self.0)
    }

    pub fn as_pde(&self) -> PDE {
        PDE::new_from_pte(self.0)
    }

    pub fn is_pte_table(&self) -> bool {
        self.get_type() != pte_tag_t::pte_table as usize
    }
    pub fn get_valid(&self) -> usize {
        (self.get_type() != pte_tag_t::pte_invalid as usize) as usize
    }

    pub fn pte_table_get_present(&self) -> bool {
        self.get_type() != pte_tag_t::pte_table as usize
    }

    #[inline]
    pub fn update(&mut self, pte: Self) {
        *self = pte;
        clean_by_va_pou(
            convert_ref_type_to_usize(self),
            convert_ref_type_to_usize(self),
        );
    }

    pub fn ap_from_vm_rights_t(rights: vm_rights_t) -> PTEFlags {
        match rights {
            vm_rights_t::VMKernelOnly => PTEFlags::empty(),
            vm_rights_t::VMReadWrite => PTEFlags::AP_EL0,
            vm_rights_t::VMReadOnly => PTEFlags::AP_EL0 | PTEFlags::AP_RO,
        }
    }

    pub fn make_user_pte(
        paddr: usize,
        rights: vm_rights_t,
        attr: vm_attributes_t,
        page_size: usize,
    ) -> Self {
        let nonexecutable = attr.get_armExecuteNever();
        let cacheable = attr.get_armPageCacheable();
        let mut flags = PTEFlags::NG;
        if cacheable {
            flags |= PTEFlags::NORMAL;
        }
        if nonexecutable {
            flags |= PTEFlags::UXN;
        }
        flags |= Self::ap_from_vm_rights_t(rights);
        if VMPageSize::ARMSmallPage as usize == page_size {
            PTE::new_4k_page(paddr, flags)
        } else {
            PTE::new(paddr, flags)
        }
    }

    pub fn pte_new(
        UXN: usize,
        page_base_address: usize,
        nG: usize,
        AF: usize,
        SH: usize,
        AP: usize,
        AttrIndx: usize,
        reserved: usize,
    ) -> PTE {
        let val = 0
            | (UXN & 0x1) << 54
            | (page_base_address & 0xfffffffff000) >> 0
            | (nG & 0x1) << 11
            | (AF & 0x1) << 10
            | (SH & 0x3) << 8
            | (AP & 0x3) << 6
            | (AttrIndx & 0x7) << 2
            | (reserved & 0x3) << 0;

        PTE(val)
    }
}
