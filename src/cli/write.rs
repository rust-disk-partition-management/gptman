use crate::cli::*;
use crate::gpt::GPT;
use std::fs;
use std::fs::OpenOptions;
use std::os::linux::fs::MetadataExt;
use std::os::unix::io::IntoRawFd;

ioctl_none!(reread_partition_table, 0x12, 95);

const S_IFMT: u32 = 0o00170000;
const S_IFBLK: u32 = 0o0060000;

pub fn write(gpt: &mut GPT, path: &str) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true);
    let mut f = options.open(path)?;
    gpt.write_into(&mut f)?;

    if fs::metadata(path)?.st_mode() & S_IFMT == S_IFBLK {
        println!("calling re-read ioctl");
        match unsafe { reread_partition_table(f.into_raw_fd()) } {
            Err(err) => println!("ioctl call failed: {}", err),
            Ok(0) => {}
            Ok(x) => println!("ioctl returned error code: {}", x),
        }
    }

    Ok(())
}
