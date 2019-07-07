#![cfg(target_os = "linux")]

use std::fs;
use std::io;
use std::os::linux::fs::MetadataExt;
use std::os::unix::io::AsRawFd;

mod ioctl {
    ioctl_read_bad!(blksszget, 0x1268, u64);
    ioctl_none!(blkrrpart, 0x12, 95);
}

const S_IFMT: u32 = 0o170_000;
const S_IFBLK: u32 = 0o60_000;

/// An error that can happen while doing an ioctl call with a block device
#[derive(Debug, Error)]
pub enum BlockError {
    /// An error that occurs when the metadata of the input file couldn't be retrieved
    #[error(display = "failed to get metadata of device fd")]
    Metadata(#[error(cause)] io::Error),
    /// An error that occurs when the partition table could not be reloaded by the OS
    #[error(display = "failed to reload partition table of device")]
    RereadTable(#[error(cause)] nix::Error),
    /// An error that occurs when an invalid return code has been received from an ioctl call
    #[error(display = "invalid return value of ioctl ({} != 0)", _0)]
    InvalidReturnValue(i32),
    /// An error that occurs when the file provided is not a block device
    #[error(display = "not a block device")]
    NotBlock,
}

/// Makes an ioctl call to make the OS reread the partition table of a block device
pub fn reread_partition_table(file: &mut fs::File) -> Result<(), BlockError> {
    let metadata = file.metadata().map_err(BlockError::Metadata)?;

    if metadata.st_mode() & S_IFMT == S_IFBLK {
        match unsafe { ioctl::blkrrpart(file.as_raw_fd()) } {
            Err(err) => Err(BlockError::RereadTable(err)),
            Ok(0) => Ok(()),
            Ok(r) => Err(BlockError::InvalidReturnValue(r)),
        }
    } else {
        Err(BlockError::NotBlock)
    }
}

/// Makes an ioctl call to obtain the sector size of a block device
pub fn get_sector_size(file: &mut fs::File) -> Result<u64, BlockError> {
    let metadata = file.metadata().map_err(BlockError::Metadata)?;
    let mut sector_size = 512;

    if metadata.st_mode() & S_IFMT == S_IFBLK {
        match unsafe { ioctl::blksszget(file.as_raw_fd(), &mut sector_size) } {
            Err(err) => Err(BlockError::RereadTable(err)),
            Ok(0) => Ok(sector_size),
            Ok(r) => Err(BlockError::InvalidReturnValue(r)),
        }
    } else {
        Err(BlockError::NotBlock)
    }
}
