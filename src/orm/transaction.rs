use super::{cursor::*, database::TableHandles, impls::dec, traits::*};
use crate::{DatabaseKind, RO, RW, Stat, TransactionKind, WriteFlags, WriteMap};
use std::{collections::HashMap, marker::PhantomData};

/// A typed transaction on an ORM [Database](crate::orm::Database).
#[derive(Debug)]
pub struct Transaction<'db, K, E = WriteMap>
where
    K: TransactionKind,
    E: DatabaseKind,
{
    pub(crate) inner: crate::Transaction<'db, K, E>,
    pub(crate) tables: &'db TableHandles,
}

impl<E: DatabaseKind> Transaction<'_, RO, E> {
    /// The total size in bytes of every named table.
    pub fn table_sizes(&self) -> crate::Result<HashMap<String, u64>> {
        let mut out = HashMap::new();
        let main_table = self.inner.open_table(None)?;
        let mut cursor = self.inner.cursor(&main_table)?;
        while let Some((table, _)) = cursor.next_nodup::<Vec<u8>, ()>()? {
            let table = String::from_utf8(table).map_err(dec)?;
            let db = self.inner.open_table(Some(&table))?;
            let stats = self.inner.table_stat(&db)?;

            // Never close `db`: table handles are shared by every transaction
            // in the environment, so closing one here would invalidate it
            // under other transactions (see `close_table`'s safety contract).
            out.insert(table, stats.total_size());
        }

        Ok(out)
    }
}

impl<'db, K, E> Transaction<'db, K, E>
where
    K: TransactionKind,
    E: DatabaseKind,
{
    /// The handle of `T`: the one opened with the database for chart tables,
    /// otherwise opened by name.
    fn table<T: Table>(&self) -> crate::Result<crate::Table<'_>> {
        match self.tables.get(T::NAME) {
            Some(&dbi) => Ok(crate::Table::new_from_ptr(dbi)),
            None => self.inner.open_table(Some(T::NAME)),
        }
    }

    /// Statistics of table `T`.
    pub fn table_stat<T>(&self) -> crate::Result<Stat>
    where
        T: Table,
    {
        self.inner.table_stat(&self.table::<T>()?)
    }

    /// Opens a cursor on table `T`.
    pub fn cursor<'tx, T>(&'tx self) -> crate::Result<Cursor<'tx, K, T>>
    where
        'db: 'tx,
        T: Table,
    {
        Ok(Cursor {
            inner: self.inner.cursor(&self.table::<T>()?)?,
            _marker: PhantomData,
        })
    }

    /// Returns the value stored at `key`, or `None` when the key is absent.
    ///
    /// On `DUP_SORT` tables this returns the first duplicate of the key; use
    /// a [`Cursor`](crate::orm::Cursor) to walk the remaining duplicates.
    pub fn get<T>(&self, key: T::Key) -> crate::Result<Option<T::Value>>
    where
        T: Table,
    {
        let key = key.encode()?;
        Ok(self
            .inner
            .get::<DecodableWrapper<_>>(&self.table::<T>()?, key.as_ref())?
            .map(|v| v.0))
    }
}

impl<E: DatabaseKind> Transaction<'_, RW, E> {
    /// Upserts a value into the table.
    ///
    /// On `DUP_SORT` tables this adds a duplicate when the key exists; use
    /// `WriteFlags::UPSERT | WriteFlags::ALLDUPS` via the core API to
    /// replace all duplicates.
    pub fn upsert<T>(&self, key: T::Key, value: T::Value) -> crate::Result<()>
    where
        T: Table,
    {
        self.inner.put(
            &self.table::<T>()?,
            key.encode()?,
            value.encode()?,
            WriteFlags::UPSERT,
        )
    }

    /// Deletes `key` (only its `value` duplicate, if given); returns whether
    /// anything was deleted.
    pub fn delete<T>(&self, key: T::Key, value: Option<T::Value>) -> crate::Result<bool>
    where
        T: Table,
    {
        let value = value.map(|v| v.encode()).transpose()?;
        let mut vref = None;

        if let Some(v) = &value {
            vref = Some(v.as_ref());
        };
        self.inner.del(&self.table::<T>()?, key.encode()?, vref)
    }

    /// Removes every item from table `T`.
    pub fn clear_table<T>(&self) -> crate::Result<()>
    where
        T: Table,
    {
        self.inner.clear_table(&self.table::<T>()?)?;

        Ok(())
    }

    /// Commits the transaction.
    pub fn commit(self) -> crate::Result<()> {
        self.inner.commit()?;

        Ok(())
    }
}
