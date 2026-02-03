//! Jazz Sakura Radio - Now Playing Watcher
//!
//! Monitors the Jazz Sakura online radio station and logs song changes.
//! https://asiadreamradio.torontocast.stream/stations/jazzsakuraplayer.html

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tiny_http::{Response, Server};

const STATS_URL: &str = "https://kathy.torontocast.com:3330/stats?json=1";
const POLL_INTERVAL_SECS: u64 = 600;
const PAGE_SIZE: usize = 10;

#[derive(Debug, Deserialize)]
struct StreamStats {
    songtitle: String,
    currentlisteners: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Track {
    timestamp: String,
    artist: String,
    song: String,
    raw: String,
    listeners: u32,
}

fn fetch_now_playing() -> Option<StreamStats> {
    ureq::get(STATS_URL)
        .timeout(Duration::from_secs(10))
        .call()
        .ok()?
        .into_json::<StreamStats>()
        .ok()
}

fn parse_song_title(title: &str) -> (String, String) {
    if let Some((artist, song)) = title.split_once(" - ") {
        (artist.trim().to_string(), song.trim().to_string())
    } else {
        (String::new(), title.to_string())
    }
}

fn log_to_file(path: &PathBuf, stats: &StreamStats) -> std::io::Result<()> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut writer = BufWriter::new(file);

    let (artist, song) = parse_song_title(&stats.songtitle);
    let track = Track {
        timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        artist,
        song,
        raw: stats.songtitle.clone(),
        listeners: stats.currentlisteners,
    };

    writeln!(writer, "{}", serde_json::to_string(&track).unwrap())?;
    writer.flush()
}

fn parse_query_param(url: &str, param: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == param {
            return parts.next().map(String::from);
        }
    }
    None
}

fn get_playlist_entries(playlist_path: &PathBuf) -> Vec<Track> {
    let content = fs::read_to_string(playlist_path).unwrap_or_default();
    content
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn start_webserver(port: u16, playlist_path: PathBuf, running: Arc<AtomicBool>) {
    let addr = format!("0.0.0.0:{}", port);
    let server = match Server::http(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to start web server: {}", e);
            return;
        }
    };

    println!("Web server running at http://localhost:{}", port);

    while running.load(Ordering::SeqCst) {
        if let Ok(Some(request)) = server.try_recv() {
            let url = request.url();
            let path = url.split('?').next().unwrap_or(url);

            let response = match path {
                "/" | "/tracks" => {
                    let page: usize = parse_query_param(url, "page")
                        .and_then(|p| p.parse().ok())
                        .unwrap_or(1)
                        .max(1);

                    let entries = get_playlist_entries(&playlist_path);
                    let total = entries.len();
                    let total_pages = total.div_ceil(PAGE_SIZE);

                    // Reverse to show newest first, then paginate
                    let start = (page - 1) * PAGE_SIZE;
                    let page_entries: Vec<_> = entries
                        .into_iter()
                        .rev()
                        .skip(start)
                        .take(PAGE_SIZE)
                        .collect();

                    let json = serde_json::json!({
                        "tracks": page_entries,
                        "page": page,
                        "per_page": PAGE_SIZE,
                        "total": total,
                        "total_pages": total_pages
                    });

                    Response::from_string(json.to_string()).with_header(
                        "Content-Type: application/json"
                            .parse::<tiny_http::Header>()
                            .unwrap(),
                    )
                }
                _ => {
                    let json = serde_json::json!({"error": "Not found"});
                    Response::from_string(json.to_string())
                        .with_status_code(404)
                        .with_header(
                            "Content-Type: application/json"
                                .parse::<tiny_http::Header>()
                                .unwrap(),
                        )
                }
            };

            let _ = request.respond(response);
        }

        thread::sleep(Duration::from_millis(10));
    }
}

fn main() {
    let output: PathBuf = env::var("OUTPUT")
        .unwrap_or_else(|_| "playlist.jsonl".to_string())
        .into();
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    println!("Jazz Sakura Radio Watcher");
    println!("{}", "=".repeat(40));
    println!("Output file: {}", output.display());
    println!("Poll interval: {}s", POLL_INTERVAL_SECS);
    println!("Web server: http://localhost:{}", port);
    println!("Press Ctrl+C to stop\n");

    // Start web server
    let playlist_path = output.clone();
    let server_running = running.clone();

    thread::spawn(move || {
        start_webserver(port, playlist_path, server_running);
    });

    let mut last_song = String::new();

    while running.load(Ordering::SeqCst) {
        if let Some(stats) = fetch_now_playing() {
            if !stats.songtitle.is_empty() && stats.songtitle != last_song {
                if let Err(e) = log_to_file(&output, &stats) {
                    eprintln!("Error writing to file: {}", e);
                } else {
                    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
                    println!("[{}] Now playing: {}", timestamp, stats.songtitle);
                    println!("             Listeners: {}", stats.currentlisteners);
                }
                last_song = stats.songtitle;
            }
        } else {
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
            eprintln!("[{}] Warning: Failed to fetch stream info", timestamp);
        }

        thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
    }

    println!("\nStopping watcher...");
}
