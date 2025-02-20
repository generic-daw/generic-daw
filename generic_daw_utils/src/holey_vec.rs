use std::{borrow::Borrow, ops::Index};

#[derive(Debug)]
pub struct HoleyVec<T>(Vec<Option<T>>);

impl<T> Default for HoleyVec<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T, I> Index<I> for HoleyVec<T>
where
    I: Borrow<usize>,
{
    type Output = T;

    fn index(&self, index: I) -> &Self::Output {
        self.0[*index.borrow()].as_ref().unwrap()
    }
}

impl<T, C> From<C> for HoleyVec<T>
where
    C: Into<Vec<Option<T>>>,
{
    fn from(value: C) -> Self {
        Self(value.into())
    }
}

impl<T> HoleyVec<T> {
    pub fn get<I>(&self, index: I) -> Option<&T>
    where
        I: Borrow<usize>,
    {
        self.0.get(*index.borrow()).and_then(Option::as_ref)
    }

    pub fn get_mut<I>(&mut self, index: I) -> Option<&mut T>
    where
        I: Borrow<usize>,
    {
        self.0.get_mut(*index.borrow()).and_then(Option::as_mut)
    }

    pub fn insert<I>(&mut self, index: I, elem: T) -> Option<T>
    where
        I: Borrow<usize>,
    {
        let index = *index.borrow();
        if index >= self.0.len() {
            self.0.resize_with(index + 1, || None);
        }

        self.0[index].replace(elem)
    }

    pub fn remove<I>(&mut self, index: I) -> Option<T>
    where
        I: Borrow<usize>,
    {
        self.0.get_mut(*index.borrow()).and_then(Option::take)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut().flatten()
    }
}

impl<T> HoleyVec<T>
where
    T: Eq,
{
    pub fn position(&self, item: &T) -> Option<usize> {
        self.0
            .iter()
            .enumerate()
            .filter_map(|(i, x)| Some((i, x.as_ref()?)))
            .find_map(|(i, x)| (item == x).then_some(i))
    }
}
