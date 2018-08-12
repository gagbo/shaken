#![allow(dead_code)] // go away clippy

use crate::color::RGB;
use rusqlite::Connection;

#[derive(Clone, PartialEq, Debug)]
pub struct User {
    pub userid: i64,
    pub display: String,
    pub color: RGB,
}

pub struct UserStore;
impl UserStore {
    pub fn init_table(conn: &Connection) {
        conn.execute(USER_TABLE, &[])
            .expect("to create Users table");
    }

    // XXX: default color for the bot: fc0fc0
    pub fn get_bot(conn: &Connection, name: &str) -> Option<User> {
        trace!("get bot by name: {}", name);
        let stmt = conn
            .prepare(
                "SELECT ID, Display, Color FROM Users WHERE DISPLAY = ? COLLATE NOCASE LIMIT 1",
            ).expect("valid sql");

        Self::get_user(name, stmt)
    }

    pub fn get_user_by_id(conn: &Connection, id: i64) -> Option<User> {
        trace!("get user by id: {}", id);
        let stmt = conn
            .prepare("SELECT ID, Display, Color FROM Users WHERE ID = ? LIMIT 1")
            .expect("valid sql");

        Self::get_user(id, stmt)
    }

    pub fn get_user_by_name(conn: &Connection, name: &str) -> Option<User> {
        trace!("get user by name: {}", name);
        let stmt = conn
            .prepare(
                "SELECT ID, Display, Color FROM Users WHERE DISPLAY = ? COLLATE NOCASE LIMIT 1",
            ).expect("valid sql");

        Self::get_user(name, stmt)
    }

    fn get_user<'a, T>(q: T, mut stmt: rusqlite::Statement<'a>) -> Option<User>
    where
        T: ::std::fmt::Display + rusqlite::types::ToSql,
    {
        let mut iter = stmt
            .query_map(&[&q], |row| User {
                userid: row.get(0),
                display: row.get(1),
                color: RGB::from(&row.get::<_, String>(2)),
            }).map_err(|e| {
                error!("cannot get user for '{}': {}", q, e);
            }).ok()?;

        if let Some(user) = iter.next() {
            return user
                .map_err(|e| {
                    error!("cannot get user for '{}': {}", q, e);
                }).ok();
        }
        None
    }

    pub fn create_user(conn: &Connection, user: &User) {
        match conn.execute(
            "INSERT OR IGNORE INTO Users (ID, Display, Color)  VALUES (?,?,?)",
            &[&user.userid, &user.display, &user.color.to_string()],
        ) {
            Ok(row) => debug!("added user({:?}) at {}", user, row),
            Err(err) => error!("cannot insert user({:?}) into table: {}", user, err),
        };
    }
}

const USER_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS Users (
    ID INTEGER PRIMARY KEY NOT NULL UNIQUE, -- twitch ID
    Display TEXT NOT NULL,                  -- twitch display name
    Color TEXT                              -- their selected color (twitch, or custom. #RRGGBB)
);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_get_user() {
        let conn = Connection::open_in_memory().unwrap();
        UserStore::init_table(&conn);

        let user = UserStore::get_user_by_id(&conn, 1004);
        assert_eq!(user, None);

        UserStore::create_user(
            &conn,
            &User {
                display: "Test".into(),
                color: RGB::from("#f0f0f0"),
                userid: 1004,
            },
        );

        let user = UserStore::get_user_by_id(&conn, 1004);
        assert_eq!(
            user,
            Some(User {
                display: "Test".into(),
                color: RGB::from("#f0f0f0"),
                userid: 1004,
            })
        );

        let user = UserStore::get_user_by_name(&conn, "test");
        assert_eq!(
            user,
            Some(User {
                display: "Test".into(),
                color: RGB::from("#f0f0f0"),
                userid: 1004,
            })
        );

        let user = UserStore::get_user_by_name(&conn, "TEST");
        assert_eq!(
            user,
            Some(User {
                display: "Test".into(),
                color: RGB::from("#f0f0f0"),
                userid: 1004,
            })
        );

        let user = UserStore::get_user_by_name(&conn, "not_test");
        assert_eq!(user, None);
    }
}