//! A slot arena with typed 32-bit handles and generation checks.
//!
//! Slots are recycled through an intrusive free list, and every recycled slot
//! bumps its generation, so a handle to a removed item can never resolve to the
//! item that later occupies the same slot.

use std::fmt;
use std::marker::PhantomData;

const NONE: u32 = u32::MAX;

/// A typed handle into an [`Arena`].
pub struct Id<T> {
    index: u32,
    generation: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    pub(crate) fn new(index: u32, generation: u32) -> Self {
        Self {
            index,
            generation,
            _marker: PhantomData,
        }
    }

    /// The slot index. Stable for the lifetime of one occupant; reused after removal.
    pub fn index(self) -> u32 {
        self.index
    }

    /// The generation of the occupant this handle was made for.
    pub fn generation(self) -> u32 {
        self.generation
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.index, self.generation).cmp(&(other.index, other.generation))
    }
}

impl<T> std::hash::Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> fmt::Debug for Id<T> {
    /// Includes the generation so a stale handle cannot be mistaken for a live
    /// one in an error message.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.generation == 0 {
            write!(f, "#{}", self.index)
        } else {
            write!(f, "#{}:{}", self.index, self.generation)
        }
    }
}

#[derive(Debug, Clone)]
enum Slot<T> {
    /// Free slot. `next` chains the free list, `generation` is the generation
    /// the next occupant will receive.
    Free {
        next: u32,
        generation: u32,
    },
    Used {
        generation: u32,
        value: T,
    },
}

/// A restore point for [`Arena`], see [`Arena::checkpoint`].
#[derive(Debug, Clone)]
pub struct ArenaCheckpoint {
    slots: usize,
    len: usize,
    free: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: u32,
    len: usize,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free: NONE,
            len: 0,
        }
    }
}

impl<T> Arena<T> {
    pub fn alloc(&mut self, value: T) -> Id<T> {
        self.len += 1;
        if self.free != NONE {
            let index = self.free;
            let Slot::Free { next, generation } = self.slots[index as usize] else {
                unreachable!("free list points at an occupied slot")
            };
            self.free = next;
            self.slots[index as usize] = Slot::Used { generation, value };
            Id::new(index, generation)
        } else {
            let index = self.slots.len();
            assert!(index < NONE as usize, "arena exhausted");
            self.slots.push(Slot::Used {
                generation: 0,
                value,
            });
            Id::new(index as u32, 0)
        }
    }

    pub fn get(&self, id: Id<T>) -> Option<&T> {
        match self.slots.get(id.index as usize)? {
            Slot::Used { generation, value } if *generation == id.generation => Some(value),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        match self.slots.get_mut(id.index as usize)? {
            Slot::Used { generation, value } if *generation == id.generation => Some(value),
            _ => None,
        }
    }

    pub fn remove(&mut self, id: Id<T>) -> Option<T> {
        let slot = self.slots.get_mut(id.index as usize)?;
        let Slot::Used { generation, .. } = slot else {
            return None;
        };
        if *generation != id.generation {
            return None;
        }
        let next_generation = generation.wrapping_add(1);
        match std::mem::replace(
            slot,
            Slot::Free {
                next: self.free,
                generation: next_generation,
            },
        ) {
            Slot::Used { value, .. } => {
                self.len -= 1;
                self.free = id.index;
                Some(value)
            }
            Slot::Free { .. } => unreachable!(),
        }
    }

    /// Snapshot enough state to undo all allocations made after this point,
    /// including slots that were free and get handed out in the meantime.
    pub fn checkpoint(&self) -> ArenaCheckpoint {
        let mut free = Vec::new();
        let mut next = self.free;
        while next != NONE {
            free.push(next);
            match self.slots[next as usize] {
                Slot::Free {
                    next: following, ..
                } => next = following,
                Slot::Used { .. } => unreachable!("free list points at an occupied slot"),
            }
        }
        ArenaCheckpoint {
            slots: self.slots.len(),
            len: self.len,
            free,
        }
    }

    /// Undo everything allocated since `checkpoint`.
    pub fn rollback(&mut self, checkpoint: ArenaCheckpoint) {
        assert!(checkpoint.slots <= self.slots.len());
        self.slots.truncate(checkpoint.slots);

        // Restore the free list as it was, re-freeing the slots that were free
        // back then but have been handed out since (their generation advances,
        // so the discarded ids stay invalid).
        self.free = NONE;
        for index in checkpoint.free {
            let slot = &mut self.slots[index as usize];
            let generation = match slot {
                Slot::Used { generation, .. } => generation.wrapping_add(1),
                Slot::Free { generation, .. } => *generation,
            };
            *slot = Slot::Free {
                next: self.free,
                generation,
            };
            self.free = index;
        }
        self.len = checkpoint.len;
    }

    pub fn contains(&self, id: Id<T>) -> bool {
        self.get(id).is_some()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Live items in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> + '_ {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            let Slot::Used { generation, value } = slot else {
                return None;
            };
            Some((Id::new(index as u32, *generation), value))
        })
    }

    /// Live items in slot order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id<T>, &mut T)> + '_ {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(index, slot)| {
                let Slot::Used { generation, value } = slot else {
                    return None;
                };
                Some((Id::new(index as u32, *generation), value))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuses_freed_slots() {
        let mut a = Arena::<u32>::default();
        let x = a.alloc(1);
        let y = a.alloc(2);
        assert_eq!((x.index(), y.index()), (0, 1));
        assert_eq!(a.remove(y), Some(2));
        let z = a.alloc(3);
        assert_eq!(z.index(), 1);
        assert_eq!(a.len(), 2);
        assert_eq!(a.iter().count(), 2);
        assert_eq!(a.get(y), None);
        assert_eq!(a.get(z), Some(&3));
    }
}
