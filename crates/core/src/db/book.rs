//! Books in the library, with the reading position saved for each.

use rusqlite::{params, OptionalExtension, Row};

use super::{require_one_changed, require_text, u32_column, Database, DbError};

/// A book to add to the library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewBook {
    pub title: String,
    pub author: Option<String>,
    pub language_code: String,
    /// Where the book file lives in app storage.
    pub file_path: String,
    pub cover_path: Option<String>,
}

/// A stored book, with the position where the reader left off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Book {
    pub id: i64,
    pub title: String,
    pub author: Option<String>,
    pub language_code: String,
    pub file_path: String,
    pub cover_path: Option<String>,
    pub chapter_index: u32,
    pub paragraph_index: u32,
}

const COLUMNS: &str = "id, title, author, language_code, file_path, cover_path, \
     chapter_index, paragraph_index";

impl Database {
    /// Adds a book and returns its id.
    ///
    /// # Errors
    /// Returns `InvalidValue` for a blank title, and a database error if the
    /// language is not stored.
    pub fn insert_book(&self, book: &NewBook) -> Result<i64, DbError> {
        require_text("title", &book.title)?;
        if self.language(&book.language_code)?.is_none() {
            return Err(DbError::NotFound {
                what: format!("language {}", book.language_code),
            });
        }
        let id = self.conn().query_row(
            "INSERT INTO book (title, author, language_code, file_path, cover_path)
             VALUES (?1, ?2, ?3, ?4, ?5)
             RETURNING id",
            params![
                book.title,
                book.author,
                book.language_code,
                book.file_path,
                book.cover_path,
            ],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    /// The book with this id, if it exists.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn book(&self, id: i64) -> Result<Option<Book>, DbError> {
        let sql = format!("SELECT {COLUMNS} FROM book WHERE id = ?1");
        Ok(self
            .conn()
            .query_row(&sql, [id], book_from_row)
            .optional()?)
    }

    /// All books, in the order they were added.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn books(&self) -> Result<Vec<Book>, DbError> {
        let sql = format!("SELECT {COLUMNS} FROM book ORDER BY id");
        let mut statement = self.conn().prepare(&sql)?;
        let rows = statement.query_map([], book_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Saves where the reader stopped in a book.
    ///
    /// # Errors
    /// Returns `NotFound` if no book has this id.
    pub fn save_position(&self, book_id: i64, chapter: u32, paragraph: u32) -> Result<(), DbError> {
        let changed = self.conn().execute(
            "UPDATE book SET chapter_index = ?2, paragraph_index = ?3 WHERE id = ?1",
            params![book_id, chapter, paragraph],
        )?;
        require_one_changed(changed, || format!("book {book_id}"))
    }

    /// Deletes a book and its reading statistics.
    ///
    /// # Errors
    /// Returns `NotFound` if no book has this id.
    pub fn delete_book(&self, book_id: i64) -> Result<(), DbError> {
        let changed = self
            .conn()
            .execute("DELETE FROM book WHERE id = ?1", [book_id])?;
        require_one_changed(changed, || format!("book {book_id}"))
    }
}

impl Database {
    /// Replaces a book's table of contents. `titles[i]` is chapter `i`'s title.
    ///
    /// # Errors
    /// Returns a database error if the book does not exist or a statement fails.
    pub fn set_chapter_titles(
        &self,
        book_id: i64,
        titles: &[Option<String>],
    ) -> Result<(), DbError> {
        let transaction = self.conn().unchecked_transaction()?;
        transaction.execute("DELETE FROM book_chapter WHERE book_id = ?1", [book_id])?;
        {
            let mut insert = transaction.prepare(
                "INSERT INTO book_chapter (book_id, chapter_index, title) VALUES (?1, ?2, ?3)",
            )?;
            for (index, title) in titles.iter().enumerate() {
                let index = u32::try_from(index)
                    .map_err(|_| DbError::InvalidValue("too many chapters".into()))?;
                insert.execute(params![book_id, index, title])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// A book's chapter titles in reading order. `None` means the chapter has no heading.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn chapter_titles(&self, book_id: i64) -> Result<Vec<Option<String>>, DbError> {
        let mut statement = self
            .conn()
            .prepare("SELECT title FROM book_chapter WHERE book_id = ?1 ORDER BY chapter_index")?;
        let rows = statement.query_map([book_id], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Sets or clears the path of a book's cover image.
    ///
    /// # Errors
    /// Returns `NotFound` if no book has this id.
    pub fn set_cover_path(&self, book_id: i64, cover_path: Option<&str>) -> Result<(), DbError> {
        let changed = self.conn().execute(
            "UPDATE book SET cover_path = ?2 WHERE id = ?1",
            params![book_id, cover_path],
        )?;
        require_one_changed(changed, || format!("book {book_id}"))
    }
}

fn book_from_row(row: &Row) -> rusqlite::Result<Book> {
    Ok(Book {
        id: row.get(0)?,
        title: row.get(1)?,
        author: row.get(2)?,
        language_code: row.get(3)?,
        file_path: row.get(4)?,
        cover_path: row.get(5)?,
        chapter_index: u32_column(row, 6)?,
        paragraph_index: u32_column(row, 7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests_support::french_db;

    fn db_with_french() -> Database {
        french_db()
    }

    fn new_book(title: &str) -> NewBook {
        NewBook {
            title: title.into(),
            author: Some("Jean Exemple".into()),
            language_code: "fr".into(),
            file_path: format!("books/{title}.epub"),
            cover_path: None,
        }
    }

    #[test]
    fn inserted_books_are_listed_in_order() {
        let db = db_with_french();
        let first = db.insert_book(&new_book("Un")).unwrap();
        let second = db.insert_book(&new_book("Deux")).unwrap();
        let titles: Vec<String> = db.books().unwrap().into_iter().map(|b| b.title).collect();
        assert_eq!(titles, ["Un", "Deux"]);
        assert!(second > first);
    }

    #[test]
    fn new_book_starts_at_the_beginning() {
        let db = db_with_french();
        let id = db.insert_book(&new_book("Un")).unwrap();
        let book = db.book(id).unwrap().unwrap();
        assert_eq!((book.chapter_index, book.paragraph_index), (0, 0));
        assert_eq!(book.author.as_deref(), Some("Jean Exemple"));
    }

    #[test]
    fn saved_position_is_read_back() {
        let db = db_with_french();
        let id = db.insert_book(&new_book("Un")).unwrap();
        db.save_position(id, 3, 42).unwrap();
        let book = db.book(id).unwrap().unwrap();
        assert_eq!((book.chapter_index, book.paragraph_index), (3, 42));
    }

    #[test]
    fn saving_position_for_a_missing_book_is_not_found() {
        let db = db_with_french();
        assert!(matches!(
            db.save_position(99, 1, 1),
            Err(DbError::NotFound { .. })
        ));
    }

    #[test]
    fn blank_title_is_refused() {
        let db = db_with_french();
        assert!(matches!(
            db.insert_book(&new_book("   ")),
            Err(DbError::InvalidValue(_))
        ));
    }

    #[test]
    fn book_in_an_unknown_language_is_a_typed_not_found() {
        let db = db_with_french();
        let mut book = new_book("Un");
        book.language_code = "zz".into();
        assert!(matches!(
            db.insert_book(&book),
            Err(DbError::NotFound { what }) if what == "language zz"
        ));
    }

    #[test]
    fn chapter_titles_are_stored_in_order_and_can_be_replaced() {
        let db = db_with_french();
        let id = db.insert_book(&new_book("Un")).unwrap();
        db.set_chapter_titles(id, &[Some("Début".into()), None])
            .unwrap();
        assert_eq!(
            db.chapter_titles(id).unwrap(),
            vec![Some("Début".to_string()), None]
        );
        db.set_chapter_titles(id, &[Some("Seul".into())]).unwrap();
        assert_eq!(
            db.chapter_titles(id).unwrap(),
            vec![Some("Seul".to_string())]
        );
    }

    #[test]
    fn chapters_and_cover_path_go_with_the_book() {
        let db = db_with_french();
        let id = db.insert_book(&new_book("Un")).unwrap();
        db.set_chapter_titles(id, &[None]).unwrap();
        db.set_cover_path(id, Some("covers/1.png")).unwrap();
        assert_eq!(
            db.book(id).unwrap().unwrap().cover_path.as_deref(),
            Some("covers/1.png")
        );
        db.delete_book(id).unwrap();
        assert!(db.chapter_titles(id).unwrap().is_empty());
    }

    #[test]
    fn deleting_a_book_removes_it() {
        let db = db_with_french();
        let id = db.insert_book(&new_book("Un")).unwrap();
        db.delete_book(id).unwrap();
        assert_eq!(db.book(id).unwrap(), None);
        assert!(matches!(db.delete_book(id), Err(DbError::NotFound { .. })));
    }
}
