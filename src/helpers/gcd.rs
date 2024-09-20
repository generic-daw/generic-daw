use std::ops::Rem;

pub fn gcd<T>(x: T, y: T) -> T
where
    T: Copy + PartialEq + PartialOrd + Rem<Output = T> + From<u8>,
{
    if y == 0.into() {
        x
    } else {
        let v = x % y;
        gcd(y, v)
    }
}
