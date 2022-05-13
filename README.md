![Rust](https://github.com/rust-disk-partition-management/gptman/workflows/Rust/badge.svg)
[![Latest Version](https://img.shields.io/crates/v/gptman.svg)](https://crates.io/crates/gptman)
![Rust 1.46+](https://img.shields.io/badge/rust-1.46%2B-orange.svg)
![License](https://img.shields.io/crates/l/gptman)
[![Docs.rs](https://docs.rs/gptman/badge.svg)](https://docs.rs/gptman)
[![LOC](https://tokei.rs/b1/github/rust-disk-partition-management/gptman)](https://github.com/rust-disk-partition-management/gptman)
[![Dependency Status](https://deps.rs/repo/github/rust-disk-partition-management/gptman/status.svg)](https://deps.rs/repo/github/rust-disk-partition-management/gptman)

gptman
======

A library that allows managing GUID partition tables.

A CLI tool for Linux that allows you to copy a partition from one disk to
another and more.

Features
--------

 *  Read/Write GPT from 512 and 4096 bytes sector size disks
 *  Create a new GPT on a disk (-i, --init)
 *  Insert/delete a partition in the table (n, d)
 *  Align partitions automatically (a)
 *  **Resize a partition (r)**
 *  **Copy/clone a partition from one disk and insert it to another (c)**
 *  Change partition type (and list by category) (t)
 *  Fix partitions order (f)
 *  Change disk GUID (i)
 *  Change partition name (L)
 *  Change partition GUID (u)
 *  Toggle legacy BIOS bootable (A)
 *  Toggle no block IO protocol (B)
 *  Toggle required partition flag (R)
 *  Toggle attributes (S)
 *  Customize columns to print (-o, --output)
 *  Print raw data of disklabel (D)
 *  Swap partition indexes (s)
 *  Randomize disk's GUID and all partition's GUID (Z)
 *  Copy/clone all partitions from one disk and insert it to another (C)
 *  Write protective MBR
 *  Check how empty a partition physically is (number of empty blocks) (z)

Installation
------------

 *  With `curl`:

    ```
    curl -sSL http://gptman.cecton.com | sh -
    ```

 *  CLI:

    ```
    cargo install --features=cli gptman
    ```

    Statically linked:

    ```
    cargo install --features=cli --target=x86_64-unknown-linux-musl gptman
    ```

 *  Library:

    Cargo.toml:
    ```toml
    [dependencies]
    gptman = "0.8.0"
    ```

Library Usage
-------------

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
