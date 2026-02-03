# Jazz Sakura Radio Watcher

A Rust application that monitors [Jazz Sakura Radio](https://asiadreamradio.torontocast.stream/stations/jazzsakuraplayer.html) and logs played songs to a JSON file. Includes a built-in web server to serve the playlist.

## Building

Requires [Rust](https://rustup.rs/) to be installed.

```bash
cargo build --release
```

The binary will be at `target/release/jazz-sakura-watcher`.

## Usage

```bash
# Run with defaults (playlist.jsonl, port 8080)
./target/release/jazz-sakura-watcher

# Custom output file and port via environment variables
OUTPUT=/path/to/playlist.jsonl PORT=3000 ./target/release/jazz-sakura-watcher
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OUTPUT` | `playlist.jsonl` | Output file path |
| `PORT` | `8080` | Web server port |

Polling interval is fixed at 10 minutes.

## Storage Format

Songs are stored as JSON lines (one JSON object per line):

```json
{"timestamp":"2026-02-03 19:22:37","artist":"松尾明トリオ","song":"GEORGIA ON MY MIND","raw":"松尾明トリオ - GEORGIA ON MY MIND","listeners":71}
```

## Web Server Endpoints

| Endpoint | Description |
|----------|-------------|
| `/` or `/tracks` | Paginated track list (10 per page, newest first) |
| `/tracks?page=2` | Get specific page |

Example response:
```json
{
  "tracks": [{"timestamp": "...", "artist": "...", "song": "...", "raw": "...", "listeners": 71}],
  "page": 1,
  "per_page": 10,
  "total": 42,
  "total_pages": 5
}
```

## Data Source

Fetches metadata from the Shoutcast stream at `https://kathy.torontocast.com:3330/stats?json=1`
