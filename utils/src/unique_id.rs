#[macro_export]
macro_rules! unique_id {
    ($mod_name:ident) => {
        unique_id!($mod_name: usize);
    };
    ($mod_name:ident: $ty:ident) => {
        mod $mod_name {
            mod atomic {
                #![allow(non_camel_case_types)]

                pub type u8 = ::core::sync::atomic::AtomicU8;
                pub type u16 = ::core::sync::atomic::AtomicU16;
                pub type u32 = ::core::sync::atomic::AtomicU32;
                pub type u64 = ::core::sync::atomic::AtomicU64;
                pub type usize = ::core::sync::atomic::AtomicUsize;

                pub type i8 = ::core::sync::atomic::AtomicI8;
                pub type i16 = ::core::sync::atomic::AtomicI16;
                pub type i32 = ::core::sync::atomic::AtomicI32;
                pub type i64 = ::core::sync::atomic::AtomicI64;
                pub type isize = ::core::sync::atomic::AtomicIsize;
            }

            static ID: atomic::$ty = atomic::$ty::new($ty::MIN);

            #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct Id($ty);

            impl Id {
                pub fn unique() -> Self {
                    Self(
                        ID.fetch_add(1, ::core::sync::atomic::Ordering::Relaxed),
                    )
                }
            }
        }
    };
}
