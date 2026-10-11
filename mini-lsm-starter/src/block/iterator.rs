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

use std::sync::Arc;

use crate::key::{KeySlice, KeyVec};

use super::Block;

/// Iterates on a block.
pub struct BlockIterator {
    /// The internal `Block`, wrapped by an `Arc`
    block: Arc<Block>,
    /// The current key, empty represents the iterator is invalid
    key: KeyVec,
    /// the current value range in the block.data, corresponds to the current key
    value_range: (usize, usize),
    /// Current index of the key-value pair, should be in range of [0, num_of_elements)
    idx: usize,
    /// The first key in the block
    first_key: KeyVec,
}

impl BlockIterator {
    fn new(block: Arc<Block>) -> Self {
        Self {
            block,
            key: KeyVec::new(),
            value_range: (0, 0),
            idx: 0,
            first_key: KeyVec::new(),
        }
    }

    fn extract_key(data_ref: &[u8], entry_offset: usize) -> KeySlice<'_> {
        let key_len =
            u16::from_be_bytes([data_ref[entry_offset], data_ref[entry_offset + 1]]) as usize;
        let key_offset = entry_offset + 2;
        let key_offest_end = key_offset + key_len;
        KeySlice::from_slice(&data_ref[key_offset..key_offest_end])
    }

    fn init_kv(&mut self) -> bool {
        self.key.clear();
        if self.block.offsets.is_empty() || self.idx >= self.block.offsets.len() {
            return false;
        }

        let data_ref = &(self.block.data);

        let entry_offset = self.block.offsets[self.idx] as usize;

        let key_len =
            u16::from_be_bytes([data_ref[entry_offset], data_ref[entry_offset + 1]]) as usize;

        let key_slice = BlockIterator::extract_key(data_ref, entry_offset);
        self.key.set_from_slice(key_slice);
        let value_block_offset = entry_offset + 2 + key_len;
        let value_len = u16::from_be_bytes([
            data_ref[value_block_offset],
            data_ref[value_block_offset + 1],
        ]) as usize;

        self.value_range = (value_block_offset + 2, value_block_offset + 2 + value_len);
        true
    }

    // handle empty block
    fn find_key(&self, key: KeySlice) -> Option<usize> {
        if self.block.offsets.is_empty() {
            return None;
        }

        let data_ref = &(self.block.data);
        let mut low: i16 = 0;
        let mut high: i16 = self.block.offsets.len() as i16 - 1;
        let mut res = high;
        let mut mid: i16;

        while low <= high {
            mid = (low + high) / 2;

            let candidate =
                BlockIterator::extract_key(data_ref, self.block.offsets[mid as usize] as usize);
            if candidate >= key {
                res = mid;
                high = mid - 1;
            } else {
                low = mid + 1;
            }
        }

        if res == self.block.offsets.len() as i16 {
            None
        } else {
            Some(res as usize)
        }
    }

    /// Creates a block iterator and seek to the first entry.
    pub fn create_and_seek_to_first(block: Arc<Block>) -> Self {
        let mut iter = BlockIterator::new(block);
        if iter.block.offsets.is_empty() {
            return iter;
        }

        iter.init_kv();
        iter.first_key
            .set_from_slice(BlockIterator::extract_key(&(iter.block.data), 0));
        iter
    }

    /// Creates a block iterator and seek to the first key that >= `key`.
    pub fn create_and_seek_to_key(block: Arc<Block>, key: KeySlice) -> Self {
        let mut iter = BlockIterator::new(block);
        if iter.block.offsets.is_empty() {
            return iter;
        }

        if let Some(idx) = iter.find_key(key) {
            iter.idx = idx;
            if iter.init_kv() {
                iter.first_key
                    .set_from_slice(BlockIterator::extract_key(&(iter.block.data), 0));
            } else {
                iter.key.clear();
            }
        }
        iter
    }

    /// Returns the key of the current entry.
    pub fn key(&self) -> KeySlice<'_> {
        self.key.as_key_slice()
    }

    /// Returns the value of the current entry.
    pub fn value(&self) -> &[u8] {
        &(self.block.data[self.value_range.0..self.value_range.1])
    }

    /// Returns true if the iterator is valid.
    /// Note: You may want to make use of `key`
    pub fn is_valid(&self) -> bool {
        !self.key.is_empty()
    }

    /// Seeks to the first key in the block.
    pub fn seek_to_first(&mut self) {
        self.first_key.clear();
        self.idx = 0;
        self.init_kv();
    }

    /// Move to the next key in the block.
    pub fn next(&mut self) {
        self.idx += 1;
        if self.idx >= self.block.offsets.len() {
            return;
        }
        self.init_kv();
    }

    /// Seek to the first key that >= `key`.
    /// Note: You should assume the key-value pairs in the block are sorted when being added by
    /// callers.
    pub fn seek_to_key(&mut self, key: KeySlice) {
        if let Some(idx) = self.find_key(key) {
            self.idx = idx;
            self.init_kv();
        }
    }
}
