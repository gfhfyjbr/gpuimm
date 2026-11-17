use std::cell::{Cell, RefCell};

pub(crate) struct Callback<T> {
    value: RefCell<Option<T>>,
    generation: Cell<u64>,
}

impl<T> Default for Callback<T> {
    fn default() -> Self {
        Self {
            value: RefCell::new(None),
            generation: Cell::new(0),
        }
    }
}

impl<T> Callback<T> {
    pub fn set(&self, value: T) {
        self.generation.set(self.generation.get().wrapping_add(1));
        self.value.replace(Some(value));
    }

    pub fn take(&self) -> Option<T> {
        self.generation.set(self.generation.get().wrapping_add(1));
        self.value.borrow_mut().take()
    }

    pub fn call<R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        let mut value = self.value.borrow_mut().take()?;
        let generation = self.generation.get();
        let result = f(&mut value);
        if generation == self.generation.get() {
            self.value.replace(Some(value));
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::Callback;

    #[test]
    fn replacement_and_withdrawal_survive_reentry() {
        let slot = Callback::default();
        slot.set(1);
        slot.call(|_| {
            assert!(slot.call(|_| ()).is_none());
            slot.set(2);
        });
        assert_eq!(slot.call(|n| *n), Some(2));
        slot.call(|_| {
            slot.take();
        });
        assert!(slot.take().is_none());
    }
}
