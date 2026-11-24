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

mod builder;
mod iterator;

pub use builder::BlockBuilder;
use bytes::{BufMut, Bytes, BytesMut};
pub use iterator::BlockIterator;

use crate::key::KeyBytes;

/// A block is the smallest unit of read and caching in LSM tree. It is a collection of sorted key-value pairs.
pub struct Block {
    pub(crate) data: Vec<u8>,
    pub(crate) offsets: Vec<u16>,
}

impl Block {
    /// Encode the internal data to the data layout illustrated in the course
    /// Note: You may want to recheck if any of the expected field is missing from your output
    pub fn encode(&self) -> Bytes {
        let mut buff: BytesMut = BytesMut::new();
        buff.extend_from_slice(&self.data);
        for &o in &self.offsets {
            buff.put_u16(o);
        }
        buff.put_u16(self.offsets.len() as u16);
        buff.freeze()
    }

    /// Decode from the data layout, transform the input `data` to a single `Block`
    pub fn decode(data: &[u8]) -> Self {
        let mut block = Block {
            data: vec![],
            offsets: vec![],
        };

        if data.is_empty() {
            return block;
        }
        if data.len() < 2 {
            eprintln!("unexpcted size of data");
            return block;
        }

        // extract size
        let entry_count = u16::from_be_bytes([data[data.len() - 2], data[data.len() - 1]]);
        let footer_start = data.len() - (2 * (entry_count as usize + 1));
        let mut offset_pos = footer_start;
        for _ in 0..entry_count {
            block
                .offsets
                .push(u16::from_be_bytes([data[offset_pos], data[offset_pos + 1]]));
            offset_pos += 2;
        }

        for id in 0..entry_count - 1 {
            let start = block.offsets[id as usize] as usize;
            let end = block.offsets[id as usize + 1] as usize;
            block.data.extend_from_slice(&data[start..end]);
        }

        // handle last key seperately
        block.data.extend_from_slice(
            &data[(block.offsets[block.offsets.len() - 1] as usize)..footer_start],
        );
        block
    }

    pub fn extract_key(&self, idx: usize) -> KeyBytes {
        assert!(idx < self.offsets.len(), "idx out of bound");
        let mut offset = self.offsets[idx] as usize;

        let key_overlap_len =
            u16::from_be_bytes([self.data[offset], self.data[offset + 1]]) as usize;

        offset += 2;

        let key_len = u16::from_be_bytes([self.data[offset], self.data[offset + 1]]) as usize;
        offset += 2;

        let key_rest_bytes = &self.data[offset..offset + key_len];
        let mut key_bytes = BytesMut::new();
        if key_overlap_len != 0 {
            let first_key_offset = self.offsets[0] as usize + 4;
            let key_overlap_byets =
                &self.data[first_key_offset..first_key_offset + key_overlap_len];
            key_bytes.extend_from_slice(key_overlap_byets);
        }
        key_bytes.extend_from_slice(key_rest_bytes);
        KeyBytes::from_bytes(key_bytes.freeze())
    }
}
