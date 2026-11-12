//! The changes made to the database over time, which are the SQL files in `migrations/`.
//!
//! The build finds the files by looking in the folder, so none is listed here. The database says
//! in its `user_version` the number of the last one it had. A new database has them all, and an
//! older one gets the ones it has not had. To change the database, add a file with the next
//! number, like `003_what_it_does.sql`. Never change a file that was released.

use rusqlite::Connection;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/migrations.rs"));
}
use embedded::MIGRATIONS;

/// Brings the database up to date. It refuses a database that is not ours, and one that a newer
/// Stubprint made, and changes neither.
pub(crate) fn apply(db: &Connection) -> rusqlite::Result<()> {
    let version: i64 = db.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let tables: i64 = db.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))?;
    let had = u32::try_from(version).unwrap_or(u32::MAX);
    if had == 0 && tables > 0 {
        return Err(rusqlite::Error::InvalidParameterName(
            "it is a database of something else".into(),
        ));
    }
    let last = MIGRATIONS.last().map_or(0, |(number, _, _)| *number);
    if had > last {
        return Err(rusqlite::Error::InvalidParameterName(
            "it was made by a newer Stubprint".into(),
        ));
    }
    for (number, _, sql) in MIGRATIONS.iter().filter(|(number, _, _)| *number > had) {
        // A change and the number that says it was made are written together or not at all.
        db.execute_batch(&format!(
            "BEGIN; {sql} PRAGMA user_version = {number}; COMMIT;"
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(db: &Connection) -> i64 {
        db.pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn the_changes_were_found_in_the_folder_and_are_numbered_without_gaps() {
        assert!(MIGRATIONS.len() >= 2);
        for (index, (number, name, sql)) in MIGRATIONS.iter().enumerate() {
            assert_eq!(usize::try_from(*number).unwrap(), index + 1, "{name}");
            assert!(!sql.trim().is_empty(), "{name} is empty");
        }
    }

    #[test]
    fn a_new_database_has_every_change() {
        let db = Connection::open_in_memory().unwrap();
        apply(&db).unwrap();
        assert_eq!(version(&db), i64::from(MIGRATIONS.last().unwrap().0));
        // The columns the last change added are there.
        db.prepare("SELECT paper_roll, paper_used FROM printers")
            .unwrap();
    }

    #[test]
    fn an_older_database_gets_the_changes_it_has_not_had_and_keeps_what_it_has() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(&format!(
            "BEGIN; {} PRAGMA user_version = 1; COMMIT;
             INSERT INTO printers VALUES (0, 'till', '127.0.0.1:9100', 'escpos', 'default', 4096, 0, '{{}}');",
            MIGRATIONS[0].2
        ))
        .unwrap();
        apply(&db).unwrap();
        assert_eq!(version(&db), 2);
        let (name, roll): (String, i64) = db
            .query_row("SELECT name, paper_roll FROM printers", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!((name.as_str(), roll), ("till", 0));
    }

    #[test]
    fn applying_again_changes_nothing() {
        let db = Connection::open_in_memory().unwrap();
        apply(&db).unwrap();
        apply(&db).unwrap();
        assert_eq!(version(&db), 2);
    }

    #[test]
    fn a_database_of_something_else_is_refused() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE notes (text TEXT);").unwrap();
        let error = apply(&db).unwrap_err().to_string();
        assert!(error.contains("something else"), "{error}");
    }

    #[test]
    fn a_database_from_a_newer_stubprint_is_refused() {
        let db = Connection::open_in_memory().unwrap();
        db.pragma_update(None, "user_version", 99).unwrap();
        let error = apply(&db).unwrap_err().to_string();
        assert!(error.contains("newer"), "{error}");
    }
}
