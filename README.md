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

Use `Lookup::by_external_id(ExternalId::new(ExternalIdProvider::Spotify, id))` or
`TrackLookup::by_isrc(isrc)` for provider IDs. Returned records group values by
provider in `external_ids`, and listings can include multiple IDs per provider.
`with_field` adds a filter on any top-level field. For nested metadata, add
`FieldFilter` paths using field names and optional array element labels. Reuse a
label when several predicates need to match the same element.

Track lookups return an empty lyrics map by default. Use `TrackLookup::by_isrc(isrc).with_lyrics()`
or set `lookup.options.lyrics = true` to include lyrics for the unique match. The option
affects the response only. Track list and ID endpoints omit lyrics. The map keys are
BCP 47 tags such as `en` or `pt-BR`, with `und` for an unknown language.

Use `TrackLookup::by_isrc(isrc).with_album(RelatedData::Full).with_tags(RelatedData::None)`
to return the full album and omit track tags. Artist and album lookups also support
`with_tags`. Each relation supports `None`, `Ids`, or `Full`.
`Ids` is the default for both and applies to track list and ID endpoints. The album uses
`RecordRef::None`, `RecordRef::Id`, or `RecordRef::Data`. Each weighted tag uses
`RecordRef::Id` or `RecordRef::Data`. The tag vector is empty when omitted. An album
embedded in a track response includes its own tag IDs.
The options apply after matching. A field filter on the album ID uses
the path `album`, `Id` regardless of which response option you choose.

`Track::derived_from` maps each source track ID to its derivations, including multiple
samples from the same recording.

Artist, album, and track lookups support `with_visuals(VisualData::Primary)` for one
portrait or cover with its renditions. `VisualData::None` omits artwork and styling.
`VisualData::Full` is the default and includes all artwork and colors, plus the
artist's custom font if present. The same visual mode applies to an album embedded
in a track response. As with tag options, this only affects the response.
Visual field filters use the top-level `artwork`, artist or album `theme`, and
artist `font` fields. Artists and albums have one optional color scheme.

Album lookups support `with_tracks(RelatedData::None)` to omit the disc list,
`Ids` for track IDs in disc and play order (the default), and `Full` for track
records in the same order. Embedded tracks follow the requested visual mode
and use the default tag, album, and lyrics detail.

Albums have one optional `album_type`. `AlbumType::Soundtrack` can carry the source work's title.
Tracks use their own release date when present and otherwise inherit the primary album's date.

## API and deployment

[`concentus_server`](crates/concentus_server/) is the native HTTP server.
[`concentus_types`](crates/concentus_types/) defines its request and response types,
and [`concentus`](crates/concentus/) provides the async HTTP client.

The `/api/` routes serve the current Postcard schema.

| Method and path | Postcard body |
| --- | --- |
| `GET /api/{collection}` | Returns `Vec<T>`. Accepts optional `after` and `limit` query parameters. |
| `GET /api/{collection}/{id}` | Returns `T`, or HTTP 404. |
| `POST /api/{collection}/lookup` | Accepts `Lookup<T>` and returns `Option<T>`. |

HTTP bodies use `application/vnd.concentus.postcard`. Build the client and server
against the same version of `concentus_types`.

Run `just dev` from the workspace root. It uses `topcoat dev -p concentus_server`
to watch for changes and print the Web UI address when the server is ready.
For a standalone run, use `cargo run --locked -p concentus_server`.

The Web UI searches the local catalog first. On an empty artist, album, or track
search, it queries the keyless MusicBrainz API and saves the returned music
metadata and relationships in SQLite. The results page opens immediately and
streams matches as each collection is imported. For an artist and title
together, enter `Artist - Title`. Album results are imported per release.
The optional `CONCENTUS_MUSICBRAINZ_ENDPOINT` selects a MusicBrainz-compatible
API endpoint for a mirror or local development.

Album and track covers try the Cover Art Archive by release and then by release
group when available. Missing fields are queued after an import. A background
worker checks the queue every minute for MusicBrainz details, Wikidata artist
metadata, Commons portraits, and LRCLIB lyrics. Successful checks retire their
queue entries even when no data is available. Request failures stay queued
for retry.

Use **Edit metadata** on an artist, album, or track page to save individual
fields, edit grouped related rows together, or queue a field or entire record
for refresh. Field and record history can restore earlier values. Revisions
keep the value before the first edit and the latest value in each UTC minute.

For local testing, the optional `CONCENTUS_LRCLIB_ENDPOINT`,
`CONCENTUS_WIKIDATA_ENDPOINT`, and `CONCENTUS_COMMONS_ENDPOINT` variables
override the respective API URLs.

The Postcard lookup API returns a unique local match immediately. On a true
miss, it can discover records using MusicBrainz for supported identifiers or
search criteria, then reruns the original exact lookup. Ambiguous matches
return no record.
The server listens on `127.0.0.1:3000` by default. Set `HOST` or `PORT` to override it.

On Linux, the database lives at `$XDG_STATE_HOME/concentus/catalog.sqlite3`, defaulting
to `~/.local/state/concentus/catalog.sqlite3`. Set `CONCENTUS_DATABASE` to use a different
file. SQLx creates the database and applies the [initial SQLite schema](crates/concentus_server/migrations/0001_initial.sql)
on startup. The schema groups each record with its child tables and keeps the
enrichment queue and revision history together.
SQLite defines the columns and relationships. Rust parses domain values such as colors,
dates, and enum labels when reading records, so a
future rename can update the stored values and the Rust enum together. External IDs are
indexed by provider and value. For SQL imports, use enum labels like `AppleMusic` or
`Isrc`, or `other:my-provider` for `ExternalIdProvider::Other("my-provider".into())`.
ID values are exact text. Dates use `YYYY`, `YYYY-MM`, or `YYYY-MM-DD` text and
are parsed by the same Rust `PartialDate` type for artists, albums, and tracks.

Tracks, albums, and artists own their artwork rows. Albums and artists own their
theme columns. Tags have their
own records so their IDs can be shared across the catalog.

For direct SQL imports with references between albums and tracks, enable SQLite
foreign keys and insert related records in one transaction.
