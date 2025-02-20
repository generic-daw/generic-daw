#[macro_export]
macro_rules! unique_id {
    ($mod_name:ident) => {
        mod $mod_name {
            use std::{
                borrow::Borrow,
                ops::Deref,
                sync::atomic::{AtomicUsize, Ordering::AcqRel},
            };

            static ID: AtomicUsize = AtomicUsize::new(0);

            #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
            pub struct Id(usize);

            impl Id {
                pub fn unique() -> Self {
                    Self(ID.fetch_add(1, AcqRel))
                }
            }

            impl Deref for Id {
                type Target = usize;

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl Borrow<usize> for Id {
                fn borrow(&self) -> &usize {
                    &**self
                }
            }
        }
    };
}
