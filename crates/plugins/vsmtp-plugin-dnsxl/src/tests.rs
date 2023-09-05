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

use crate::api::dnsxl;
use rhai::Engine;

#[test]
fn test_building_blocklist() {
    let engine = Engine::new();
    let map = engine.parse_json(
        r#"
            {
                "bl": ["spamhaus", "spamrats"],
            }"#,
        true,
    );
    dnsxl::blacklist(map.unwrap()).unwrap();
}

#[test]
fn test_building_whitelist() {
    let engine = Engine::new();
    let map = engine.parse_json(
        r#"
            {
                "wl": ["localhost"],
            }"#,
        true,
    );
    dnsxl::blacklist(map.unwrap()).unwrap();
}

#[test]
fn test_kw_spam_check() {
    let engine = Engine::new();
    let map = engine.parse_json(
        r#"
            {
                "bl": ["s5h"],
            }"#,
        true,
    );
    let mut dnsxl = dnsxl::blacklist(map.unwrap()).unwrap();
    assert_eq!(
        dnsxl::contains_bl(&mut dnsxl, "2.0.0.127".into()).type_name(),
        String::from("map")
    );
}

#[test]
fn test_url_spam_check() {
    let engine = Engine::new();
    let map = engine.parse_json(
        r#"
            {
                "bl": ["all.s5h.net"],
            }"#,
        true,
    );
    let mut dnsxl = dnsxl::blacklist(map.unwrap()).unwrap();
    assert_eq!(
        dnsxl::contains_bl(&mut dnsxl, "2.0.0.127".into()).type_name(),
        String::from("map")
    );
}

#[test]
fn test_non_spam_check() {
    let engine = Engine::new();
    let map = engine.parse_json(
        r#"
            {
                "bl": ["spamhaus"],
            }"#,
        true,
    );
    let mut dnsxl = dnsxl::blacklist(map.unwrap()).unwrap();
    assert_eq!(
        dnsxl::contains_bl(&mut dnsxl, "example.com".into()).type_name(),
        String::from("()")
    );
}

#[test]
fn test_contains_whitelist() {
    let engine = Engine::new();
    let map = engine.parse_json(
        r#"
            {
                "wl": ["localhost"],
            }"#,
        true,
    );
    let mut dnsxl = dnsxl::whitelist(map.unwrap()).unwrap();
    assert_eq!(
        dnsxl::contains_wl(&mut dnsxl, "wwW.google.com".into()).type_name(),
        String::from("map")
    );
}
