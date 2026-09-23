use super::traits::*;
use crate::{RW, TransactionKind, WriteFlags};
use std::marker::PhantomData;

#[derive(Clone, Debug)]
pub(crate) struct DecodableWrapper<T>(pub T);

impl<T> crate::Decodable<'_> for DecodableWrapper<T>
where
    T: Decodable,
{
    fn decode(data_val: &[u8]) -> Result<Self, crate::Error>
    where
        Self: Sized,
    {
        T::decode(data_val).map(Self)
    }
}

#[derive(Debug)]
pub struct Cursor<'tx, K, T>
where
    K: TransactionKind,
    T: Table,
{
    pub(crate) inner: crate::Cursor<'tx, K>,
    pub(crate) _marker: PhantomData<T>,
}

fn map_res_inner<T>(
    v: crate::Result<Option<(DecodableWrapper<T::Key>, DecodableWrapper<T::Value>)>>,
) -> crate::Result<Option<(T::Key, T::Value)>>
where
    T: Table<Key: Decodable>,
{
    Ok(v?.map(|(k, v)| (k.0, v.0)))
}

impl<K, T> Cursor<'_, K, T>
where
    K: TransactionKind,
    T: Table,
{
    pub fn first(&mut self) -> crate::Result<Option<(T::Key, T::Value)>>
    where
        T::Key: Decodable,
    {
        map_res_inner::<T>(self.inner.first())
    }

    pub fn seek_closest(&mut self, key: T::SeekKey) -> crate::Result<Option<(T::Key, T::Value)>>
    where
        T::Key: Decodable,
    {
        map_res_inner::<T>(self.inner.set_range(key.encode()?.as_ref()))
    }

    pub fn seek_exact(&mut self, key: T::Key) -> crate::Result<Option<(T::Key, T::Value)>>
    where
        T::Key: Decodable,
    {
        map_res_inner::<T>(self.inner.set_key(key.encode()?.as_ref()))
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> crate::Result<Option<(T::Key, T::Value)>>
    where
        T::Key: Decodable,
    {
        map_res_inner::<T>(self.inner.next())
    }

    pub fn prev(&mut self) -> crate::Result<Option<(T::Key, T::Value)>>
    where
        T::Key: Decodable,
    {
        map_res_inner::<T>(self.inner.prev())
    }

    pub fn last(&mut self) -> crate::Result<Option<(T::Key, T::Value)>>
    where
        T::Key: Decodable,
    {
        map_res_inner::<T>(self.inner.last())
    }

    pub fn current(&mut self) -> crate::Result<Option<(T::Key, T::Value)>>
    where
        T::Key: Decodable,
    {
        map_res_inner::<T>(self.inner.get_current())
    }

    /// Walks the table ascending starting at the closest key `>= start`
    /// (or the first key when `start` is `None`).
    pub fn walk(
        self,
        start: Option<T::SeekKey>,
    ) -> impl Iterator<Item = crate::Result<(T::Key, T::Value)>>
    where
        T: Table<Key: Decodable>,
    {
        let inner = match start {
            // Encode is fallible (user serialization): capture the error in
            // the IntoIter::Err variant — NEVER unwrap user-controlled encode.
            Some(key) => match key.encode() {
                Ok(k) => self
                    .inner
                    .into_iter_from::<DecodableWrapper<T::Key>, DecodableWrapper<T::Value>>(
                        k.as_ref(),
                    ),
                Err(e) => crate::IntoIter::failed(self.inner, e),
            },
            None => self.inner.into_iter_start(),
        };
        inner.map(decode_pair::<T>)
    }

    /// Walks the table descending starting at the closest key `<= bound`,
    /// inclusive (or the last key when `bound` is `None`).
    ///
    /// For DUPSORT tables the bound is key-level: iteration starts at the
    /// largest key `<= bound`, and that key's duplicates are yielded before
    /// moving on to the previous key.
    pub fn walk_back(
        self,
        bound: Option<T::SeekKey>,
    ) -> impl Iterator<Item = crate::Result<(T::Key, T::Value)>>
    where
        T: Table<Key: Decodable>,
    {
        let inner = match bound {
            Some(key) => match key.encode() {
                Ok(k) => self
                    .inner
                    .into_iter_back_from::<DecodableWrapper<T::Key>, DecodableWrapper<T::Value>>(
                        k.as_ref(),
                    ),
                Err(e) => crate::IntoIter::failed(self.inner, e),
            },
            None => self.inner.into_iter_back_start(),
        };
        inner.map(decode_pair::<T>)
    }
}

fn decode_pair<T>(
    kv: crate::Result<(DecodableWrapper<T::Key>, DecodableWrapper<T::Value>)>,
) -> crate::Result<(T::Key, T::Value)>
where
    T: Table<Key: Decodable>,
{
    kv.map(|(k, v)| (k.0, v.0))
}

impl<K, T> Cursor<'_, K, T>
where
    K: TransactionKind,
    T: DupSort,
{
    pub fn seek_value(
        &mut self,
        key: T::Key,
        seek_value: T::SeekValue,
    ) -> crate::Result<Option<T::Value>> {
        let res = self.inner.get_both_range::<DecodableWrapper<T::Value>>(
            key.encode()?.as_ref(),
            seek_value.encode()?.as_ref(),
        )?;

        if let Some(v) = res {
            return Ok(Some(v.0));
        }

        Ok(None)
    }

    /// Returns the last duplicate of the key the cursor is currently
    /// positioned at; requires a positioned cursor (e.g. after
    /// [`seek_exact`](Self::seek_exact) or [`first`](Self::first)).
    pub fn last_value(&mut self) -> crate::Result<Option<T::Value>> {
        Ok(self
            .inner
            .last_dup::<DecodableWrapper<T::Value>>()?
            .map(|v| v.0))
    }

    pub fn next_key(&mut self) -> crate::Result<Option<(T::Key, T::Value)>>
    where
        T::Key: Decodable,
    {
        map_res_inner::<T>(self.inner.next_nodup())
    }

    pub fn next_value(&mut self) -> crate::Result<Option<(T::Key, T::Value)>>
    where
        T::Key: Decodable,
    {
        map_res_inner::<T>(self.inner.next_dup())
    }

    pub fn prev_key(&mut self) -> crate::Result<Option<(T::Key, T::Value)>>
    where
        T::Key: Decodable,
    {
        map_res_inner::<T>(self.inner.prev_nodup())
    }

    pub fn prev_value(&mut self) -> crate::Result<Option<(T::Key, T::Value)>>
    where
        T::Key: Decodable,
    {
        map_res_inner::<T>(self.inner.prev_dup())
    }

    pub fn walk_key(
        self,
        start: T::Key,
        seek_value: Option<T::SeekValue>,
    ) -> impl Iterator<Item = crate::Result<T::Value>>
    where
        T::Key: Clone + Decodable,
    {
        struct I<'tx, K, T>
        where
            K: TransactionKind,
            T: DupSort<Key: Clone + Decodable>,
        {
            cursor: Cursor<'tx, K, T>,
            start: Option<T::Key>,
            seek_value: Option<T::SeekValue>,

            first: bool,
        }

        impl<K, T> Iterator for I<'_, K, T>
        where
            K: TransactionKind,
            T: DupSort<Key: Clone + Decodable>,
        {
            type Item = crate::Result<T::Value>;

            fn next(&mut self) -> Option<Self::Item> {
                if self.first {
                    self.first = false;
                    let start_key = self.start.take().unwrap();
                    if let Some(seek_both_key) = self.seek_value.take() {
                        self.cursor.seek_value(start_key, seek_both_key)
                    } else {
                        self.cursor.seek_exact(start_key).map(|v| v.map(|(_, v)| v))
                    }
                } else {
                    self.cursor.next_value().map(|v| v.map(|(_, v)| v))
                }
                .transpose()
            }
        }

        I {
            cursor: self,
            start: Some(start),
            seek_value,
            first: true,
        }
    }
}

impl<T> Cursor<'_, RW, T>
where
    T: Table,
{
    pub fn upsert(&mut self, key: T::Key, value: T::Value) -> crate::Result<()> {
        self.inner.put(
            key.encode()?.as_ref(),
            value.encode()?.as_ref(),
            WriteFlags::UPSERT,
        )
    }

    pub fn append(&mut self, key: T::Key, value: T::Value) -> crate::Result<()> {
        self.inner.put(
            key.encode()?.as_ref(),
            value.encode()?.as_ref(),
            WriteFlags::APPEND,
        )
    }

    pub fn delete_current(&mut self) -> crate::Result<()> {
        self.inner.del(WriteFlags::CURRENT)?;

        Ok(())
    }
}

impl<T> Cursor<'_, RW, T>
where
    T: DupSort,
{
    pub fn delete_current_key(&mut self) -> crate::Result<()> {
        self.inner.del(WriteFlags::ALLDUPS)
    }
    pub fn append_value(&mut self, key: T::Key, value: T::Value) -> crate::Result<()> {
        self.inner.put(
            key.encode()?.as_ref(),
            value.encode()?.as_ref(),
            WriteFlags::APPEND_DUP,
        )
    }
}
