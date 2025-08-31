#[macro_export]
macro_rules! unique_id {
    ($mod_name:ident) => {
        unique_id!($mod_name: usize);
    };
    ($mod_name:ident: $ty:ident) => {
        mod $mod_name {
            mod atomic {
                #![allow(non_camel_case_types)]

                pub type u8 = std::sync::atomic::AtomicU8;
                pub type u16 = std::sync::atomic::AtomicU16;
                pub type u32 = std::sync::atomic::AtomicU32;
                pub type u64 = std::sync::atomic::AtomicU64;
                pub type usize = std::sync::atomic::AtomicUsize;

                pub type i8 = std::sync::atomic::AtomicI8;
                pub type i16 = std::sync::atomic::AtomicI16;
                pub type i32 = std::sync::atomic::AtomicI32;
                pub type i64 = std::sync::atomic::AtomicI64;
                pub type isize = std::sync::atomic::AtomicIsize;
            }

            static ID: atomic::$ty = atomic::$ty::new($ty::MIN);

            #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
            pub struct Id($ty);

            impl Id {
                pub fn unique() -> Self {
                    Self(
                        ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed),
                    )
                }

                pub fn is_last(self) -> bool {
                    self.0.wrapping_add(1) == ID.load(::std::sync::atomic::Ordering::Relaxed)
                }
            }

            impl ::std::convert::AsRef<$ty> for Id {
                fn as_ref(&self) -> &$ty {
                    &self.0
                }
            }

            impl ::std::borrow::Borrow<$ty> for Id {
                fn borrow(&self) -> &$ty {
                    &self.0
                }
            }

            impl ::std::ops::Deref for Id {
                type Target = $ty;

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }
        }
    };
}
