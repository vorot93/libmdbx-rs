use super::{cursor::*, traits::*};
use crate::{DatabaseKind, NoWriteMap, Stat, TransactionKind, WriteFlags, WriteMap, RO, RW};
use anyhow::Context;
use std::{collections::HashMap, marker::PhantomData};

#[derive(Debug)]
pub struct Transaction<'db, TK, DK = WriteMap>
where
    TK: TransactionKind,
    DK: DatabaseKind,
{
    pub(crate) inner: crate::Transaction<'db, TK, DK>,
}

impl<'db, DK> Transaction<'db, RO, DK>
where
    DK: DatabaseKind,
{
    pub fn table_sizes(&self) -> anyhow::Result<HashMap<String, u64>> {
        let mut out = HashMap::new();
        let main_table = self.inner.open_table(None)?;
        let mut cursor = self.inner.cursor(&main_table)?;
        while let Some((table, _)) = cursor.next_nodup::<Vec<u8>, ()>()? {
            let table = String::from_utf8(table)?;
            let db = self
                .inner
                .open_table(Some(&table))
                .with_context(|| format!("failed to open table: {table}"))?;
            let stats = self
                .inner
                .table_stat(&db)
                .with_context(|| format!("failed to get stats for table: {table}"))?;

            out.insert(table, stats.total_size());

            unsafe {
                self.inner.close_table(db)?;
            }
        }

        Ok(out)
    }
}

impl<'db, TK, DK> Transaction<'db, TK, DK>
where
    TK: TransactionKind,
    DK: DatabaseKind,
{
    pub fn table_stat<T>(&self) -> Result<Stat, crate::Error>
    where
        T: Table,
    {
        self.inner
            .table_stat(&self.inner.open_table(Some(T::NAME))?)
    }

    pub fn cursor<'tx, T>(&'tx self) -> anyhow::Result<Cursor<'tx, TK, T>>
    where
        'db: 'tx,
        T: Table,
    {
        Ok(Cursor {
            inner: self.inner.cursor(&self.inner.open_table(Some(T::NAME))?)?,
            _marker: PhantomData,
        })
    }

    pub fn get<T>(&self, key: T::Key) -> anyhow::Result<Option<T::Value>>
    where
        T: Table,
    {
        Ok(self
            .inner
            .get::<DecodableWrapper<_>>(
                &self.inner.open_table(Some(T::NAME))?,
                key.encode().as_ref(),
            )?
            .map(|v| v.0))
    }
}

impl<'db, DK> Transaction<'db, RW, DK>
where
    DK: DatabaseKind,
{
    pub fn upsert<T>(&self, key: T::Key, value: T::Value) -> anyhow::Result<()>
    where
        T: Table,
    {
        Ok(self.inner.put(
            &self.inner.open_table(Some(T::NAME))?,
            &key.encode(),
            &value.encode(),
            WriteFlags::UPSERT,
        )?)
    }

    pub fn delete<T>(&self, key: T::Key, value: Option<T::Value>) -> anyhow::Result<bool>
    where
        T: Table,
    {
        let mut vref = None;
        let value = value.map(Encodable::encode);

        if let Some(v) = &value {
            vref = Some(v.as_ref());
        };
        Ok(self
            .inner
            .del(&self.inner.open_table(Some(T::NAME))?, key.encode(), vref)?)
    }

    pub fn clear_table<T>(&self) -> anyhow::Result<()>
    where
        T: Table,
    {
        self.inner
            .clear_table(&self.inner.open_table(Some(T::NAME))?)?;

        Ok(())
    }

    pub fn commit(self) -> anyhow::Result<()> {
        self.inner.commit()?;

        Ok(())
    }
}

impl<'db> Transaction<'db, RW, NoWriteMap> {
    /// Begins a new nested transaction inside of this transaction.
    pub fn begin_nested_txn(&mut self) -> anyhow::Result<Transaction<'_, RW, NoWriteMap>> {
        Ok(Transaction {
            inner: self.inner.begin_nested_txn()?,
        })
    }
}
