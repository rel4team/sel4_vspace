#[cfg(target_arch = "riscv64")]
pub mod riscv64;

#[cfg(target_arch = "riscv64")]
pub use riscv64::*;

#[cfg(target_arch = "riscv64")]
pub use riscv64::set_vm_root;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

#[cfg(target_arch = "aarch64")]
pub use aarch64::*;

#[derive(Debug, Copy, Clone)]
#[cfg_attr(target_arch = "riscv64", allow(dead_code))]
pub struct VAddr(usize);

/// Convert usize to VAddr
impl From<usize> for VAddr {
    fn from(value: usize) -> Self {
        VAddr(value)
    }
}
