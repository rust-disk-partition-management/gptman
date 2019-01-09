[![Build Status](https://travis-ci.org/cecton/gptman.svg?branch=master)](https://travis-ci.org/cecton/gptman)

gptman
======

A GPT manager that allows you to copy a partition from one disk to another and
more.

Features
--------

 *  [x] Read GPT from 512 bytes sector size disks
 *  [x] Read GPT from 4096 bytes sector size disks
 *  [x] Use backup header in case of failure
 *  [x] Calculates checksums
 *  [x] Write GPT from 512 bytes sector size disks
 *  [x] Write GPT from 4096 bytes sector size disks
 *  [x] Create a new GPT on a disk
 *  [x] Insert a new partition in the table (n)
 *  [x] Delete a partition in the table (d)
 *  [x] Resize a partition
 *  [x] Copy/clone a partition from one disk and insert it to another (c)
 *  [x] Change partition type (and list) (t)
 *  [x] Fix partitions order (f)
 *  [x] List free unpartitioned space (F)
 *  [x] Help (m)
 *  [x] Change disk GUID (i)
 *  [x] Change partition name (L)
 *  [x] Change partition GUID (u)
 *  [x] Toggle legacy BIOS bootable (A)
 *  [x] Toggle no block IO protocol (B)
 *  [x] Toggle required partition flag (R)
 *  [x] Toggle attributes (S)
 *  [x] Customize columns to print
 *  [x] Print raw data of disklabel (D)
 *  [x] Call ioctl to re-read the partition table
 *  [ ] Automatically determine the real block size for SSDs
 *  [x] Swap partition indexes (s)
 *  [x] Change partition alignment & auto-detect (a)
 *  [x] Randomize disk's GUID and all partition's GUID (Z)
 *  [ ] Copy/clone all partitions from one disk and insert it to another (c)
