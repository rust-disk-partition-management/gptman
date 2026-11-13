use std::fs;
use std::io;
use std::os::linux::fs::MetadataExt;
use std::os::unix::io::AsRawFd;

mod ioctl {
    use nix::{ioctl_none, ioctl_read_bad, request_code_none};

    ioctl_read_bad!(
        blksszget,
        request_code_none!(0x12, 104),
        ::std::os::raw::c_int
    );
    ioctl_none!(blkrrpart, 0x12, 95);
}

const S_IFMT: u32 = 0o170_000;
const S_IFBLK: u32 = 0o60_000;

/// An error that can happen while doing an ioctl call with a block device
#[derive(Debug)]
pub enum BlockError {
    /// An error that occurs when the metadata of the input file couldn't be retrieved
    Metadata(io::Error),
    /// An error that occurs when the partition table could not be reloaded by the OS
    RereadTable(nix::Error),
    /// An error that occurs when the sector size could not be retrieved from the OS
    GetSectorSize(nix::Error),
    /// An error that occurs when an invalid return code has been received from an ioctl call
    InvalidReturnValue(i32),
    /// An error that occurs when the file provided is not a block device
    NotBlock,
}

impl core::fmt::Display for BlockError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BlockError::Metadata(_) => f.write_str("failed to get metadata of device fd"),
            BlockError::RereadTable(_) => f.write_str("failed to reload partition table of device"),
            BlockError::GetSectorSize(err) => {
                write!(f, "failed to get the sector size of device: {err}")
            }
            BlockError::InvalidReturnValue(value) => {
                write!(f, "invalid return value of ioctl ({value} != 0)")
            }
            BlockError::NotBlock => f.write_str("not a block device"),
        }
    }
}

impl core::error::Error for BlockError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            BlockError::Metadata(err) => Some(err),
            BlockError::RereadTable(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for BlockError {
    fn from(err: io::Error) -> BlockError {
        BlockError::Metadata(err)
    }
}

impl From<nix::Error> for BlockError {
    fn from(err: nix::Error) -> BlockError {
        BlockError::RereadTable(err)
    }
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
///
/// Under normal conditions, `get_sector_size` retrieves the logical sector size using the `BLKSSZGET` ioctl command.
/// If the system returns an error, the function handles it appropriately. However, in the rare event that the system
/// reports success (0) but writes a negative value, the function will default to 512 bytes to ensure stability.
/// This is a safeguard against unexpected system behavior.
pub fn get_sector_size(file: &mut fs::File) -> Result<u64, BlockError> {
    let metadata = file.metadata().map_err(BlockError::Metadata)?;
    let mut sector_size = 512;

    if metadata.st_mode() & S_IFMT == S_IFBLK {
        match unsafe { ioctl::blksszget(file.as_raw_fd(), &mut sector_size) } {
            Err(err) => Err(BlockError::GetSectorSize(err)),
            Ok(0) => Ok(sector_size.try_into().unwrap_or(512)),
            Ok(r) => Err(BlockError::InvalidReturnValue(r)),
        }
    } else {
        Err(BlockError::NotBlock)
    }
}
