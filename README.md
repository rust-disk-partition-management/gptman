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
 *  [ ] Create a new GPT on a disk
 *  [x] Insert a new partition in the table (n)
 *  [x] Delete a partition in the table (d)
 *  [ ] Resize a partition
 *  [ ] Copy/clone a partition from one disk and insert it to another
 *  [ ] Copy/clone metadata of a partition from one disk to another partition
 *  [x] Change partition type (and list) (t)
 *  [ ] List free unpartitioned space (F)
 *  [ ] Help (m)
 *  [ ] Change disk GUID (i)
 *  [ ] Change partition name (L)
 *  [x] Change partition GUID (u)
 *  [ ] Toggle legacy BIOS bootable (A)
 *  [ ] Toggle no block IO protocol (B)
 *  [ ] Toggle required partition flag (R)
 *  [ ] Toggle attributes (S)
 *  [ ] Customize columns to print
 *  [ ] Print raw data from first sector (d)
 *  [ ] Print raw data of disklabel (D)
 *  [x] Call ioctl to re-read the partition table
