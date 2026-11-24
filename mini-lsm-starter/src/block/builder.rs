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

use crate::key::{KeySlice, KeyVec};
use bytes::BufMut;

use super::Block;

/// Builds a block.
pub struct BlockBuilder {
    /// Offsets of each key-value entries.
    offsets: Vec<u16>,
    /// All serialized key-value pairs in the block.
    data: Vec<u8>,
    /// The expected block size.
    block_size: usize,
    /// The first key in the block
    first_key: KeyVec,
}

impl BlockBuilder {
    /// Creates a new block builder.
    pub fn new(block_size: usize) -> Self {
        Self {
            offsets: vec![],
            data: vec![],
            block_size,
            first_key: KeyVec::new(),
        }
    }

    /// Adds a key-value pair to the block. Returns false when the block is full.
    /// You may find the `bytes::BufMut` trait useful for manipulating binary data.
    #[must_use]
    pub fn add(&mut self, key: KeySlice, value: &[u8]) -> bool {
        let key_len = key.len() as u16;
        let value_len = value.len() as u16;
        let overlap_len = self.overlap_len(key.raw_ref());

        if !self.has_space(key_len as usize, value_len as usize, overlap_len) {
            return false;
        }

        // add offset
        self.offsets.push(self.data.len() as u16);

        // append data
        // 1. add 2B of key len
        self.data.put_u16(overlap_len as u16);
        let rest_len = key_len - overlap_len as u16;
        self.data.put_u16(rest_len);
        let key_slice = key.raw_ref();
        let rest_key = &key_slice[overlap_len..];
        self.data.extend_from_slice(rest_key);

        self.data.put_u16(value_len);
        self.data.extend_from_slice(value);

        if self.first_key.is_empty() {
            self.first_key.append(key.raw_ref());
        }
        true
    }

    /// Check if there is no key-value pair in the block.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn size(&self) -> usize {
        if self.is_empty() {
            0
        } else {
            self.data.len() + (self.offsets.len() + 1) * 2
        }
    }

    pub fn overlap_len(&self, key: &[u8]) -> usize {
        let end = self.first_key.len().min(key.len());
        let first_key_slice = self.first_key.as_key_slice().into_inner();

        for i in 0..end {
            if first_key_slice[i] != key[i] {
                return i;
            }
        }
        end
    }

    pub fn has_space(&self, key_len: usize, value_len: usize, overlap_len: usize) -> bool {
        if self.is_empty() {
            return true;
        }
        let curr_size = self.data.len() + 2 * (self.offsets.len() + 1);
        let rest_key_footprint = 4 + (key_len - overlap_len);
        let value_footprint = 2 + value_len;
        let new_size = curr_size + rest_key_footprint + value_footprint;
        new_size <= self.block_size
    }

    /// Finalize the block.
    pub fn build(self) -> Block {
        Block {
            data: self.data.clone(),
            offsets: self.offsets.clone(),
        }
    }
}
