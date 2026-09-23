use super::{traits::*, transaction::Transaction};
use crate::{DatabaseKind, DatabaseOptions, Mode, RO, RW, TableFlags, WriteMap};
use std::{
    collections::{BTreeMap, HashMap},
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
    /// Handles of the chart's tables, opened once: reopening a table by name
    /// on every operation takes libmdbx's handle mutex and searches by name.
    tables: TableHandles,
    /// Declared after `inner` so a temporary directory outlives the environment.
    folder: DbFolder,
}

/// Table handles by table name.
pub(crate) type TableHandles = HashMap<&'static str, ffi::MDBX_dbi>;

impl<E: DatabaseKind> Database<E> {
    /// The directory holding the database files.
    pub fn path(&self) -> &Path {
        self.folder.path()
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

        let read_only = matches!(options.mode, Mode::ReadOnly);
        if !read_only {
            DirBuilder::new().recursive(true).create(folder.path())?;
        }
        let inner = crate::Database::open_with_options(folder.path(), options)?;

        // Handles opened by a committed transaction stay open for the
        // environment's lifetime.
        let tables = if read_only {
            let tx = inner.begin_ro_txn()?;
            let mut tables = TableHandles::new();
            for &name in chart.keys() {
                match tx.open_table(Some(name)) {
                    Ok(table) => {
                        tables.insert(name, table.dbi());
                    }
                    // Absent from the file: operations on it report the error.
                    Err(crate::Error::NotFound) => {}
                    Err(error) => return Err(error),
                }
            }
            tx.commit()?;
            tables
        } else {
            let tx = inner.begin_rw_txn()?;
            let mut tables = TableHandles::new();
            for (&name, settings) in chart {
                let flags = if settings.dup_sort {
                    TableFlags::DUP_SORT
                } else {
                    TableFlags::default()
                };
                tables.insert(name, tx.create_table(Some(name), flags)?.dbi());
            }
            tx.commit()?;
            tables
        };

        Ok(Self {
            inner,
            tables,
            folder,
        })
    }

    /// Opens the database at `path` for reading and writing, creating it and
    /// the chart's tables if needed. `None` uses a temporary directory that is
    /// deleted with the database.
    pub fn create(path: Option<PathBuf>, chart: &DatabaseChart) -> crate::Result<Self> {
        Self::create_with_options(path, DatabaseOptions::default(), chart)
    }

    /// [Database::create] with custom options.
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

    /// Opens an existing database read-only; see [Database::open_with_options].
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
    /// Starts a read-only transaction.
    pub fn begin_read(&self) -> crate::Result<Transaction<'_, RO, E>> {
        Ok(Transaction {
            inner: self.inner.begin_ro_txn()?,
            tables: &self.tables,
        })
    }

    /// Starts a read-write transaction, waiting for any other one to finish.
    pub fn begin_readwrite(&self) -> crate::Result<Transaction<'_, RW, E>> {
        Ok(Transaction {
            inner: self.inner.begin_rw_txn()?,
            tables: &self.tables,
        })
    }
}

/// Table `T` accessed with raw byte keys and values.
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
    /// Encodes a key of `T`.
    pub fn encode_key(key: T::Key) -> crate::Result<<T::Key as Encodable>::Encoded> {
        key.encode()
    }

    /// Decodes a key of `T`.
    pub fn decode_key(encoded: &[u8]) -> crate::Result<T::Key>
    where
        T::Key: Decodable,
    {
        <T::Key as Decodable>::decode(encoded)
    }

    /// Encodes a value of `T`.
    pub fn encode_value(value: T::Value) -> crate::Result<<T::Value as Encodable>::Encoded> {
        value.encode()
    }

    /// Decodes a value of `T`.
    pub fn decode_value(encoded: &[u8]) -> crate::Result<T::Value> {
        <T::Value as Decodable>::decode(encoded)
    }

    /// Encodes a seek key of `T`.
    pub fn encode_seek_key(value: T::SeekKey) -> crate::Result<<T::SeekKey as Encodable>::Encoded> {
        value.encode()
    }
}

/// Declares a table type: `table!((Name) Key => Value)`, or
/// `table!((Name) Key [SeekKey] => Value)` with a distinct seek-key type.
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

/// Declares a `DUP_SORT` table type, like [table!] plus an optional
/// `[SeekValue]` after the value type.
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

/// How a table in a [DatabaseChart] is created.
#[derive(Clone, Debug, Default)]
pub struct TableSettings {
    /// Whether the table is `DUP_SORT`.
    pub dup_sort: bool,
}

/// Contains settings for each table in the database to be created or opened.
pub type DatabaseChart = BTreeMap<&'static str, TableSettings>;

/// The [DatabaseChart] entry of a table type: `table_info!(Name)`.
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
