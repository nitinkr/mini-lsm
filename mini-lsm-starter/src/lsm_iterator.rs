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

use anyhow::{Result, bail};
use bytes::Bytes;
use std::ops::Bound::{self, Excluded, Included, Unbounded};

use crate::{
    iterators::{
        StorageIterator, merge_iterator::MergeIterator, two_merge_iterator::TwoMergeIterator,
    },
    mem_table::MemTableIterator,
    table::SsTableIterator,
};

/// Represents the internal type for an LSM iterator. This type will be changed across the course for multiple times.
type LsmIteratorInner =
    TwoMergeIterator<MergeIterator<MemTableIterator>, MergeIterator<SsTableIterator>>;

pub struct LsmIterator {
    inner: LsmIteratorInner,
    end_bound: Bound<Bytes>,
    within_bound: bool,
}

impl LsmIterator {
    pub(crate) fn new(iter: LsmIteratorInner, end_bound: Bound<Bytes>) -> Result<Self> {
        let mut s = Self {
            inner: iter,
            end_bound,
            within_bound: true,
        };
        s.init()?;
        Ok(s)
    }

    fn init(&mut self) -> Result<()> {
        while self.is_valid() && self.value().is_empty() {
            self.next()?;
        }
        // incorporate end_bound
        if self.inner.is_valid() {
            let key = self.key();
            self.within_bound = match &self.end_bound {
                Unbounded => true,
                Included(end) => key <= end,
                Excluded(end) => key < end,
            };
        } else {
            self.within_bound = false;
        }
        Ok(())
    }
}

impl StorageIterator for LsmIterator {
    type KeyType<'a> = &'a [u8];

    fn is_valid(&self) -> bool {
        self.inner.is_valid() && self.within_bound
    }

    fn key(&self) -> &[u8] {
        self.inner.key().raw_ref()
    }

    fn value(&self) -> &[u8] {
        self.inner.value()
    }

    fn next(&mut self) -> Result<()> {
        if !self.is_valid() {
            bail!("Invalid LsmIterator");
        }
        if self.inner.next().is_err() {
            bail!("Error: LsmIterator next");
        }

        while self.inner.is_valid() && self.value().is_empty() {
            if self.inner.next().is_err() {
                bail!("Error: LsmIterator next");
            }
        }

        // check and populate end_bound
        if self.is_valid() {
            let key = self.key();
            self.within_bound = match &self.end_bound {
                Unbounded => true,
                Included(end) => key <= end,
                Excluded(end) => key < end,
            };
        }
        Ok(())
    }

    fn num_active_iterators(&self) -> usize {
        self.inner.num_active_iterators()
    }
}

/// A wrapper around existing iterator, will prevent users from calling `next` when the iterator is
/// invalid. If an iterator is already invalid, `next` does not do anything. If `next` returns an error,
/// `is_valid` should return false, and `next` should always return an error.
pub struct FusedIterator<I: StorageIterator> {
    iter: I,
    has_errored: bool,
}

impl<I: StorageIterator> FusedIterator<I> {
    pub fn new(iter: I) -> Self {
        Self {
            iter,
            has_errored: false,
        }
    }
}

impl<I: StorageIterator> StorageIterator for FusedIterator<I> {
    type KeyType<'a>
        = I::KeyType<'a>
    where
        Self: 'a;

    fn is_valid(&self) -> bool {
        !self.has_errored && self.iter.is_valid()
    }

    fn key(&self) -> Self::KeyType<'_> {
        self.iter.key()
    }

    fn value(&self) -> &[u8] {
        self.iter.value()
    }

    fn next(&mut self) -> Result<()> {
        if self.has_errored {
            bail!("FusedIterator has errored");
        }

        if self.is_valid() && self.iter.next().is_err() {
            self.has_errored = true;
            bail!("FusedIterator has errored");
        }
        Ok(())
    }

    fn num_active_iterators(&self) -> usize {
        self.iter.num_active_iterators()
    }
}
