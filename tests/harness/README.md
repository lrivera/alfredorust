# Harness Notes

This directory documents harness conventions for integration and HTTP tests.

Use `tests/common/mod.rs` for shared setup. Add reusable safe fixtures under `tests/fixtures/` when a test needs representative files or payloads.

Harness tests should prefer real `AppState`, real MongoDB collections, and in-memory Axum routers over mocked internals. Mock or fake only external systems that cannot run safely in tests, such as SAT network calls or production certificate material.
