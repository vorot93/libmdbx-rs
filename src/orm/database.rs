use super::{traits::*, transaction::Transaction};
use crate::{DatabaseKind, DatabaseOptions, Mode, RO, RW, TableFlags, WriteMap};
use std::{
    collections::BTreeMap,
    fs::DirBuilder,
    ops::Deref,
    path::{Path, PathBuf},
};
use tempfile::tempdir;

#[derive(Debug)]
enum DbFolder {
    Persisted(std::path::PathBuf),
    Temporary(tempfile::TempDir),
}

impl DbFolder {
    fn path(&self) -> &Path {
        match self {
            Self::Persisted(p) => p.as_path(),
            Self::Temporary(temp_dir) => temp_dir.path(),
        }
    }
}

/// Typed database handle.
///
/// The type parameter selects the database kind (see
/// [`DatabaseKind`](crate::DatabaseKind)); it defaults to
/// [`WriteMap`] so writes modify the database directly in mapped memory and
/// flush to disk with a single system call. Use `Database<NoWriteMap>` to
/// instead stock modified pages in memory and write them to disk through file
/// operations.
#[derive(Debug)]
pub struct Database<E: DatabaseKind = WriteMap> {
    inner: crate::Database<E>,
    folder: DbFolder,
}

impl<E: DatabaseKind> Database<E> {
    pub fn path(&self) -> &Path {
        self.folder.path()
    }

    fn open_db(folder: DbFolder, options: DatabaseOptions) -> crate::Result<Self> {
        Ok(Self {
            inner: crate::Database::open_with_options(folder.path(), options)?,
            folder,
        })
    }

    fn new(
        folder: DbFolder,
        mut options: DatabaseOptions,
        chart: &DatabaseChart,
    ) -> crate::Result<Self> {
        // The chart's tables must always fit; never clobber a larger
        // user-provided value, and always allow at least one named table.
        options.max_tables = Some(
            chart
                .len()
                .max(options.max_tables.unwrap_or(0) as usize)
                .max(1) as u64,
        );

        if let Mode::ReadOnly = options.mode {
            Self::open_db(folder, options)
        } else {
            let _ = DirBuilder::new().recursive(true).create(folder.path());

            let this = Self::open_db(folder, options)?;

            let tx = this.inner.begin_rw_txn()?;
            for (table, settings) in chart {
                tx.create_table(
                    Some(table),
                    if settings.dup_sort {
                        TableFlags::DUP_SORT
                    } else {
                        TableFlags::default()
                    },
                )?;
            }
            tx.commit()?;

            Ok(this)
        }
    }

    pub fn create(path: Option<PathBuf>, chart: &DatabaseChart) -> crate::Result<Self> {
        Self::create_with_options(path, DatabaseOptions::default(), chart)
    }

    pub fn create_with_options(
        path: Option<PathBuf>,
        options: DatabaseOptions,
        chart: &DatabaseChart,
    ) -> crate::Result<Self> {
        let folder = if let Some(path) = path {
            DbFolder::Persisted(path)
        } else {
            let path = tempdir()?;
            DbFolder::Temporary(path)
        };

        Self::new(folder, options, chart)
    }

    pub fn open(path: impl AsRef<Path>, chart: &DatabaseChart) -> crate::Result<Self> {
        Self::open_with_options(path, DatabaseOptions::default(), chart)
    }

    /// Opens an existing database for reading.
    ///
    /// Forces read-only mode; use `create_with_options` with
    /// [`Mode::ReadWrite`] to open an existing database for writing.
    pub fn open_with_options(
        path: impl AsRef<Path>,
        mut options: DatabaseOptions,
        chart: &DatabaseChart,
    ) -> crate::Result<Self> {
        options.mode = Mode::ReadOnly;

        Self::new(
            DbFolder::Persisted(path.as_ref().to_path_buf()),
            options,
            chart,
        )
    }
}

impl<E: DatabaseKind> Deref for Database<E> {
    type Target = crate::Database<E>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<E: DatabaseKind> Database<E> {
    pub fn begin_read(&self) -> crate::Result<Transaction<'_, RO, E>> {
        Ok(Transaction {
            inner: self.inner.begin_ro_txn()?,
        })
    }

    pub fn begin_readwrite(&self) -> crate::Result<Transaction<'_, RW, E>> {
        Ok(Transaction {
            inner: self.inner.begin_rw_txn()?,
        })
    }
}

#[derive(Debug)]
pub struct UntypedTable<T>(pub T)
where
    T: Table;

impl<T> Table for UntypedTable<T>
where
    T: Table,
{
    const NAME: &'static str = T::NAME;

    type Key = Vec<u8>;
    type Value = Vec<u8>;
    type SeekKey = Vec<u8>;
}

impl<T> UntypedTable<T>
where
    T: Table,
{
    pub fn encode_key(key: T::Key) -> crate::Result<<T::Key as Encodable>::Encoded> {
        key.encode()
    }

    pub fn decode_key(encoded: &[u8]) -> crate::Result<T::Key>
    where
        T::Key: Decodable,
    {
        <T::Key as Decodable>::decode(encoded)
    }

    pub fn encode_value(value: T::Value) -> crate::Result<<T::Value as Encodable>::Encoded> {
        value.encode()
    }

    pub fn decode_value(encoded: &[u8]) -> crate::Result<T::Value> {
        <T::Value as Decodable>::decode(encoded)
    }

    pub fn encode_seek_key(value: T::SeekKey) -> crate::Result<<T::SeekKey as Encodable>::Encoded> {
        value.encode()
    }
}

#[macro_export]
macro_rules! table {
    ($(#[$docs:meta])* ( $name:ident ) $key:ty [ $seek_key:ty ] => $value:ty) => {
        $(#[$docs])*
        ///
        #[doc = concat!("Takes [`", stringify!($key), "`] as a key and returns [`", stringify!($value), "`]")]
        #[derive(Clone, Copy, Debug, Default)]
        pub struct $name;

        impl $crate::orm::Table for $name {
            const NAME: &'static str = stringify!($name);

            type Key = $key;
            type SeekKey = $seek_key;
            type Value = $value;
        }

        impl $name {
            pub const fn untyped(self) -> $crate::orm::UntypedTable<Self> {
                $crate::orm::UntypedTable(self)
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::write!(f, "{}", <Self as $crate::orm::Table>::NAME)
            }
        }
    };
    ($(#[$docs:meta])* ( $name:ident ) $key:ty => $value:ty) => {
        $crate::table!(
            $(#[$docs])*
            ( $name ) $key [ $key ] => $value
        );
    };
}

#[macro_export]
macro_rules! dupsort {
    ($(#[$docs:meta])* ( $table_name:ident ) $key:ty [$seek_key:ty] => $value:ty [$seek_value:ty] ) => {
        $crate::table!(
            $(#[$docs])*
            ///
            #[doc = concat!("`DUPSORT` table with seek value type being: [`", stringify!($seek_value), "`].")]
            ( $table_name ) $key [$seek_key] => $value
        );
        impl $crate::orm::DupSort for $table_name {
            type SeekValue = $seek_value;
        }
    };

    ($(#[$docs:meta])* ( $table_name:ident ) $key:ty [$seek_key:ty] => $value:ty ) => {
        $crate::dupsort!(
            $(#[$docs])*
            ( $table_name ) $key [$seek_key] => $value [$value]
        );
    };

    ($(#[$docs:meta])* ( $table_name:ident ) $key:ty => $value:ty [$seek_value:ty] ) => {
        $crate::dupsort!(
            $(#[$docs])*
            ( $table_name ) $key [$key] => $value [$seek_value]
        );
    };

    ($(#[$docs:meta])* ( $table_name:ident ) $key:ty => $value:ty ) => {
        $crate::dupsort!(
            $(#[$docs])*
            ( $table_name ) $key [$key] => $value [$value]
        );
    };
}

#[derive(Clone, Debug, Default)]
pub struct TableSettings {
    pub dup_sort: bool,
}

/// Contains settings for each table in the database to be created or opened.
pub type DatabaseChart = BTreeMap<&'static str, TableSettings>;

#[macro_export]
macro_rules! table_info {
    ($t:ty) => {
        (
            <$t as $crate::orm::Table>::NAME,
            $crate::orm::TableSettings {
                dup_sort: $crate::impls::impls!($t: $crate::orm::DupSort),
            },
        )
    };
}
