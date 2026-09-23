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
fn test_iter_rev() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    {
        let txn = db.begin_rw_txn().unwrap();
        let table = txn.open_table(None).unwrap();
        for (k, v) in [(b"key1", b"val1"), (b"key2", b"val2"), (b"key3", b"val3")] {
            txn.put(&table, k, v, WriteFlags::empty()).unwrap();
        }
        txn.commit().unwrap();
    }

    let txn = db.begin_ro_txn().unwrap();
    let table = txn.open_table(None).unwrap();
    let cursor = txn.cursor(&table).unwrap();

    let items: Vec<_> = cursor
        .into_iter_start::<Vec<u8>, Vec<u8>>()
        .rev()
        .map(|kv| kv.unwrap().0)
        .collect();
    assert_eq!(
        items,
        vec![b"key3".to_vec(), b"key2".to_vec(), b"key1".to_vec()]
    );
}

#[test]
fn test_iter_back_from_upperbound() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    {
        let txn = db.begin_rw_txn().unwrap();
        let table = txn.open_table(None).unwrap();
        for i in 1..=5u8 {
            txn.put(
                &table,
                format!("key{i}"),
                format!("val{i}"),
                WriteFlags::empty(),
            )
            .unwrap();
        }
        txn.commit().unwrap();
    }

    let txn = db.begin_ro_txn().unwrap();
    let table = txn.open_table(None).unwrap();
    let mut cursor = txn.cursor(&table).unwrap();

    // back from "key3" yields key3, key2, key1
    let items: Vec<_> = cursor
        .clone()
        .into_iter_back_from::<Vec<u8>, Vec<u8>>(b"key3")
        .map(|kv| kv.unwrap().0)
        .collect();
    assert_eq!(
        items,
        vec![b"key3".to_vec(), b"key2".to_vec(), b"key1".to_vec()]
    );

    // absent upper bound: back from "key3x" must NOT yield key3x or beyond
    let items: Vec<_> = cursor
        .clone()
        .into_iter_back_from::<Vec<u8>, Vec<u8>>(b"key3x")
        .map(|kv| kv.unwrap().0)
        .collect();
    assert_eq!(
        items,
        vec![b"key3".to_vec(), b"key2".to_vec(), b"key1".to_vec()]
    );

    // .rev() on a bounded back iterator is an ascending iteration over the
    // same domain: keys <= bound, and nothing beyond it
    let items: Vec<_> = cursor
        .clone()
        .into_iter_back_from::<Vec<u8>, Vec<u8>>(b"key3")
        .rev()
        .map(|kv| kv.unwrap().0)
        .collect();
    assert_eq!(
        items,
        vec![b"key1".to_vec(), b"key2".to_vec(), b"key3".to_vec()]
    );

    // back from a key before the first yields nothing
    assert_eq!(
        0,
        cursor
            .clone()
            .into_iter_back_from::<(), ()>(b"key0")
            .count()
    );

    // mixed front/back: full table partitioned
    let mut it = cursor.iter_start::<Vec<u8>, Vec<u8>>();
    assert_eq!(it.next().unwrap().unwrap().0, b"key1".to_vec());
    assert_eq!(it.next_back().unwrap().unwrap().0, b"key5".to_vec());
}

#[test]
fn test_iter_back_from_bound_hard_stop() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    {
        let txn = db.begin_rw_txn().unwrap();
        let table = txn.open_table(None).unwrap();
        for i in 1..=5u8 {
            txn.put(
                &table,
                format!("key{i}"),
                format!("val{i}"),
                WriteFlags::empty(),
            )
            .unwrap();
        }
        txn.commit().unwrap();
    }

    let txn = db.begin_ro_txn().unwrap();
    let table = txn.open_table(None).unwrap();
    let cursor = txn.cursor(&table).unwrap();

    // Drive the back direction into the bound: key1..key3, then the 4th
    // next_back hits the bound and parks the cursor on key4.
    let mut iter = cursor
        .clone()
        .into_iter_back_from::<Vec<u8>, Vec<u8>>(b"key3");
    assert_eq!(iter.next_back().unwrap().unwrap().0, b"key1".to_vec());
    assert_eq!(iter.next_back().unwrap().unwrap().0, b"key2".to_vec());
    assert_eq!(iter.next_back().unwrap().unwrap().0, b"key3".to_vec());
    assert!(iter.next_back().is_none());

    // The parked key4 is outside the "keys <= key3" domain: the front
    // direction must not leak it, and the iterator stays exhausted.
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(iter.next_back().is_none());

    // .rev() domain is unaffected: ascending keys <= bound.
    let items: Vec<_> = cursor
        .clone()
        .into_iter_back_from::<Vec<u8>, Vec<u8>>(b"key3")
        .rev()
        .map(|kv| kv.unwrap().0)
        .collect();
    assert_eq!(
        items,
        vec![b"key1".to_vec(), b"key2".to_vec(), b"key3".to_vec()]
    );
}

#[test]
fn test_iter_back_start() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    {
        let txn = db.begin_rw_txn().unwrap();
        let table = txn.open_table(None).unwrap();
        for (k, v) in [(b"key1", b"val1"), (b"key2", b"val2"), (b"key3", b"val3")] {
            txn.put(&table, k, v, WriteFlags::empty()).unwrap();
        }
        txn.commit().unwrap();
    }

    let txn = db.begin_ro_txn().unwrap();
    let table = txn.open_table(None).unwrap();
    let cursor = txn.cursor(&table).unwrap();

    let items: Vec<_> = cursor
        .into_iter_back_start::<Vec<u8>, Vec<u8>>()
        .map(|kv| kv.unwrap().0)
        .collect();
    assert_eq!(
        items,
        vec![b"key3".to_vec(), b"key2".to_vec(), b"key1".to_vec()]
    );
}

#[test]
fn test_set_upperbound() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    {
        let txn = db.begin_rw_txn().unwrap();
        let table = txn.open_table(None).unwrap();
        for (k, v) in [
            (b"key1", b"val1"),
            (b"key2", b"val2"),
            (b"key3", b"val3"),
            (b"key4", b"val4"),
            (b"key5", b"val5"),
        ] {
            txn.put(&table, k, v, WriteFlags::empty()).unwrap();
        }
        txn.commit().unwrap();
    }

    let txn = db.begin_ro_txn().unwrap();
    let table = txn.open_table(None).unwrap();
    let mut cursor = txn.cursor(&table).unwrap();

    // position at largest key <= given: absent key falls back to key3
    assert_eq!(
        cursor.set_upperbound::<Vec<u8>, Vec<u8>>(b"key3x").unwrap(),
        Some((b"key3".to_vec(), b"val3".to_vec()))
    );
    // exact key
    assert_eq!(
        cursor.set_upperbound::<Vec<u8>, Vec<u8>>(b"key2").unwrap(),
        Some((b"key2".to_vec(), b"val2".to_vec()))
    );
    // before the first key: nothing
    assert_eq!(cursor.set_upperbound::<(), ()>(b"key0").unwrap(), None);
    // at the last key: still the last key
    assert_eq!(
        cursor.set_upperbound::<Vec<u8>, Vec<u8>>(b"key5").unwrap(),
        Some((b"key5".to_vec(), b"val5".to_vec()))
    );
    // beyond the last key: the last key
    assert_eq!(
        cursor.set_upperbound::<Vec<u8>, Vec<u8>>(b"key6").unwrap(),
        Some((b"key5".to_vec(), b"val5".to_vec()))
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
    assert!(
        cursor
            .iter_dup::<(), ()>()
            .flat_map(|nested| nested.unwrap())
            .next()
            .is_none()
    );
    assert!(
        cursor
            .iter_dup_start::<(), ()>()
            .flat_map(|nested| nested.unwrap())
            .next()
            .is_none()
    );
    assert!(
        cursor
            .iter_dup_from::<(), ()>(b"foo")
            .flat_map(|nested| nested.unwrap())
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
            .flat_map(|nested| nested.unwrap())
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    cursor.set::<()>(b"b").unwrap();
    assert_eq!(
        items.iter().copied().skip(4).collect::<Vec<_>>(),
        cursor
            .iter_dup()
            .flat_map(|nested| nested.unwrap())
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        items,
        cursor
            .iter_dup_start()
            .flat_map(|nested| nested.unwrap())
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        items.iter().copied().skip(3).collect::<Vec<_>>(),
        cursor
            .iter_dup_from(b"b")
            .flat_map(|nested| nested.unwrap())
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        items.iter().copied().skip(3).collect::<Vec<_>>(),
        cursor
            .iter_dup_from(b"ab")
            .flat_map(|nested| nested.unwrap())
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        items.iter().copied().skip(9).collect::<Vec<_>>(),
        cursor
            .iter_dup_from(b"d")
            .flat_map(|nested| nested.unwrap())
            .collect::<Result<Vec<_>>>()
            .unwrap()
    );

    assert_eq!(
        Vec::<([u8; 1], [u8; 1])>::new(),
        cursor
            .iter_dup_from(b"f")
            .flat_map(|nested| nested.unwrap())
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
fn test_iter_dup_shape() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open_with_options(
        &dir,
        DatabaseOptions {
            max_tables: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    {
        let tx = db.begin_rw_txn().unwrap();
        let table = tx.create_table(Some("test"), TableFlags::DUP_SORT).unwrap();
        tx.put(&table, "key1", "val1", WriteFlags::UPSERT).unwrap();
        tx.put(&table, "key1", "val2", WriteFlags::UPSERT).unwrap();
        tx.put(&table, "key2", "val3", WriteFlags::UPSERT).unwrap();
        tx.commit().unwrap();
    }

    let tx = db.begin_ro_txn().unwrap();
    let table = tx.open_table(Some("test")).unwrap();
    let mut cursor = tx.cursor(&table).unwrap();
    let mut collected = vec![];
    for nested in cursor.iter_dup_start::<Vec<u8>, Vec<u8>>() {
        let nested = nested.unwrap();
        let dups: Vec<_> = nested.map(|kv| kv.unwrap().1).collect();
        collected.push(dups);
    }
    assert_eq!(
        collected,
        vec![
            vec![b"val1".to_vec(), b"val2".to_vec()],
            vec![b"val3".to_vec()]
        ]
    );
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
            .flat_map(|nested| nested.unwrap())
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
fn test_reseek_with_borrowed_key() {
    let dir = tempdir().unwrap();
    let db = Database::open_with_options(
        &dir,
        DatabaseOptions {
            max_tables: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    let tx = db.begin_rw_txn().unwrap();
    let table = tx.create_table(Some("test"), Default::default()).unwrap();
    tx.put(&table, b"key1", b"val1", WriteFlags::UPSERT)
        .unwrap();
    tx.put(&table, b"key2", b"val2", WriteFlags::UPSERT)
        .unwrap();
    tx.commit().unwrap();

    let tx = db.begin_ro_txn().unwrap();
    let table = tx.open_table(Some("test")).unwrap();
    let mut cursor = tx.cursor(&table).unwrap();
    let (k, v) = cursor
        .set_range::<Cow<[u8]>, Cow<[u8]>>(b"key1")
        .unwrap()
        .unwrap();
    assert_eq!(&*v, b"val1");
    // k is `Cow::Borrowed` pointing at db memory; reseeking with it must not panic.
    let (k2, v2) = cursor.set_key::<Cow<[u8]>, Cow<[u8]>>(&k).unwrap().unwrap();
    assert_eq!(&*k2, b"key1");
    assert_eq!(&*v2, b"val1");
    let (_, _, v3) = cursor
        .set_lowerbound::<Cow<[u8]>, Cow<[u8]>>(&k, None)
        .unwrap()
        .unwrap();
    assert_eq!(&*v3, b"val1");
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

/// `MDBX_MULTIPLE` (0x80000) makes libmdbx treat `data` as an array of two
/// `MDBX_val`s and `MDBX_RESERVE` (0x10000) leaves the value uninitialized;
/// neither may be reachable through the plain put flags.
#[test]
fn test_write_flags_exclude_multiple_and_reserve() {
    assert_eq!(WriteFlags::from_bits(0x80000), None);
    assert_eq!(WriteFlags::from_bits(0x10000), None);
}

#[test]
fn test_put_ignores_unknown_flag_bits() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    let txn = db.begin_rw_txn().unwrap();
    let table = txn
        .create_table(None, TableFlags::DUP_SORT | TableFlags::DUP_FIXED)
        .unwrap();
    let smuggled = WriteFlags::from_bits_retain(0x80000 | 0x10000);
    txn.put(&table, b"key", b"val1", smuggled).unwrap();
    let mut cursor = txn.cursor(&table).unwrap();
    cursor.put(b"key", b"val2", smuggled).unwrap();

    assert_eq!(cursor.first().unwrap(), Some((*b"key", *b"val1")));
    assert_eq!(cursor.get_multiple().unwrap(), Some(*b"val1val2"));
}

#[test]
fn test_put_multiple() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    let txn = db.begin_rw_txn().unwrap();
    let table = txn
        .create_table(None, TableFlags::DUP_SORT | TableFlags::DUP_FIXED)
        .unwrap();
    let mut cursor = txn.cursor(&table).unwrap();
    assert_eq!(
        cursor
            .put_multiple(b"key", b"val3val1val2", 4, WriteFlags::empty())
            .unwrap(),
        3
    );
    assert_eq!(
        txn.put_multiple(&table, b"key2", b"abcd", 4, WriteFlags::empty())
            .unwrap(),
        1
    );
    assert_eq!(cursor.first().unwrap(), Some((*b"key", *b"val1")));
    assert_eq!(cursor.get_multiple().unwrap(), Some(*b"val1val2val3"));
    assert_eq!(
        cursor
            .put_multiple(b"key", b"", 4, WriteFlags::empty())
            .unwrap(),
        0
    );
}

#[test]
fn test_put_multiple_integer_dup_misaligned() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    let txn = db.begin_rw_txn().unwrap();
    let table = txn
        .create_table(
            None,
            TableFlags::DUP_SORT | TableFlags::DUP_FIXED | TableFlags::INTEGER_DUP,
        )
        .unwrap();
    // Offset by one byte so the u64 elements are misaligned.
    let mut bytes = vec![0u8];
    for v in [3u64, 1, 2] {
        bytes.extend_from_slice(&v.to_ne_bytes());
    }
    let mut cursor = txn.cursor(&table).unwrap();
    assert_eq!(
        cursor
            .put_multiple(b"key", &bytes[1..], 8, WriteFlags::empty())
            .unwrap(),
        3
    );
    let values: Vec<u64> = cursor
        .iter_dup_of::<(), [u8; 8]>(b"key")
        .map(|r| u64::from_ne_bytes(r.unwrap().1))
        .collect();
    assert_eq!(values, [1, 2, 3]);
}

#[test]
fn test_put_multiple_rejects_bad_lengths() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir).unwrap();

    let txn = db.begin_rw_txn().unwrap();
    let table = txn
        .create_table(None, TableFlags::DUP_SORT | TableFlags::DUP_FIXED)
        .unwrap();
    let mut cursor = txn.cursor(&table).unwrap();
    assert!(matches!(
        cursor.put_multiple(b"key", b"abc", 0, WriteFlags::empty()),
        Err(Error::InvalidArgument(_))
    ));
    assert!(matches!(
        cursor.put_multiple(b"key", b"abcde", 2, WriteFlags::empty()),
        Err(Error::InvalidArgument(_))
    ));
}
