/*
 * vSMTP mail transfer agent
 *
 * Copyright (C) 2003 - viridIT SAS
 * Licensed under the Elastic License 2.0
 *
 * You should have received a copy of the Elastic License 2.0 along with
 * this program. If not, see https://www.elastic.co/licensing/elastic-license.
 *
 */

use crate::api::sqlite_api;
use rhai::Engine;

#[test]
fn test_query() {
    let engine = Engine::new();
    let map = engine.parse_json(
        r#"
            {
                "path": "sharks.db",
                "connections": 1,
                "timeout": "1s"
            }"#,
        true,
    );
    let mut server = sqlite_api::connect(map.unwrap()).unwrap();
    sqlite_api::query(&mut server, "CREATE TABLE sharks(id integer NOT NULL, name text NOT NULL, sharktype text NOT NULL, length integer NOT NULL);").unwrap();
    sqlite_api::query(
        &mut server,
        "INSERT INTO sharks VALUES (1, \"Sammy\", \"Greenland Shark\", 427);",
    )
    .unwrap();
    sqlite_api::query(
        &mut server,
        "INSERT INTO sharks VALUES (2, \"Alyoshka\", \"Great White Shark\", 600);",
    )
    .unwrap();
    sqlite_api::query(
        &mut server,
        "INSERT INTO sharks VALUES (3, \"Himari\", \"Megaladon\", 1800);",
    )
    .unwrap();
    sqlite_api::query(&mut server, "SELECT * FROM sharks;").unwrap();
    dbg!(sqlite_api::query(&mut server, "SELECT * FROM sharks;")).unwrap();
}
