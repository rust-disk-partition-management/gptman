extern crate bincode;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate crc;

mod gpt;

use self::gpt::*;
use std::fs;

fn inspect_disk() {
    let mut f = fs::File::open("disk1.img").unwrap();
    let gpt = GPT::find_from(&mut f).unwrap();

    println!("GPT header: {:#?}", gpt.header);
    println!(
        "Partitions: {:#?}",
        gpt.partitions
            .iter()
            .filter(|x| x.is_used())
            .collect::<Vec<_>>()
    );
}

fn main() {
    println!("Hello, world!");
    inspect_disk();
}
