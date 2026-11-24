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

pub(crate) mod bloom;
mod builder;
mod iterator;

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Result, bail};
pub use builder::SsTableBuilder;
use bytes::{Buf, BufMut, Bytes};
pub use iterator::SsTableIterator;
use std::ops::Bound;

use crate::block::Block;
use crate::key::{KeyBytes, KeySlice};
use crate::lsm_storage::BlockCache;

use self::bloom::Bloom;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockMeta {
    /// Offset of this data block.
    pub offset: usize,
    /// The first key of the data block.
    pub first_key: KeyBytes,
    /// The last key of the data block.
    pub last_key: KeyBytes,
}

impl BlockMeta {
    /// Encode block meta to a buffer.
    /// You may add extra fields to the buffer,
    /// in order to help keep track of `first_key` when decoding from the same buffer in the future.
    pub fn encode_block_meta(
        block_meta: &[BlockMeta],
        #[allow(clippy::ptr_arg)] // remove this allow after you finish
        buf: &mut Vec<u8>,
    ) {
        for meta in block_meta {
            meta.encode(buf);
        }
    }

    /// Decode block meta from a buffer.
    pub fn decode_block_meta(mut buf: impl Buf) -> Vec<BlockMeta> {
        let mut metas = vec![];
        while buf.has_remaining() {
            // metas.push(BlockMeta::decode_block(&mut buf));
            let meta_offset: usize = buf.get_u32() as usize;

            // first key
            let first_key_len = buf.get_u16() as usize;
            let first_key_bytes = buf.copy_to_bytes(first_key_len);

            // last key
            let last_key_len = buf.get_u16() as usize;
            let last_key_bytes = buf.copy_to_bytes(last_key_len);

            metas.push(BlockMeta {
                offset: meta_offset,
                first_key: KeyBytes::from_bytes(first_key_bytes),
                last_key: KeyBytes::from_bytes(last_key_bytes),
            });
        }
        metas
    }

    fn encode(&self, buf: &mut Vec<u8>) {
        buf.put_u32(self.offset as u32);

        buf.put_u16(self.first_key.len() as u16);
        buf.extend_from_slice(self.first_key.raw_ref());

        buf.put_u16(self.last_key.len() as u16);
        buf.extend_from_slice(self.last_key.raw_ref());
    }
}

/// A file object.
pub struct FileObject(Option<File>, u64);

impl FileObject {
    pub fn read(&self, offset: u64, len: u64) -> Result<Vec<u8>> {
        use std::os::unix::fs::FileExt;
        let mut data = vec![0; len as usize];
        self.0
            .as_ref()
            .unwrap()
            .read_exact_at(&mut data[..], offset)?;
        Ok(data)
    }

    pub fn size(&self) -> u64 {
        self.1
    }

    /// Create a new file object (day 2) and write the file to the disk (day 4).
    pub fn create(path: &Path, data: Vec<u8>) -> Result<Self> {
        std::fs::write(path, &data)?;
        File::open(path)?.sync_all()?;
        Ok(FileObject(
            Some(File::options().read(true).write(false).open(path)?),
            data.len() as u64,
        ))
    }

    pub fn open(path: &Path) -> Result<Self> {
        let file = File::options().read(true).write(false).open(path)?;
        let size = file.metadata()?.len();
        Ok(FileObject(Some(file), size))
    }
}

/// An SSTable.
pub struct SsTable {
    /// The actual storage unit of SsTable, the format is as above.
    pub(crate) file: FileObject,
    /// The meta blocks that hold info for data blocks.
    pub(crate) block_meta: Vec<BlockMeta>,
    /// The offset that indicates the start point of meta blocks in `file`.
    pub(crate) block_meta_offset: usize,
    id: usize,
    block_cache: Option<Arc<BlockCache>>,
    first_key: KeyBytes,
    last_key: KeyBytes,
    pub(crate) bloom: Option<Bloom>,
    /// The maximum timestamp stored in this SST, implemented in week 3.
    max_ts: u64,
}

impl SsTable {
    #[cfg(test)]
    pub(crate) fn open_for_test(file: FileObject) -> Result<Self> {
        Self::open(0, Some(Arc::new(BlockCache::new(1000))), file)
    }

    /// Open SSTable from a file.
    pub fn open(id: usize, block_cache: Option<Arc<BlockCache>>, file: FileObject) -> Result<Self> {
        let mut curr_end = file.1;
        // get bloom filter up and running
        let bloom_offset =
            u32::from_be_bytes(file.read(curr_end - 4, 4)?.try_into().unwrap()) as usize;
        curr_end -= 4;
        let bloom_block_len = curr_end as usize - bloom_offset;
        let data = file.read(bloom_offset as u64, bloom_block_len as u64)?;
        let bloom = Bloom::decode(data.as_ref())?;

        curr_end = bloom_offset as u64;

        let mut sstable = SsTable {
            file,
            block_meta: vec![],
            block_meta_offset: 0,
            id,
            block_cache,
            first_key: KeyBytes::from_bytes(Bytes::new()),
            last_key: KeyBytes::from_bytes(Bytes::new()),
            bloom: Some(bloom),
            max_ts: 0,
        };

        sstable.block_meta_offset =
            u32::from_be_bytes(sstable.file.read(curr_end - 4, 4)?.try_into().unwrap()) as usize;
        curr_end -= 4;
        let meta_block_len = (curr_end as usize) - sstable.block_meta_offset;
        let meta_bytes = sstable
            .file
            .read(sstable.block_meta_offset as u64, meta_block_len as u64)?;
        sstable.block_meta = BlockMeta::decode_block_meta(meta_bytes.as_slice());

        sstable.first_key = sstable.block_meta.first().unwrap().first_key.clone();
        sstable.last_key = sstable.block_meta.last().unwrap().last_key.clone();
        Ok(sstable)
    }

    /// Create a mock SST with only first key + last key metadata
    pub fn create_meta_only(
        id: usize,
        file_size: u64,
        first_key: KeyBytes,
        last_key: KeyBytes,
    ) -> Self {
        Self {
            file: FileObject(None, file_size),
            block_meta: vec![],
            block_meta_offset: 0,
            id,
            block_cache: None,
            first_key,
            last_key,
            bloom: None,
            max_ts: 0,
        }
    }

    /// Read a block from the disk.
    pub fn read_block(&self, block_idx: usize) -> Result<Arc<Block>> {
        if block_idx >= self.block_meta.len() {
            bail!("block index out of bound");
        }
        let block_meta = &self.block_meta[block_idx];
        let block_start = block_meta.offset as u64;
        let block_end = if block_idx + 1 == self.block_meta.len() {
            self.block_meta_offset
        } else {
            self.block_meta[block_idx + 1].offset
        } as u64;
        let block = self.file.read(block_start, block_end - block_start)?;
        Ok(Arc::new(Block::decode(block.as_slice())))
    }

    /// Read a block from disk, with block cache. (Day 4)
    pub fn read_block_cached(&self, block_idx: usize) -> Result<Arc<Block>> {
        if let Ok(block) = self
            .block_cache
            .clone()
            .unwrap()
            .try_get_with((self.id, block_idx), || self.read_block(block_idx))
        {
            return Ok(block.clone());
        }
        bail!("error read_block_cached");
    }

    /// Find the block that may contain `key`.
    /// Note: You may want to make use of the `first_key` stored in `BlockMeta`.
    /// You may also assume the key-value pairs stored in each consecutive block are sorted.
    pub fn find_block_idx(&self, key: KeySlice) -> usize {
        let mut low = 0;
        let mut high = self.block_meta.len() as i32 - 1;
        let mut mid: usize;
        let mut res = self.block_meta.len();

        while low <= high {
            mid = ((low + high) / 2) as usize;
            let curr_key = self.block_meta[mid].last_key.as_key_slice();
            match key.cmp(&curr_key) {
                std::cmp::Ordering::Less => {
                    res = mid;
                    high = mid as i32 - 1;
                }
                std::cmp::Ordering::Equal => {
                    res = mid;
                    break;
                }
                std::cmp::Ordering::Greater => {
                    low = mid as i32 + 1;
                }
            }
        }

        res
    }

    /// Get number of data blocks.
    pub fn num_of_blocks(&self) -> usize {
        self.block_meta.len()
    }

    pub fn first_key(&self) -> &KeyBytes {
        &self.first_key
    }

    pub fn last_key(&self) -> &KeyBytes {
        &self.last_key
    }

    pub fn table_size(&self) -> u64 {
        self.file.1
    }

    pub fn sst_id(&self) -> usize {
        self.id
    }

    pub fn max_ts(&self) -> u64 {
        self.max_ts
    }

    pub fn key_not_included(&self, key_slice: KeySlice) -> bool {
        if key_slice < self.first_key().as_key_slice() || key_slice > self.last_key().as_key_slice()
        {
            return true;
        }
        let b = self.bloom.as_ref().unwrap();
        !b.may_contain(farmhash::fingerprint32(key_slice.raw_ref()))
    }

    pub fn range_not_included(&self, lower: Bound<&[u8]>, upper: Bound<&[u8]>) -> bool {
        let res = match lower {
            Bound::Unbounded => false,
            Bound::Included(e) => KeySlice::from_slice(e) > self.last_key().as_key_slice(),
            Bound::Excluded(e) => KeySlice::from_slice(e) >= self.last_key().as_key_slice(),
        };

        res || (match upper {
            Bound::Unbounded => false,
            Bound::Included(e) => KeySlice::from_slice(e) < self.first_key().as_key_slice(),
            Bound::Excluded(e) => KeySlice::from_slice(e) <= self.first_key().as_key_slice(),
        })
    }
}
