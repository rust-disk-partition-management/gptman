use crate::gpt::GPT;

pub fn fix_partitions_order(gpt: &mut GPT) {
    gpt.sort();
}
