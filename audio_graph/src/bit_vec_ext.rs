use bit_vec::BitVec;

pub trait BitVecExt {
    fn iter_ones(&self) -> impl Iterator<Item = usize>;
}

impl BitVecExt for BitVec {
    fn iter_ones(&self) -> impl Iterator<Item = usize> {
        self.iter()
            .enumerate()
            .filter_map(|(i, e)| if e { Some(i) } else { None })
    }
}
