#[macro_export]
macro_rules! unique_id {
    ($mod_name:ident) => {
        unique_id!($mod_name: usize);
    };
    ($mod_name:ident: $ty:ident) => {
        mod $mod_name {
            mod atomic {
                #![allow(non_camel_case_types)]

                pub(super) type u8 = std::sync::atomic::AtomicU8;
                pub(super) type u16 = std::sync::atomic::AtomicU16;
                pub(super) type u32 = std::sync::atomic::AtomicU32;
                pub(super) type u64 = std::sync::atomic::AtomicU64;
                pub(super) type usize = std::sync::atomic::AtomicUsize;

                pub(super) type i8 = std::sync::atomic::AtomicI8;
                pub(super) type i16 = std::sync::atomic::AtomicI16;
                pub(super) type i32 = std::sync::atomic::AtomicI32;
                pub(super) type i64 = std::sync::atomic::AtomicI64;
                pub(super) type isize = std::sync::atomic::AtomicIsize;
            }

            static ID: atomic::$ty = atomic::$ty::new(0);

            #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
            pub struct Id($ty);

            impl Id {
                pub fn unique() -> Self {
                    Self(
                        ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed),
                    )
                }

                pub fn is_last(self) -> bool {
                    self.0 + 1 == ID.load(::std::sync::atomic::Ordering::Relaxed)
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
