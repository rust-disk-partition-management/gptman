![Rust](https://github.com/rust-disk-partition-management/gptman/actions/workflows/rust.yml/badge.svg)
[![Latest Version](https://img.shields.io/crates/v/gptman.svg)](https://crates.io/crates/gptman)
![Rust 1.63+](https://img.shields.io/badge/rust-1.63%2B-orange.svg)
![License](https://img.shields.io/crates/l/gptman)
[![Docs.rs](https://docs.rs/gptman/badge.svg)](https://docs.rs/gptman)
[![LOC](https://tokei.rs/b1/github/rust-disk-partition-management/gptman)](https://github.com/rust-disk-partition-management/gptman)
[![Dependency Status](https://deps.rs/repo/github/rust-disk-partition-management/gptman/status.svg)](https://deps.rs/repo/github/rust-disk-partition-management/gptman)

gptman
======

Pure Rust library to read and modify GUID partition tables.

Things you can do
-----------------

 *  Read/Write GPT from 512 and 4096 bytes sector size disks
 *  Create a new GPT on a disk
 *  Insert/delete a partition in the table
 *  Align partitions automatically (to sector size)
 *  Resize a partition
 *  Copy/clone a partition from one disk and insert it into another
 *  Change partition type
 *  Fix partitions order
 *  Change disk GUID
 *  Change partition name
 *  Change partition GUID
 *  Toggle legacy BIOS bootable
 *  Toggle no block IO protocol
 *  Toggle required partition flag
 *  Toggle attributes
 *  Swap partition indexes
 *  Copy/clone all partitions from one disk and insert it to another
 *  Write protective MBR

Installation
------------

Cargo.toml:
```toml
[dependencies]
gptman = "1"
```

Usage
-----

Reading all the partitions of a disk:

```rust
let mut f = std::fs::File::open("tests/fixtures/disk1.img")
    .expect("could not open disk");
let gpt = gptman::GPT::find_from(&mut f)
    .expect("could not find GPT");

println!("Disk GUID: {:?}", gpt.header.disk_guid);

for (i, p) in gpt.iter() {
    if p.is_used() {
        println!("Partition #{}: type = {:?}, size = {} bytes, starting lba = {}",
            i,
            p.partition_type_guid,
            p.size().unwrap() * gpt.sector_size,
            p.starting_lba);
    }
}
```

Creating new partitions:

```rust
let mut f = std::fs::File::open("tests/fixtures/disk1.img")
    .expect("could not open disk");
let mut gpt = gptman::GPT::find_from(&mut f)
    .expect("could not find GPT");

let free_partition_number = gpt.iter().find(|(i, p)| p.is_unused()).map(|(i, _)| i)
    .expect("no more places available");
let size = gpt.get_maximum_partition_size()
    .expect("no more space available");
let starting_lba = gpt.find_optimal_place(size)
    .expect("could not find a place to put the partition");
let ending_lba = starting_lba + size - 1;

gpt[free_partition_number] = gptman::GPTPartitionEntry {
    partition_type_guid: [0xff; 16],
    unique_partition_guid: [0xff; 16],
    starting_lba,
    ending_lba,
    attribute_bits: 0,
    partition_name: "A Robot Named Fight!".into(),
};
```

Creating a new partition table with one entry that fills the entire disk:

```rust
let ss = 512;
let data = vec![0; 100 * ss as usize];
let mut cur = std::io::Cursor::new(data);
let mut gpt = gptman::GPT::new_from(&mut cur, ss as u64, [0xff; 16])
    .expect("could not create partition table");

gpt[1] = gptman::GPTPartitionEntry {
    partition_type_guid: [0xff; 16],
    unique_partition_guid: [0xff; 16],
    starting_lba: gpt.header.first_usable_lba,
    ending_lba: gpt.header.last_usable_lba,
    attribute_bits: 0,
    partition_name: "A Robot Named Fight!".into(),
};
```
