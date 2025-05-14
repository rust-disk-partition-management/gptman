Changelog
=========

## v2.0.0

- Bump versions of bincode, thiserror nix

## v1.1.4

- Fix get sector size on mips64 (#138)

## v1.1.3

- Document that pmbr shouldn't be used unless needed (#136)
- Update library MSRV to 1.65 because of crc 3.2.1 (#139)

## v1.1.2

- Remove leftover Cargo.lock file
- Allow a wider version range for nix (up to 0.27) (#135)

## v1.1.1

- [CLI] Remove CLI (#127)

## v1.1.0

- Add function write_bootable_protective_mbr_into (#132)
- [CLI] Update MSRV to 1.70 for clap 4.4

## v1.0.2

- Update library MSRV to 1.63 and edition to 2021 (#120)
- Turn off nix's default features (#123)

## v1.0.1

- Allow a wider version range for nix (#116)

## v1.0.0

- [BREAKING] Disable `cli` feature by default (#81)
- [BREAKING] Make `Error` type `non_exhaustive` so the API can be stabilized better (#94)
- [BREAKING CLI]: Have `--output` take lowercase field names as arguments (#77)
- [CLI] Replace structopt with clap 3.1 (#77)
- Define library MSRV of 1.46 and CLI MSRV of 1.54 (#79)
- Remove useless allocation (#93)
- Update repository URL (#82)
- Update dependencies

## v0.8.4

- Add GPTPartitionEntry::range and GPT::get_partition_byte_range (#76)

## v0.8.3

- Remove count-zeroes from library's dependency (only used by CLI)
- Fix licensing missing author and date

## v0.8.2

- Change release build to provide MUSL build for Linux by default
- Add download script on http://gptman.cecton.com
- [CLI] Add feature to check a partition for zeroes (useful to check if you accidentally trimmed it)

## v0.8.1

- Fix some rustc and clippy warnings (does **not** break API of gptman's lib)
- Update dependencies: crc from 1.0.0 to 2.0.0 and nix from 0.18 to 0.22
- Fix spacing in doc
- [CLI] Fix invalid maximum size calculation in `resize` command

## v0.8.0

- [BREAKING] Switch to `thiserror`: it is possible that the type `Error` has changed a tad bit because of this change. I did not investigated so I considered this a breaking change.
- Update all dependencies (most notable: rand from version 0.7 to version 0.8)

## v0.7.3

- Update license badge
- Include test fixtures in packaged crate

## v0.7.2 (yanked)

- [CLI] Automatic binary release on GitHub

## v0.7.1 (yanked)

- Remove rustfmt and clippy checks for branch master and tags
- Clippy fixes
- Get serde's `derive` dependency from feature instead of crate
- Replace Travis CI by GitHub Actions
- Add FUNDING.yml
- Change license to MIT OR Apache-2.0

## v0.7.0

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

## v0.6.5

- Bump version of serde

## v0.6.4

- Exclude tool metadata from crate
- Bump versions of serde, structopt, unicode-width, bincode

## v0.6.3

- Update dependencies
- Fix path for NVMe disks in the table in the CLI
- Fix wrong command description

## v0.6.2

- Bump version of dependencies

## v0.6.1

- [CLI] Correct path for disks on NVMe disks

## v0.6.0

- Bump version of dependencies

## v0.5.0

- Added ioctls to the library for Linux.
- Updated dependencies
- Added `find_at_sector()` and `remove_at_sector()` to the library
- Added `write_protective_mbr_into()` to the library
- Fixed: check partition index to be in range (CLI)
- Impl Error for gptman::Error
- Fixed wrong Error type in the API

## v0.4.0

- Added ioctls to the library for Linux.
- Updated dependencies
- Added `find_at_sector()` and `remove_at_sector()` to the library
- Added `write_protective_mbr_into()` to the library
- Fixed: check partition index to be in range (CLI)
- Impl Error for gptman::Error

## v0.3.0

- Allow the user to give size and starting LBA in bytes (#2)

## v0.2.0

- Reorganize: make a library, add documentation... (#1)

## v0.1.0

Initial release
