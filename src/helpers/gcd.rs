pub fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        a %= b;
        std::mem::swap(&mut a, &mut b);
    }
    a
}
