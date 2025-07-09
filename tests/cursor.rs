use libmdbx::*;
use std::borrow::Cow;
use tempfile::tempdir;

type Database = libmdbx::Database<NoWriteMap>;

#[test]
fn test_get() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    let txn = db.begin_rw_txn().unwrap();
    let table = txn.open_table(None).unwrap();

    assert_eq!(None, txn.cursor(&table).unwrap().first::<(), ()>().unwrap());

    for (k, v) in [(b"key1", b"val1"), (b"key2", b"val2"), (b"key3", b"val3")] {
        txn.put(&table, k, v, WriteFlags::empty()).unwrap();
    }

    let mut cursor = txn.cursor(&table).unwrap();
    assert_eq!(cursor.first().unwrap(), Some((*b"key1", *b"val1")));
    assert_eq!(cursor.get_current().unwrap(), Some((*b"key1", *b"val1")));
    assert_eq!(cursor.next().unwrap(), Some((*b"key2", *b"val2")));
    assert_eq!(cursor.prev().unwrap(), Some((*b"key1", *b"val1")));
    assert_eq!(cursor.last().unwrap(), Some((*b"key3", *b"val3")));
    assert_eq!(cursor.set(b"key1").unwrap(), Some(*b"val1"));
    assert_eq!(cursor.set_key(b"key3").unwrap(), Some((*b"key3", *b"val3")));
    assert_eq!(
        cursor.set_range(b"key2\0").unwrap(),
        Some((*b"key3", *b"val3"))
    );
}

#[test]
fn test_get_dup() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    let txn = db.begin_rw_txn().unwrap();
    let table = txn.create_table(None, TableFlags::DUP_SORT).unwrap();
    for (k, v) in [
        (b"key1", b"val1"),
        (b"key1", b"val2"),
        (b"key1", b"val3"),
        (b"key2", b"val1"),
        (b"key2", b"val2"),
        (b"key2", b"val3"),
    ] {
        txn.put(&table, k, v, WriteFlags::empty()).unwrap();
    }

    let mut cursor = txn.cursor(&table).unwrap();
    assert_eq!(cursor.first().unwrap(), Some((*b"key1", *b"val1")));
    assert_eq!(cursor.first_dup().unwrap(), Some(*b"val1"));
    assert_eq!(cursor.get_current().unwrap(), Some((*b"key1", *b"val1")));
    assert_eq!(cursor.next_nodup().unwrap(), Some((*b"key2", *b"val1")));
    assert_eq!(cursor.next().unwrap(), Some((*b"key2", *b"val2")));
    assert_eq!(cursor.prev().unwrap(), Some((*b"key2", *b"val1")));
    assert_eq!(cursor.next_dup().unwrap(), Some((*b"key2", *b"val2")));
    assert_eq!(cursor.next_dup().unwrap(), Some((*b"key2", *b"val3")));
    assert_eq!(cursor.next_dup::<(), ()>().unwrap(), None);
    assert_eq!(cursor.prev_dup().unwrap(), Some((*b"key2", *b"val2")));
    assert_eq!(cursor.last_dup().unwrap(), Some(*b"val3"));
    assert_eq!(cursor.prev_nodup().unwrap(), Some((*b"key1", *b"val3")));
    assert_eq!(cursor.next_dup::<(), ()>().unwrap(), None);
    assert_eq!(cursor.set(b"key1").unwrap(), Some(*b"val1"));
    assert_eq!(cursor.set(b"key2").unwrap(), Some(*b"val1"));
    assert_eq!(
        cursor.set_range(b"key1\0").unwrap(),
        Some((*b"key2", *b"val1"))
    );
    assert_eq!(cursor.get_both(b"key1", b"val3").unwrap(), Some(*b"val3"));
    assert_eq!(cursor.get_both_range::<()>(b"key1", b"val4").unwrap(), None);
    assert_eq!(
        cursor.get_both_range(b"key2", b"val").unwrap(),
        Some(*b"val1")
    );

    for kv in [
        (*b"key2", *b"val3"),
        (*b"key2", *b"val2"),
        (*b"key2", *b"val1"),
        (*b"key1", *b"val3"),
    ] {
        assert_eq!(cursor.last().unwrap(), Some(kv));
        cursor.del(WriteFlags::empty()).unwrap();
    }
}

#[test]
fn test_get_dupfixed() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    let txn = db.begin_rw_txn().unwrap();
    let table = txn
        .create_table(None, TableFlags::DUP_SORT | TableFlags::DUP_FIXED)
        .unwrap();
    for (k, v) in [
        (b"key1", b"val1"),
        (b"key1", b"val2"),
        (b"key1", b"val3"),
        (b"key2", b"val1"),
        (b"key2", b"val2"),
        (b"key2", b"val3"),
    ] {
        txn.put(&table, k, v, WriteFlags::empty()).unwrap();
    }

    let mut cursor = txn.cursor(&table).unwrap();
    assert_eq!(cursor.first().unwrap(), Some((*b"key1", *b"val1")));
    assert_eq!(cursor.get_multiple().unwrap(), Some(*b"val1val2val3"));
    assert_eq!(cursor.next_multiple::<(), ()>().unwrap(), None);
}

#[test]
fn test_iter() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    let items = vec![
        (*b"key1", *b"val1"),
        (*b"key2", *b"val2"),
        (*b"key3", *b"val3"),
        (*b"key5", *b"val5"),
    ];

    {
        let txn = db.begin_rw_txn().unwrap();
        let table = txn.open_table(None).unwrap();
        for (key, data) in &items {
            txn.put(&table, key, data, WriteFlags::empty()).unwrap();
        }
        assert!(!txn.commit().unwrap());
    }

    let txn = db.begin_ro_txn().unwrap();
    let table = txn.open_table(None).unwrap();
    let mut cursor = txn.cursor(&table).unwrap();

    // Because Result implements FromIterator, we can collect the iterator
    // of items of type Result<_, E> into a Result<Vec<_, E>> by specifying
    // the collection type via the turbofish syntax.
    assert_eq!(items, cursor.iter().collect::<Result<Vec<_>>>().unwrap());

    // Alternately, we can collect it into an appropriately typed variable.
    let retr: Result<Vec<_>> = cursor.iter_start().collect();
    assert_eq!(items, retr.unwrap());

    cursor.set::<()>(b"key2").unwrap();
    assert_eq!(
        items.clone().into_iter().skip(2).collect::<Vec<_>>(),
        cursor.iter().collect::<Result<Vec<_>>>().unwrap()
    );

    assert_eq!(
        items,
        cursor.iter_start().collect::<Result<Vec<_>>>().unwrap()
    );

    assert_eq!(
        items.clone().into_iter().skip(1).collect::<Vec<_>>(),
        cursor
            .iter_from(b"key2")
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        items.into_iter().skip(3).collect::<Vec<_>>(),
        cursor
            .iter_from(b"key4")
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        Vec::<((), ())>::new(),
        cursor
            .iter_from(b"key6")
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );
}

#[test]
fn test_iter_empty_database() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();
    let txn = db.begin_ro_txn().unwrap();
    let table = txn.open_table(None).unwrap();
    let mut cursor = txn.cursor(&table).unwrap();

    assert!(cursor.iter::<(), ()>().next().is_none());
    assert!(cursor.iter_start::<(), ()>().next().is_none());
    assert!(cursor.iter_from::<(), ()>(b"foo").next().is_none());
}

#[test]
fn test_iter_empty_dup_database() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    let txn = db.begin_rw_txn().unwrap();
    txn.create_table(None, TableFlags::DUP_SORT).unwrap();
    txn.commit().unwrap();

    let txn = db.begin_ro_txn().unwrap();
    let table = txn.open_table(None).unwrap();
    let mut cursor = txn.cursor(&table).unwrap();

    assert!(cursor.iter::<(), ()>().next().is_none());
    assert!(cursor.iter_start::<(), ()>().next().is_none());
    assert!(cursor.iter_from::<(), ()>(b"foo").next().is_none());
    assert!(cursor.iter_from::<(), ()>(b"foo").next().is_none());
    assert!(cursor.iter_dup::<(), ()>().flatten().next().is_none());
    assert!(cursor.iter_dup_start::<(), ()>().flatten().next().is_none());
    assert!(
        cursor
            .iter_dup_from::<(), ()>(b"foo")
            .flatten()
            .next()
            .is_none()
    );
    assert!(cursor.iter_dup_of::<(), ()>(b"foo").next().is_none());
}

#[test]
fn test_iter_dup() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    let txn = db.begin_rw_txn().unwrap();
    txn.create_table(None, TableFlags::DUP_SORT).unwrap();
    txn.commit().unwrap();

    let items = [
        (b"a", b"1"),
        (b"a", b"2"),
        (b"a", b"3"),
        (b"b", b"1"),
        (b"b", b"2"),
        (b"b", b"3"),
        (b"c", b"1"),
        (b"c", b"2"),
        (b"c", b"3"),
        (b"e", b"1"),
        (b"e", b"2"),
        (b"e", b"3"),
    ]
    .iter()
    .map(|&(&k, &v)| (k, v))
    .collect::<Vec<_>>();

    {
        let txn = db.begin_rw_txn().unwrap();
        for (key, data) in items.clone() {
            let table = txn.open_table(None).unwrap();
            txn.put(&table, key, data, WriteFlags::empty()).unwrap();
        }
        txn.commit().unwrap();
    }

    let txn = db.begin_ro_txn().unwrap();
    let table = txn.open_table(None).unwrap();
    let mut cursor = txn.cursor(&table).unwrap();
    assert_eq!(
        items,
        cursor
            .iter_dup()
            .flatten()
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    cursor.set::<()>(b"b").unwrap();
    assert_eq!(
        items.iter().copied().skip(4).collect::<Vec<_>>(),
        cursor
            .iter_dup()
            .flatten()
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        items,
        cursor
            .iter_dup_start()
            .flatten()
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        items.iter().copied().skip(3).collect::<Vec<_>>(),
        cursor
            .iter_dup_from(b"b")
            .flatten()
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        items.iter().copied().skip(3).collect::<Vec<_>>(),
        cursor
            .iter_dup_from(b"ab")
            .flatten()
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        items.iter().copied().skip(9).collect::<Vec<_>>(),
        cursor
            .iter_dup_from(b"d")
            .flatten()
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        Vec::<([u8; 1], [u8; 1])>::new(),
        cursor
            .iter_dup_from(b"f")
            .flatten()
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        items.iter().copied().skip(3).take(3).collect::<Vec<_>>(),
        cursor
            .iter_dup_of(b"b")
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(0, cursor.iter_dup_of::<(), ()>(b"foo").count());
}

#[test]
fn test_iter_del_get() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    let items = vec![(*b"a", *b"1"), (*b"b", *b"2")];
    {
        let txn = db.begin_rw_txn().unwrap();
        let table = txn.create_table(None, TableFlags::DUP_SORT).unwrap();
        assert_eq!(
            txn.cursor(&table)
                .unwrap()
                .iter_dup_of::<(), ()>(b"a")
                .collect::<Result<Vec<_>>>()
                .unwrap()
                .len(),
            0
        );
        txn.commit().unwrap();
    }

    {
        let txn = db.begin_rw_txn().unwrap();
        let table = txn.open_table(None).unwrap();
        for (key, data) in &items {
            txn.put(&table, key, data, WriteFlags::empty()).unwrap();
        }
        txn.commit().unwrap();
    }

    let txn = db.begin_rw_txn().unwrap();
    let table = txn.open_table(None).unwrap();
    let mut cursor = txn.cursor(&table).unwrap();
    assert_eq!(
        items,
        cursor
            .iter_dup()
            .flatten()
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        items.iter().copied().take(1).collect::<Vec<(_, _)>>(),
        cursor
            .iter_dup_of(b"a")
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(cursor.set(b"a").unwrap(), Some(*b"1"));

    cursor.del(WriteFlags::empty()).unwrap();

    assert_eq!(
        cursor
            .iter_dup_of::<(), ()>(b"a")
            .collect::<Result<Vec<_>>>()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn test_put_del() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    let txn = db.begin_rw_txn().unwrap();
    let table = txn.open_table(None).unwrap();
    let mut cursor = txn.cursor(&table).unwrap();

    for (k, v) in [(b"key1", b"val1"), (b"key2", b"val2"), (b"key3", b"val3")] {
        cursor.put(k, v, WriteFlags::empty()).unwrap();
    }

    assert_eq!(
        cursor.get_current().unwrap().unwrap(),
        (
            Cow::Borrowed(b"key3" as &[u8]),
            Cow::Borrowed(b"val3" as &[u8])
        )
    );

    cursor.del(WriteFlags::empty()).unwrap();
    assert_eq!(cursor.next::<Vec<u8>, Vec<u8>>().unwrap(), None);
    assert_eq!(
        cursor.last().unwrap().unwrap(),
        (
            Cow::Borrowed(b"key2" as &[u8]),
            Cow::Borrowed(b"val2" as &[u8])
        )
    );
}
