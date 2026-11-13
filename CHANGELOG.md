# Changelog

## Unreleased (v4.0.0)

Breaking changes:

- Remove the `wincode` dependency (#149)
- `GPTHeader`, `GPTPartitionEntry` and `PartitionName` no longer implement wincode's `SchemaRead`/`SchemaWrite`. Use the new `from_bytes`/`to_bytes` methods to encode them in memory (#149)
- `GPTHeader::generate_partition_entry_array_crc32` and `GPTHeader::update_partition_entry_array_crc32` now return a `Result`: the array must hold as many entries as the header describes (#149)
- Remove `Error::Deserialize` and `Error::Seserialize`, which can no longer happen (#149)
- A truncated header or partition entry now returns `Error::Io` with `UnexpectedEof` (#149)
- Remove `PartitionName::as_str`, which has no replacement: a `&str` cannot borrow into `[u16; 36]`, so use `Display`, `to_string` or `format!` to print a name (#150)
- `PartitionName` no longer holds a `String`; it keeps the 36 UTF-16LE code units of the name alone (#150)

Changed:

- `GPTHeader::read_partitions` seeks once and reads the partition entry array a sector at a time, instead of seeking and reading once per entry as it did in v3.1.1. Peak heap is 19 984 bytes for `GPT::read_from` on a 128-entry table, against 19 624 in v3.1.1 (#149)
- `GPTHeader::read_from` rejects a partition entry size other than 128 and an array larger than 16 MiB (#149)
- Remove the `thiserror` dependency; `Error` and `BlockError` implement `core::fmt::Display` and `core::error::Error` by hand, with the same messages and sources (#149)
- Remove the `crc` dependency; the CRC-32 the specification requires is computed by a private module. Other than the optional `nix` feature, `gptman` has no dependencies left (#149)
- `PartitionName` is 72 bytes, the size of the name field of a partition entry, and `GPTPartitionEntry` is 128 bytes, the size the specification gives it, against 96 and 152 before: a 128-entry table takes 16 KiB instead of 19 KiB (#150)
- Remove the `homepage` field of `Cargo.toml`, which held the same URL as `repository`, where cargo warns about it since 1.101 (#150)

Added:

- `GPTHeader::from_bytes`/`to_bytes` over `&[u8; 92]`, `GPTPartitionEntry::from_bytes`/`to_bytes` over `&[u8; 128]` and `PartitionName::from_bytes`/`to_bytes` over `&[u8; 72]` (#149)
- `Error::InvalidPartitionEntrySize`, `Error::InvalidPartitionEntryArraySize` and `Error::InvalidPartitionEntryCount` (#149)
- `PartitionName` and `GPTPartitionEntry` are `Copy`, and `Debug` for `PartitionName` prints the name rather than its 36 code units (#150)

Fixed:

- `GPTHeader::generate_partition_entry_array_crc32` panicked on a header describing partition entries of a size other than 128, and on a `number_of_partition_entries` whose product with the entry size overflows a `u32`. Both now return an error (#149)
- `GPTHeader::read_partitions` allocated and seeked according to `number_of_partition_entries` read straight from the disk, so a crafted header could request a multi-gigabyte allocation (#149)
- `PartitionName::from` kept a name longer than 36 UTF-16 code units whole while writing only the first 36 to disk, so `as_str` could report a name the disk never holds. The name is now truncated to the 36 code units that are written (#150)
- The test module tripped `clippy::precedence` on an expression that shifted before it masked, which the lint job of the MSRV build reports as an error. The expression sat in a loop sampling code units that the exhaustive test below it already covers, so the loop went with it (#150)
- Test that a partition name whose bytes after the null terminator hold garbage survives a rewrite, which is the case parted and GParted produce when they rename a partition, and the one issue #142 was about (#150)

## v3.1.1 (2026-04-28)

- Add release workflow with OIDC trusted publishing
- Add CONTRIBUTING.md
- Document `nix` feature flag in README
- Use README as crate-level documentation via `include_str!`
- Add dates to all changelog entries
- Replace broken LOC badge with CHANGELOG badge

## v3.1.0 (2026-04-14)

- Fix fallback to backup GPT header when primary partition entry array CRC fails (#145)
- Add `GPTHeader::read_partitions` to read and validate the partition entry array for a given header (#145)

## v3.0.0 (2026-01-19)

- Replace unmaintained `bincode` dependency with `wincode` (#144)
- Remove `serde` dependency (#144)

## v2.0.2 (2025-12-23)

- Properly perform checksums on partition table arrays when partition names contains garbage data after the null terminator (#142)

## v2.0.1 (2025-06-16)

- Fix underflow when input is shorter than one sector (#141)

## v2.0.0 (2025-05-14)

- Bump versions of bincode, thiserror nix

## v1.1.4 (2025-02-12)

- Fix get sector size on mips64 (#138)

## v1.1.3 (2025-02-11)

- Document that pmbr shouldn't be used unless needed (#136)
- Update library MSRV to 1.65 because of crc 3.2.1 (#139)

## v1.1.2 (2024-02-12)

- Remove leftover Cargo.lock file
- Allow a wider version range for nix (up to 0.27) (#135)

## v1.1.1 (2024-02-07)

- [CLI] Remove CLI (#127)

## v1.1.0 (2024-01-15)

- Add function write_bootable_protective_mbr_into (#132)
- [CLI] Update MSRV to 1.70 for clap 4.4

## v1.0.2 (2023-08-21)

- Update library MSRV to 1.63 and edition to 2021 (#120)
- Turn off nix's default features (#123)

## v1.0.1 (2023-07-22)

- Allow a wider version range for nix (#116)

## v1.0.0 (2022-06-04)

- [BREAKING] Disable `cli` feature by default (#81)
- [BREAKING] Make `Error` type `non_exhaustive` so the API can be stabilized better (#94)
- [BREAKING CLI]: Have `--output` take lowercase field names as arguments (#77)
- [CLI] Replace structopt with clap 3.1 (#77)
- Define library MSRV of 1.46 and CLI MSRV of 1.54 (#79)
- Remove useless allocation (#93)
- Update repository URL (#82)
- Update dependencies

## v0.8.4 (2022-05-10)

- Add GPTPartitionEntry::range and GPT::get_partition_byte_range (#76)

## v0.8.3 (2021-12-10)

- Remove count-zeroes from library's dependency (only used by CLI)
- Fix licensing missing author and date

## v0.8.2 (2021-11-07)

- Change release build to provide MUSL build for Linux by default
- Add download script on http://gptman.cecton.com
- [CLI] Add feature to check a partition for zeroes (useful to check if you accidentally trimmed it)

## v0.8.1 (2021-11-07)

- Fix some rustc and clippy warnings (does **not** break API of gptman's lib)
- Update dependencies: crc from 1.0.0 to 2.0.0 and nix from 0.18 to 0.22
- Fix spacing in doc
- [CLI] Fix invalid maximum size calculation in `resize` command

## v0.8.0 (2021-03-13)

- [BREAKING] Switch to `thiserror`: it is possible that the type `Error` has changed a tad bit because of this change. I did not investigated so I considered this a breaking change.
- Update all dependencies (most notable: rand from version 0.7 to version 0.8)

## v0.7.3 (2020-12-23)

- Update license badge
- Include test fixtures in packaged crate

## v0.7.2 (2020-12-12) (yanked)

- [CLI] Automatic binary release on GitHub

## v0.7.1 (2020-12-12) (yanked)

- Remove rustfmt and clippy checks for branch master and tags
- Clippy fixes
- Get serde's `derive` dependency from feature instead of crate
- Replace Travis CI by GitHub Actions
- Add FUNDING.yml
- Change license to MIT OR Apache-2.0

## v0.7.0 (2020-08-11)

- The dependencies have been upgraded to the latest
- [BREAKING] A typo in the API has been fixed `gptman::GPTPartitionEntry::unique_parition_guid` is now `gptman::GPTPartitionEntry::unique_partition_guid`
- All structs now implement PartialEq and Eq
- A lot of improvement in the documentation
- [CLI] Now capable to also display the partitions in the order on the disk
- [CLI] Fix missing doc like how to quit the program
- Fix calculation of `first_usable_lba` for backup GPT
- Partitions boundaries are now check ( partitions must have positive size, must not overlap, and must fit within the disk)
- [BREAKING] `partition_entry_lba`'s update has been moved to `update_from` (was in `write_into`)
- New helpers: `is_primary`, `is_backup`
- [BREAKING] `write_into` does not update the GPT's first/last LBAs according to disk anymore

## v0.6.5 (2020-07-20)

- Bump version of serde

## v0.6.4 (2020-07-20)

- Exclude tool metadata from crate
- Bump versions of serde, structopt, unicode-width, bincode

## v0.6.3 (2020-05-16)

- Update dependencies
- Fix path for NVMe disks in the table in the CLI
- Fix wrong command description

## v0.6.2 (2020-03-08)

- Bump version of dependencies

## v0.6.1 (2020-01-04)

- [CLI] Correct path for disks on NVMe disks

## v0.6.0 (2020-01-03)

- Bump version of dependencies

## v0.5.0 (2019-07-14)

- Added ioctls to the library for Linux.
- Updated dependencies
- Added `find_at_sector()` and `remove_at_sector()` to the library
- Added `write_protective_mbr_into()` to the library
- Fixed: check partition index to be in range (CLI)
- Impl Error for gptman::Error
- Fixed wrong Error type in the API

## v0.4.0 (2019-07-14)

- Added ioctls to the library for Linux.
- Updated dependencies
- Added `find_at_sector()` and `remove_at_sector()` to the library
- Added `write_protective_mbr_into()` to the library
- Fixed: check partition index to be in range (CLI)
- Impl Error for gptman::Error

## v0.3.0 (2019-05-01)

- Allow the user to give size and starting LBA in bytes (#2)

## v0.2.0 (2019-01-10)

- Reorganize: make a library, add documentation... (#1)

## v0.1.0 (2019-01-07)

Initial release
