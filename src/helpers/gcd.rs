pub fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        a %= b;
        (a, b) = (b, a);
    }
    a
}
