// Copyright (c) 2022-2026 Alex Chi Z
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(unused_variables)] // TODO(you): remove this lint after implementing this mod
#![allow(dead_code)] // TODO(you): remove this lint after implementing this mod

use anyhow::Result;

use super::StorageIterator;

/// Merges two iterators of different types into one. If the two iterators have the same key, only
/// produce the key once and prefer the entry from A.
pub struct TwoMergeIterator<A: StorageIterator, B: StorageIterator> {
    a: A,
    b: B,
    pick_from_a: bool,
    // Add fields as need
}

impl<
    A: 'static + StorageIterator,
    B: 'static + for<'a> StorageIterator<KeyType<'a> = A::KeyType<'a>>,
> TwoMergeIterator<A, B>
{
    pub fn create(a: A, b: B) -> Result<Self> {
        let mut iter = TwoMergeIterator {
            a,
            b,
            pick_from_a: true,
        };
        iter.init()?;
        Ok(iter)
    }

    fn init(&mut self) -> Result<()> {
        // calibrate
        // we don't need a while loop because keys from a given iterator can't be duplicates.
        self.pick_from_a = self.a.is_valid();
        if self.a.is_valid() && self.b.is_valid() {
            let cmp = self.a.key().cmp(&(self.b.key()));
            match cmp {
                std::cmp::Ordering::Equal => {
                    self.b.next()?;
                    self.pick_from_a = true;
                }
                std::cmp::Ordering::Less => {
                    self.pick_from_a = true;
                }
                std::cmp::Ordering::Greater => {
                    self.pick_from_a = false;
                }
            }
        }
        Ok(())
    }
}

impl<
    A: 'static + StorageIterator,
    B: 'static + for<'a> StorageIterator<KeyType<'a> = A::KeyType<'a>>,
> StorageIterator for TwoMergeIterator<A, B>
{
    type KeyType<'a> = A::KeyType<'a>;

    fn key(&self) -> Self::KeyType<'_> {
        if self.pick_from_a {
            self.a.key()
        } else {
            self.b.key()
        }
    }

    fn value(&self) -> &[u8] {
        if self.pick_from_a {
            self.a.value()
        } else {
            self.b.value()
        }
    }

    fn is_valid(&self) -> bool {
        self.a.is_valid() || self.b.is_valid()
    }

    fn next(&mut self) -> Result<()> {
        if self.is_valid() {
            if self.pick_from_a {
                self.a.next()?;
            } else {
                self.b.next()?;
            }
            self.init()?;
        }

        Ok(())
    }

    fn num_active_iterators(&self) -> usize {
        let mut res: usize = 0;
        if self.a.is_valid() {
            res += self.a.num_active_iterators();
        }
        if self.b.is_valid() {
            res += self.b.num_active_iterators();
        }
        res
    }
}
