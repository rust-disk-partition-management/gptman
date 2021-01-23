use std::fs;
use std::io;
use std::os::linux::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use thiserror::Error;

mod ioctl {
    use nix::{ioctl_none, ioctl_read_bad};

    ioctl_read_bad!(blksszget, 0x1268, u64);
    ioctl_none!(blkrrpart, 0x12, 95);
}

const S_IFMT: u32 = 0o170_000;
const S_IFBLK: u32 = 0o60_000;

/// An error that can happen while doing an ioctl call with a block device
#[derive(Debug, Error)]
pub enum BlockError {
    /// An error that occurs when the metadata of the input file couldn't be retrieved
    #[error("failed to get metadata of device fd")]
    Metadata(#[from] io::Error),
    /// An error that occurs when the partition table could not be reloaded by the OS
    #[error("failed to reload partition table of device")]
    RereadTable(#[from] nix::Error),
    /// An error that occurs when the sector size could not be retrieved from the OS
    #[error("failed to get the sector size of device: {0}")]
    GetSectorSize(nix::Error),
    /// An error that occurs when an invalid return code has been received from an ioctl call
    #[error("invalid return value of ioctl ({0} != 0)")]
    InvalidReturnValue(i32),
    /// An error that occurs when the file provided is not a block device
    #[error("not a block device")]
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
            Err(err) => Err(BlockError::GetSectorSize(err)),
            Ok(0) => Ok(sector_size),
            Ok(r) => Err(BlockError::InvalidReturnValue(r)),
        }
    } else {
        Err(BlockError::NotBlock)
    }
}
