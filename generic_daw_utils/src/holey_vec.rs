use std::ops::Index;

#[derive(Debug)]
pub struct HoleyVec<T>(Vec<Option<T>>);

impl<T> Default for HoleyVec<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> Index<usize> for HoleyVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.0[index].as_ref().unwrap()
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
    pub fn get(&self, index: usize) -> Option<&T> {
        self.0.get(index).and_then(Option::as_ref)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.0.get_mut(index).and_then(Option::as_mut)
    }

    pub fn insert(&mut self, index: usize, elem: T) -> Option<T> {
        if index >= self.0.len() {
            self.0.resize_with(index + 1, || None);
        }

        self.0[index].replace(elem)
    }

    pub fn remove(&mut self, index: usize) -> Option<T> {
        self.0.get_mut(index).and_then(Option::take)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut().flatten()
    }
}
