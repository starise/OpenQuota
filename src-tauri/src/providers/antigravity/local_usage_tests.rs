use std::{fs, path::PathBuf};

use chrono::{TimeZone, Utc};
use rusqlite::Connection;
use tempfile::TempDir;

use crate::pricing::{ModelPricing, ModelRates, PricingCatalog, PricingSupplement};

use super::scan_stores;

const FIXTURE: &str = include_str!("fixtures/conversation_usage.sql");

fn database_path(root: &TempDir, store: &str, file: &str) -> PathBuf {
    let directory = root.path().join(store).join("conversations");
    fs::create_dir_all(&directory).unwrap();
    directory.join(file)
}

fn pricing() -> ModelPricing {
    let mut catalog = PricingCatalog::default();
    catalog
        .entries
        .insert("priced-model".into(), ModelRates::new(1.0, 2.0));
    ModelPricing::new(
        PricingSupplement::default(),
        catalog,
        PricingCatalog::default(),
    )
}

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 7, 11, 12, 10, 0).unwrap()
}

#[test]
fn reads_committed_wal_counts_once_and_prices_only_known_models() {
    let root = tempfile::tempdir().unwrap();
    let cli = database_path(&root, "antigravity-cli", "original.db");
    let mut writer = Connection::open(&cli).unwrap();
    writer
        .execute_batch("PRAGMA journal_mode = WAL; PRAGMA wal_autocheckpoint = 0;")
        .unwrap();
    writer.execute_batch(FIXTURE).unwrap();
    assert!(cli.with_extension("db-wal").exists());
    let uncommitted = writer.transaction().unwrap();
    uncommitted
        .execute(
            "INSERT INTO gen_metadata SELECT 5, data FROM gen_metadata WHERE idx = 0",
            [],
        )
        .unwrap();

    let copy = database_path(&root, "antigravity", "renamed-copy.db");
    let copy = Connection::open(copy).unwrap();
    copy.execute_batch(FIXTURE).unwrap();
    copy.execute("DELETE FROM gen_metadata WHERE idx = 1", [])
        .unwrap();
    let mut warnings = Vec::new();
    let usage = scan_stores(root.path(), now(), &pricing(), &mut warnings).unwrap();

    let today = usage.today.unwrap();
    assert_eq!(today.tokens, 26_513);
    assert_eq!(today.unknown_models, ["unpriced-model"]);
    assert!(!today.estimate_complete);
    assert!((today.estimated_cost_usd.unwrap() - 0.016_959_2).abs() < 1e-12);
    assert!(usage.yesterday.is_none());
    assert_eq!(warnings.len(), 1);
}

#[test]
fn distinguishes_empty_incompatible_and_temporarily_locked_databases() {
    let root = tempfile::tempdir().unwrap();
    let path = database_path(&root, "antigravity-ide", "empty.db");
    let writer = Connection::open(&path).unwrap();
    writer
        .execute_batch("CREATE TABLE gen_metadata (idx INTEGER PRIMARY KEY, data BLOB);")
        .unwrap();
    fs::write(database_path(&root, "antigravity-ide", "stub.db"), []).unwrap();
    let mut warnings = Vec::new();
    let usage = scan_stores(root.path(), now(), &pricing(), &mut warnings).unwrap();
    assert!(usage.daily.is_empty());
    assert!(warnings.is_empty());

    let incompatible = database_path(&root, "antigravity-cli", "unsupported.db");
    Connection::open(incompatible)
        .unwrap()
        .execute_batch("CREATE TABLE gen_metadata (idx INTEGER PRIMARY KEY, unknown BLOB);")
        .unwrap();
    let usage = scan_stores(root.path(), now(), &pricing(), &mut warnings).unwrap();
    assert!(usage.daily.is_empty());
    assert_eq!(warnings.len(), 1);
    warnings.clear();

    writer.execute_batch("BEGIN EXCLUSIVE;").unwrap();
    assert!(scan_stores(root.path(), now(), &pricing(), &mut warnings).is_err());
    writer.execute_batch("ROLLBACK;").unwrap();
    assert!(scan_stores(root.path(), now(), &pricing(), &mut warnings).is_ok());
}
