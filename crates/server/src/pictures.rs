use std::collections::{HashMap, VecDeque};

use axum::body::Bytes;

/// What a picture is of: the receipt, how many of its events, and the scale it is drawn at.
pub type PictureKey = (u64, Option<usize>, u32);

/// The most the pictures kept take up.
const LIMIT: usize = 64 * 1024 * 1024;

/// Pictures that were drawn, so a slider that goes over a receipt draws each step once. A receipt
/// never changes, so a picture stays right until its receipt is gone.
#[derive(Debug, Default)]
pub struct Pictures {
    size: usize,
    order: VecDeque<PictureKey>,
    files: HashMap<PictureKey, Bytes>,
}

impl Pictures {
    pub fn get(&self, key: &PictureKey) -> Option<Bytes> {
        self.files.get(key).cloned()
    }

    /// Keeps a picture, and lets the oldest ones go when there are too many.
    pub fn put(&mut self, key: PictureKey, file: Bytes) {
        if file.len() > LIMIT || self.files.contains_key(&key) {
            return;
        }
        while self.size + file.len() > LIMIT {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(gone) = self.files.remove(&oldest) {
                self.size -= gone.len();
            }
        }
        self.size += file.len();
        self.order.push_back(key);
        self.files.insert(key, file);
    }

    /// Lets go of the pictures of the receipts that `gone` says are gone.
    pub fn forget(&mut self, gone: impl Fn(u64) -> bool) {
        self.order.retain(|key| !gone(key.0));
        self.files.retain(|key, file| {
            let kept = !gone(key.0);
            if !kept {
                self.size -= file.len();
            }
            kept
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_kept_until_its_receipt_is_gone() {
        let mut pictures = Pictures::default();
        pictures.put((1, None, 1), Bytes::from_static(b"one"));
        pictures.put((1, Some(2), 1), Bytes::from_static(b"two"));
        pictures.put((2, None, 1), Bytes::from_static(b"three"));
        assert_eq!(pictures.get(&(1, Some(2), 1)).as_deref(), Some(&b"two"[..]));
        pictures.forget(|id| id == 1);
        assert!(pictures.get(&(1, None, 1)).is_none());
        assert!(pictures.get(&(1, Some(2), 1)).is_none());
        assert_eq!(pictures.get(&(2, None, 1)).as_deref(), Some(&b"three"[..]));
        assert_eq!(pictures.size, 5);
    }

    #[test]
    fn the_oldest_pictures_let_go_when_there_are_too_many() {
        let mut pictures = Pictures::default();
        let big = Bytes::from(vec![0; LIMIT / 2 + 1]);
        pictures.put((1, None, 1), big.clone());
        pictures.put((2, None, 1), big.clone());
        assert!(pictures.get(&(1, None, 1)).is_none());
        assert!(pictures.get(&(2, None, 1)).is_some());
    }
}
