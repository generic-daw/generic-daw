/// Stein's algorithm <https://en.wikipedia.org/wiki/Binary_GCD_algorithm>
pub fn gcd(a: u32, b: u32) -> u32 {
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }

    let mut a = i64::from(a);
    let mut b = i64::from(b);

    let mut az = a.trailing_zeros();
    let bz = b.trailing_zeros();
    let shift = a.min(b);
    b >>= bz;

    while a != 0 {
        a >>= az;
        let diff = b - a;
        az = diff.trailing_zeros();
        b = a.min(b);
        a = diff.abs();
    }

    (b << shift) as u32
}
