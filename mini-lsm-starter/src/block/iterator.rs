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

    fn extract_key(&mut self, idx: usize) -> KeyVec {
        assert!(idx < self.block.offsets.len(), "index out of range");
        let key = self.block.extract_key(idx);
        let mut res_key = KeyVec::new();
        res_key.set_from_slice(key.as_key_slice());
        res_key
    }

    fn init(&mut self) {
        if self.first_key.is_empty() {
            self.first_key = self.extract_key(0);
        }

        if self.idx >= self.block.offsets.len() {
            self.key.clear();
            return;
        }

        self.key = self.extract_key(self.idx);
        let data_block = &self.block.data;
        let key_block_start = self.block.offsets[self.idx] as usize;
        let key_overlap_len =
            u16::from_be_bytes([data_block[key_block_start], data_block[key_block_start + 1]])
                as usize;
        let key_block_end = key_block_start + 4 + self.key.len() - key_overlap_len;

        let val_len =
            u16::from_be_bytes([data_block[key_block_end], data_block[key_block_end + 1]]) as usize;
        let value_begin_offset = key_block_end + 2;
        let value_end_offset = value_begin_offset + val_len;
        self.value_range = (value_begin_offset, value_end_offset);
    }

    /// Creates a block iterator and seek to the first entry.
    pub fn create_and_seek_to_first(block: Arc<Block>) -> Self {
        let mut iter = BlockIterator::new(block);
        iter.init();
        iter
    }

    /// Creates a block iterator and seek to the first key that >= `key`.
    pub fn create_and_seek_to_key(block: Arc<Block>, key: KeySlice) -> Self {
        let mut iter = BlockIterator::new(block);
        iter.seek_to_key(key);
        iter
    }

    /// Returns the key of the current entry.
    pub fn key(&self) -> KeySlice<'_> {
        println!("block/iterator::key() called");
        self.key.as_key_slice()
    }

    /// Returns the value of the current entry.
    pub fn value(&self) -> &[u8] {
        println!("block/iterator::value() called");
        &self.block.data[self.value_range.0..self.value_range.1]
    }

    /// Returns true if the iterator is valid.
    /// Note: You may want to make use of `key`
    pub fn is_valid(&self) -> bool {
        !self.key.is_empty() && self.idx < self.block.offsets.len()
    }

    /// Seeks to the first key in the block.
    pub fn seek_to_first(&mut self) {
        self.idx = 0;
        self.init();
    }

    /// Move to the next key in the block.
    pub fn next(&mut self) {
        println!("block/iterator::next() called");
        self.idx += 1;
        self.init();
    }

    /// Seek to the first key that >= `key`.
    /// Note: You should assume the key-value pairs in the block are sorted when being added by
    /// callers.
    pub fn seek_to_key(&mut self, key: KeySlice) {
        let mut low: usize = 0;
        let mut high: usize = self.block.offsets.len() - 1;
        self.idx = self.block.offsets.len();
        let mut mid: usize;
        while low <= high {
            mid = (low + high) / 2;
            let curr_key = self.extract_key(mid);
            match key.cmp(&curr_key.as_key_slice()) {
                std::cmp::Ordering::Equal => {
                    self.idx = mid;
                    break;
                }
                std::cmp::Ordering::Greater => {
                    low = mid + 1;
                }
                std::cmp::Ordering::Less => {
                    self.idx = mid;
                    if mid == 0 {
                        break;
                    }
                    high = mid - 1;
                }
            }
        }

        self.init();
    }
}
