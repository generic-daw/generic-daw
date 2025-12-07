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

            #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
            pub struct Id($ty);

            impl Id {
                pub fn unique() -> Self {
                    Self(
                        ID.fetch_add(1, ::core::sync::atomic::Ordering::Relaxed),
                    )
                }

                pub fn is_latest(self) -> bool {
                    self.0.wrapping_add(1) == ID.load(::core::sync::atomic::Ordering::Relaxed)
                }
            }

            impl ::core::convert::AsRef<$ty> for Id {
                fn as_ref(&self) -> &$ty {
                    &self.0
                }
            }

            impl ::core::borrow::Borrow<$ty> for Id {
                fn borrow(&self) -> &$ty {
                    &self.0
                }
            }

            impl ::core::ops::Deref for Id {
                type Target = $ty;

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }
        }
    };
}
