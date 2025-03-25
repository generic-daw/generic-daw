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
        let out = self.0.get_mut(index).and_then(Option::take)?;

        if let Some(shrink) = self.0.iter().rev().position(Option::is_some) {
            self.0.truncate(self.0.len() - shrink);
        } else {
            self.0.clear();
        }

        Some(out)
    }

    #[must_use]
    pub fn entry(&mut self, index: usize) -> &mut Option<T> {
        if index >= self.0.len() {
            self.0.resize_with(index + 1, || None);
        }

        &mut self.0[index]
    }

    #[must_use]
    pub fn contains_key(&self, key: usize) -> bool {
        self.get(key).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &T)> {
        self.0
            .iter()
            .enumerate()
            .filter_map(|(i, t)| Some((i, t.as_ref()?)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut T)> {
        self.0
            .iter_mut()
            .enumerate()
            .filter_map(|(i, t)| Some((i, t.as_mut()?)))
    }

    pub fn keys(&self) -> impl Iterator<Item = usize> {
        self.0
            .iter()
            .enumerate()
            .filter_map(|(i, item)| item.as_ref().map(|_| i))
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.0.iter().flatten()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut().flatten()
    }
}

impl<T> HoleyVec<T>
where
    T: PartialEq,
{
    #[must_use]
    pub fn position(&self, item: &T) -> Option<usize> {
        self.iter().find_map(|(i, x)| (item == x).then_some(i))
    }

    #[must_use]
    pub fn contains_value(&self, value: &T) -> bool {
        self.values().any(|v| v == value)
    }
}
