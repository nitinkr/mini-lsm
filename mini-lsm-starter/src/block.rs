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
use bytes::{BufMut, Bytes};
pub use iterator::BlockIterator;

/// A block is the smallest unit of read and caching in LSM tree. It is a collection of sorted key-value pairs.
pub struct Block {
    pub(crate) data: Vec<u8>,
    pub(crate) offsets: Vec<u16>,
}

impl Block {
    /// Encode the internal data to the data layout illustrated in the course
    /// Note: You may want to recheck if any of the expected field is missing from your output
    pub fn encode(&self) -> Bytes {
        if self.offsets.is_empty() {
            return Bytes::new();
        }

        let mut encoded = self.data.clone();

        for o in self.offsets.iter() {
            encoded.put_u16(*o);
        }
        encoded.put_u16(self.offsets.len() as u16);
        Bytes::from(encoded)
    }

    /// Decode from the data layout, transform the input `data` to a single `Block`
    pub fn decode(data: &[u8]) -> Self {
        let mut b = Block {
            data: vec![],
            offsets: vec![],
        };
        b.populate(data);
        b
    }

    fn populate(&mut self, data: &[u8]) {
        if data.len() < 2 {
            return;
        }
        let offset_ct_bytes = &data[data.len() - 2..];
        let offset_count = u16::from_be_bytes([offset_ct_bytes[0], offset_ct_bytes[1]]);
        if offset_count == 0 {
            return;
        }

        let mut offsets_start_index = data.len() - (2 * (offset_count as usize + 1));

        for _ in 0..offset_count {
            self.offsets.push(u16::from_be_bytes([
                data[offsets_start_index],
                data[offsets_start_index + 1],
            ]));
            offsets_start_index += 2;
        }

        for &o in self.offsets.iter() {
            let idx = o as usize;
            let key_len = u16::from_be_bytes([data[idx], data[idx + 1]]);
            let val_idx = idx + 2 + key_len as usize;
            let val_len = u16::from_be_bytes([data[val_idx], data[val_idx + 1]]);
            let data_len = (key_len + val_len + 4) as usize;
            self.data.put_slice(&data[idx..idx + data_len]);
        }
    }
}
