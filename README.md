# Concentus

Provider-agnostic music metadata server for rich, first-class visuals.

## Rust client

Depend on the `concentus` workspace package. It exports the shared types
under `concentus::types`. The client handles HTTP and Postcard encoding.

```rust
use concentus::{Client, Error, types::{Track, TrackLookup}};

async fn find_track(client: &Client, isrc: concentus::types::Isrc)
    -> Result<Option<Track>, Error>
{
    client.lookup_track(TrackLookup::by_isrc(isrc)).await
}
```

Construct a client with `Client::new("https://your-concentus-host")`. Use
`lookup_artist`, `lookup_album`, `lookup_track`, or `lookup_tag`
for metadata resolution. All supplied filters must match. Zero or multiple matches
return `None`, as does a request with no filters. `list` and `get` are
also available.

Use `Lookup::by_external_id` for Spotify, Apple Music, and other provider IDs.
`with_field` adds a filter on any top-level field. For nested metadata, add
`FieldFilter` paths using field names and optional array element labels. Reuse a
label when several predicates need to match the same element.

Track lookups return an empty lyrics map by default. Use `TrackLookup::by_isrc(isrc).with_lyrics()`
or set `lookup.options.lyrics = true` to include lyrics for the unique match. The option
affects the response only. Track list and ID endpoints omit lyrics. The map keys are
BCP 47 tags such as `en` or `pt-BR`, with `und` for an unknown language.

## API and deployment

[`concentus_server`](crates/concentus_server/) is the native HTTP server.
[`concentus_types`](crates/concentus_types/) defines its request and response types,
and [`concentus`](crates/concentus/) provides the async HTTP client.

The unversioned `/api/` routes serve the current schema.

| Method and path | Postcard body |
| --- | --- |
| `GET /api/{collection}` | Returns `Vec<T>`. Accepts optional `after` and `limit` query parameters. |
| `GET /api/{collection}/{id}` | Returns `T`, or HTTP 404. |
| `POST /api/{collection}/lookup` | Accepts `Lookup<T>` and returns `Option<T>`. |

HTTP bodies use `application/vnd.concentus.postcard`. Both sides send
`x-concentus-schema`. Increment `SCHEMA_REVISION` in `concentus_types` when the
binary layout changes and release the client and server together. The server
returns HTTP 409 for a missing or mismatched revision.

Run `just dev` or `cargo run --locked -p concentus_server` from the workspace root.
The server listens on `127.0.0.1:3000` by default. Set `HOST` or `PORT` to override it.

On Linux, the database lives at `$XDG_STATE_HOME/concentus/catalog.sqlite3`, defaulting
to `~/.local/state/concentus/catalog.sqlite3`. Set `CONCENTUS_DATABASE` to use a different
file. SQLx creates the database and applies the [SQLite migrations](crates/concentus_server/migrations/)
on startup. The catalog uses strict SQLite tables with typed columns, foreign keys and
ordered child tables for repeated fields. Provider IDs with an unsigned 64-bit value use
checked decimal text to preserve their full range. Postcard handles the HTTP API.

For imports with references between albums and tracks, enable SQLite foreign keys and
insert related records in one transaction. If you have a database from an earlier
schema, choose a fresh file with `CONCENTUS_DATABASE`.
