// Cargo.toml dependencies:
// [dependencies]
// gtk = { version = "0.7", package = "gtk4", features = ["v4_10"] }
// reqwest = { version = "0.11", features = ["blocking", "json"] }
// serde = { version = "1.0", features = ["derive"] }
// serde_json = "1.0"
// urlencoding = "2.1"

use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box, Button, Entry, Label, ListBox, ScrolledWindow, 
          Orientation, SearchEntry, DropDown, Grid, Frame, Separator, StringList, Window, Picture, 
          Align};
use gtk::gdk_pixbuf::Pixbuf;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::{File, read_dir, create_dir_all};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::process::{Command, Stdio};
use serde::{Deserialize, Serialize};
use gtk::glib;

// Helper function to escape HTML entities in strings for Pango markup
fn escape_markup(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// Get config directory path
fn get_config_dir() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("movie-database");
    path
}

// Get config file path
fn get_config_file() -> PathBuf {
    let mut path = get_config_dir();
    path.push("config.json");
    path
}

// Configuration structure
#[derive(Serialize, Deserialize, Default, Clone)]
struct Config {
    tmdb_api_key: String,
    #[serde(default)]
    scan_directories: Vec<String>,
    #[serde(default = "default_auto_scan")]
    auto_scan_on_startup: bool,
    #[serde(default = "default_year_cutoff")]
    year_cutoff: i32,
}

fn default_auto_scan() -> bool {
    true  // Enable by default
}

fn default_year_cutoff() -> i32 {
    1966  // Default to pre-1966 movies
}

// Save config to file
fn save_config(config: &Config) -> std::io::Result<()> {
    let config_dir = get_config_dir();
    create_dir_all(&config_dir)?;
    
    let config_file = get_config_file();
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(config_file, json)?;
    
    Ok(())
}

// Load config from file
fn load_config() -> Option<Config> {
    let config_file = get_config_file();
    if !config_file.exists() {
        return None;
    }
    
    let mut file = File::open(config_file).ok()?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).ok()?;
    
    serde_json::from_str(&contents).ok()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CastMember {
    name: String,
    #[serde(default)]
    profile_path: String,  // TMDB profile photo URL
    #[serde(default)]
    character: String,     // Character name
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WatchLogEntry {
    date: String,  // ISO format: "2026-01-01"
    rating: Option<f32>,  // Personal rating 0-10
    comments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Movie {
    id: u32,
    title: String,
    year: u16,
    director: String,
    genre: Vec<String>,
    rating: f32,
    runtime: u16,
    description: String,
    #[serde(default)]
    cast: Vec<String>,  // Keep for backwards compatibility
    #[serde(default)]
    cast_details: Vec<CastMember>,  // New detailed cast info
    file_path: String,
    poster_url: String,
    tmdb_id: u32,
    #[serde(default)]
    imdb_id: String,  // IMDb ID (e.g., "tt0111161")
    #[serde(default)]
    poster_path: String,  // Local cached poster path
    #[serde(default)]
    watch_log: Vec<WatchLogEntry>,  // Watch history with comments
}

#[derive(Debug, Deserialize)]
struct TMDBSearchResponse {
    results: Vec<TMDBMovie>,
}

#[derive(Debug, Deserialize)]
struct TMDBMovie {
    id: u32,
    #[serde(default)]
    release_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TMDBMovieDetails {
    title: String,
    #[serde(default)]
    release_date: String,
    overview: String,
    #[serde(default)]
    vote_average: f32,
    #[serde(default)]
    poster_path: Option<String>,
    #[serde(default)]
    runtime: Option<u16>,
    #[serde(default)]
    genres: Vec<TMDBGenre>,
    #[serde(default)]
    credits: TMDBCredits,
}

#[derive(Debug, Deserialize, Default)]
struct TMDBGenre {
    name: String,
}

#[derive(Debug, Deserialize, Default)]
struct TMDBCredits {
    #[serde(default)]
    cast: Vec<TMDBCast>,
    #[serde(default)]
    crew: Vec<TMDBCrew>,
}

#[derive(Debug, Deserialize)]
struct TMDBCast {
    name: String,
    #[serde(default)]
    character: String,
    #[serde(default)]
    profile_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TMDBCrew {
    name: String,
    job: String,
}

#[derive(Debug, Deserialize)]
struct TMDBExternalIds {
    #[serde(default)]
    imdb_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct MovieDatabase {
    movies: HashMap<u32, Movie>,
    next_id: u32,
    #[serde(skip)]  // Don't serialize file path
    data_file: String,
    #[serde(skip)]  // Don't serialize posters directory path
    posters_dir: String,
    tmdb_api_key: String,
    #[serde(default)]
    tmdb_cache: HashMap<String, CachedTMDBSearch>,  // search_query -> cached results
    #[serde(skip)]  // Don't serialize pixbufs (can't serialize)
    poster_cache: Rc<RefCell<HashMap<u32, Pixbuf>>>,  // movie_id -> cached pixbuf
    #[serde(skip)]  // Cache for search/filter/sort results
    result_cache: RefCell<HashMap<String, Vec<Movie>>>,  // cache_key -> filtered/sorted movies
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedTMDBSearch {
    query: String,
    results: Vec<(u32, String, String, f32)>,  // (tmdb_id, title, year, rating)
    timestamp: u64,  // Unix timestamp
}

impl CachedTMDBSearch {
    fn is_expired(&self, max_age_days: u64) -> bool {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let age_days = (now - self.timestamp) / 86400;
        age_days > max_age_days
    }
}

fn download_poster(poster_url: &str, movie_id: u32, posters_dir: &str) -> Option<String> {
    if poster_url.is_empty() {
        return None;
    }
    
    // Create posters directory if it doesn't exist
    create_dir_all(posters_dir).ok()?;
    
    // Download the poster
    let response = reqwest::blocking::get(poster_url).ok()?;
    let bytes = response.bytes().ok()?;
    
    // Save to local file
    let poster_path = format!("{}/poster_{}.jpg", posters_dir, movie_id);
    let mut file = File::create(&poster_path).ok()?;
    std::io::copy(&mut bytes.as_ref(), &mut file).ok()?;
    
    Some(poster_path)
}

// Async function to fetch metadata for a single movie (non-blocking)
async fn fetch_movie_metadata_async(
    client: &reqwest::Client,
    api_key: &str,
    title: &str,
    file_path: String,
    posters_dir: String,
    year_cutoff: i32,
) -> Option<Movie> {
    let search_url = format!(
        "https://api.themoviedb.org/3/search/movie?api_key={}&query={}",
        api_key,
        urlencoding::encode(title)
    );
    
    let search_response = client
        .get(&search_url)
        .send()
        .await
        .ok()?
        .json::<TMDBSearchResponse>()
        .await
        .ok()?;
    
    if search_response.results.is_empty() {
        return None;
    }
    
    // Prioritize movies before year_cutoff (filter by release_date)
    let movie_id = {
        // First try to find a movie before the cutoff year
        let before_cutoff = search_response.results.iter().find(|movie| {
            if let Some(ref date) = movie.release_date {
                if let Some(year_str) = date.split('-').next() {
                    if let Ok(year) = year_str.parse::<i32>() {
                        return year <= year_cutoff;
                    }
                }
            }
            false
        });
        
        // If found a movie before cutoff, use it. Otherwise fall back to first result
        if let Some(movie) = before_cutoff {
            movie.id
        } else {
            search_response.results[0].id
        }
    };
    
    let details_url = format!(
        "https://api.themoviedb.org/3/movie/{}?api_key={}&append_to_response=credits",
        movie_id, api_key
    );
    
    let details = client
        .get(&details_url)
        .send()
        .await
        .ok()?
        .json::<TMDBMovieDetails>()
        .await
        .ok()?;
    
    let year: u16 = details.release_date
        .split('-')
        .next()
        .and_then(|y| y.parse().ok())
        .unwrap_or(0);
    
    let director = details.credits.crew
        .iter()
        .find(|c| c.job == "Director")
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "Unknown".to_string());
    
    let cast: Vec<String> = details.credits.cast
        .iter()
        .take(5)
        .map(|c| c.name.clone())
        .collect();
    
    let cast_details: Vec<CastMember> = details.credits.cast
        .iter()
        .take(5)
        .map(|c| CastMember {
            name: c.name.clone(),
            character: c.character.clone(),
            profile_path: c.profile_path.as_ref()
                .map(|p| format!("https://image.tmdb.org/t/p/w185{}", p))
                .unwrap_or_default(),
        })
        .collect();
    
    let genres: Vec<String> = details.genres
        .iter()
        .map(|g| g.name.clone())
        .collect();
    
    let poster_url = details.poster_path
        .map(|p| format!("https://image.tmdb.org/t/p/original{}", p))
        .unwrap_or_default();
    
    let poster_path = if !poster_url.is_empty() {
        download_poster(&poster_url, movie_id, &posters_dir).unwrap_or_default()
    } else {
        String::new()
    };
    
    // Fetch IMDb ID from external_ids endpoint
    let external_ids_url = format!(
        "https://api.themoviedb.org/3/movie/{}/external_ids?api_key={}",
        movie_id, api_key
    );
    
    let imdb_id = if let Ok(response) = client.get(&external_ids_url).send().await {
        if let Ok(external_ids) = response.json::<TMDBExternalIds>().await {
            external_ids.imdb_id.unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    
    Some(Movie {
        id: 0,
        title: details.title,
        year,
        director,
        genre: if genres.is_empty() { vec!["Unknown".to_string()] } else { genres },
        rating: details.vote_average,
        runtime: details.runtime.unwrap_or(0),
        description: details.overview,
        cast,
        cast_details,
        file_path,
        poster_url,
        tmdb_id: movie_id,
        imdb_id,
        poster_path,
        watch_log: Vec::new(),
    })
}

impl MovieDatabase {
    fn new(data_file: &str, posters_dir: &str, api_key: &str) -> Self {
        let mut db = MovieDatabase {
            movies: HashMap::new(),
            next_id: 1,
            data_file: data_file.to_string(),
            posters_dir: posters_dir.to_string(),
            tmdb_api_key: api_key.to_string(),
            tmdb_cache: HashMap::new(),
            poster_cache: Rc::new(RefCell::new(HashMap::new())),
            result_cache: RefCell::new(HashMap::new()),
        };
        db.load_from_file();
        db
    }

    fn add_movie(&mut self, mut movie: Movie) {
        movie.id = self.next_id;
        self.movies.insert(self.next_id, movie);
        self.next_id += 1;
        self.invalidate_result_cache();
        self.save_to_file();
    }

    fn search_by_title(&self, query: &str) -> Vec<Movie> {
        let query_lower = query.to_lowercase();
        self.movies
            .values()
            .filter(|m| m.title.to_lowercase().contains(&query_lower))
            .cloned()
            .collect()
    }

    fn search_by_genre(&self, genre: &str) -> Vec<Movie> {
        if genre.is_empty() || genre == "All" {
            return self.list_all();
        }
        let genre_lower = genre.to_lowercase();
        self.movies
            .values()
            .filter(|m| m.genre.iter().any(|g| g.to_lowercase().contains(&genre_lower)))
            .cloned()
            .collect()
    }

    fn delete_movie(&mut self, id: u32) -> bool {
        if self.movies.remove(&id).is_some() {
            self.invalidate_result_cache();
            self.save_to_file();
            true
        } else {
            false
        }
    }

    fn save_to_file(&self) {
        // Serialize entire database to JSON (including cache)
        let json = serde_json::to_string_pretty(&self).expect("Unable to serialize database");
        std::fs::write(&self.data_file, json).expect("Unable to write to file");
    }

    fn load_from_file(&mut self) {
        if !Path::new(&self.data_file).exists() {
            return;
        }

        // Try to load new format (entire database as JSON)
        if let Ok(contents) = std::fs::read_to_string(&self.data_file) {
            if let Ok(loaded_db) = serde_json::from_str::<MovieDatabase>(&contents) {
                // Successfully loaded new format
                self.movies = loaded_db.movies;
                self.next_id = loaded_db.next_id;
                self.tmdb_api_key = loaded_db.tmdb_api_key;
                self.tmdb_cache = loaded_db.tmdb_cache;
                
                // Migrate old poster paths to new location
                self.migrate_poster_paths();
                
                return;
            }
        }
        
        // Fall back to old format (line-by-line movies) for backwards compatibility
        if let Ok(file) = File::open(&self.data_file) {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if let Ok(movie) = serde_json::from_str::<Movie>(&line) {
                        let id = movie.id;
                        self.movies.insert(id, movie);
                        if id >= self.next_id {
                            self.next_id = id + 1;
                        }
                    }
                }
            }
        }
    }

    fn migrate_poster_paths(&mut self) {
        // Update old poster paths to use new location
        let mut needs_save = false;
        
        for movie in self.movies.values_mut() {
            if !movie.poster_path.is_empty() {
                // Check if using old "posters/" or "~/posters/" path
                if movie.poster_path.starts_with("posters/") || movie.poster_path.starts_with("~/posters/") {
                    // Extract just the filename
                    if let Some(filename) = movie.poster_path.split('/').last() {
                        // Update to new path
                        let new_path = format!("{}/{}", self.posters_dir, filename);
                        movie.poster_path = new_path;
                        needs_save = true;
                    }
                }
            }
        }
        
        // Save if we made any changes
        if needs_save {
            self.save_to_file();
        }
    }

    fn list_all(&self) -> Vec<Movie> {
        let mut movies: Vec<Movie> = self.movies.values().cloned().collect();
        movies.sort_by(|a, b| a.title.cmp(&b.title));
        movies
    }
    
    // TMDB Cache methods
    fn get_cached_search(&self, query: &str) -> Option<Vec<(u32, String, String, f32)>> {
        let cache_max_age_days = 30;  // Cache expires after 30 days
        
        if let Some(cached) = self.tmdb_cache.get(query) {
            if !cached.is_expired(cache_max_age_days) {
                return Some(cached.results.clone());
            }
        }
        None
    }
    
    fn cache_search_results(&mut self, query: String, results: Vec<(u32, String, String, f32)>) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let cached = CachedTMDBSearch {
            query: query.clone(),
            results,
            timestamp,
        };
        
        self.tmdb_cache.insert(query, cached);
        // Save to persist cache
        self.save_to_file();
    }
    
    // Result cache methods for search/filter/sort
    fn get_cached_results(&self, cache_key: &str) -> Option<Vec<Movie>> {
        self.result_cache.borrow().get(cache_key).cloned()
    }
    
    fn cache_results(&self, cache_key: String, results: Vec<Movie>) {
        self.result_cache.borrow_mut().insert(cache_key, results);
    }
    
    fn invalidate_result_cache(&self) {
        self.result_cache.borrow_mut().clear();
    }
}

fn create_movie_row(movie: &Movie, poster_cache: &Rc<RefCell<HashMap<u32, Pixbuf>>>) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    
    // Store the movie ID in the row's name property for later retrieval
    row.set_widget_name(&movie.id.to_string());
    
    let hbox = Box::new(Orientation::Horizontal, 12);
    hbox.set_margin_start(12);
    hbox.set_margin_end(12);
    hbox.set_margin_top(8);
    hbox.set_margin_bottom(8);

    // Add poster thumbnail
    let poster_container = gtk::Overlay::new();
    poster_container.set_size_request(60, 90);
    
    let poster_box = Box::new(Orientation::Vertical, 0);
    poster_box.set_size_request(60, 90);
    
    if !movie.poster_path.is_empty() && Path::new(&movie.poster_path).exists() {
        // Safe cache access - get or insert in one operation
        let pixbuf_opt = {
            let cache = poster_cache.borrow();
            cache.get(&movie.id).cloned()
        };
        
        if let Some(pixbuf) = pixbuf_opt {
            // Use cached THUMBNAIL (already at display size!)
            let picture = Picture::for_pixbuf(&pixbuf);
            picture.set_can_shrink(true);
            poster_box.append(&picture);
        } else {
            // Load from disk and scale DOWN immediately, cache ONLY the thumbnail
            if let Ok(pixbuf) = Pixbuf::from_file(&movie.poster_path) {
                // Scale to display size IMMEDIATELY (don't cache full res!)
                if let Some(thumbnail) = pixbuf.scale_simple(60, 90, gtk::gdk_pixbuf::InterpType::Bilinear) {
                    // Cache the THUMBNAIL only (saves massive memory)
                    if let Ok(mut cache) = poster_cache.try_borrow_mut() {
                        cache.insert(movie.id, thumbnail.clone());
                    }
                    
                    let picture = Picture::for_pixbuf(&thumbnail);
                    picture.set_can_shrink(true);
                    poster_box.append(&picture);
                }
            }
        }
    } else {
        // Placeholder for missing poster
        let placeholder = Label::new(Some("🎬"));
        placeholder.set_markup("<span size='xx-large'>🎬</span>");
        poster_box.append(&placeholder);
    }
    
    poster_container.set_child(Some(&poster_box));
    
    // Add "SEEN" badge if movie has been watched
    if !movie.watch_log.is_empty() {
        let seen_label = Label::new(Some("SEEN"));
        seen_label.set_markup("<span foreground='red' weight='bold' size='large'>SEEN</span>");
        seen_label.set_halign(Align::Center);
        seen_label.set_valign(Align::Center);
        poster_container.add_overlay(&seen_label);
    }
    
    hbox.append(&poster_container);

    let vbox = Box::new(Orientation::Vertical, 4);
    
    let title_label = Label::new(Some(&format!("{} ({})", movie.title, movie.year)));
    title_label.set_xalign(0.0);
    // Escape special characters for Pango markup
    let escaped_title = escape_markup(&movie.title);
    title_label.set_markup(&format!("<b>{}</b> ({})", escaped_title, movie.year));
    
    let info_label = Label::new(Some(&format!("⭐ {:.1}/10 | {} | {} min", 
        movie.rating, movie.genre.join(", "), movie.runtime)));
    info_label.set_xalign(0.0);
    info_label.set_opacity(0.7);
    
    let director_label = Label::new(Some(&format!("Director: {}", movie.director)));
    director_label.set_xalign(0.0);
    director_label.set_opacity(0.6);

    vbox.append(&title_label);
    vbox.append(&info_label);
    vbox.append(&director_label);
    
    hbox.append(&vbox);
    row.set_child(Some(&hbox));
    
    row
}

fn create_movie_row_with_context(
    movie: &Movie, 
    poster_cache: &Rc<RefCell<HashMap<u32, Pixbuf>>>,
    db: &Rc<RefCell<MovieDatabase>>,
) -> gtk::ListBoxRow {
    let row = create_movie_row(movie, poster_cache);
    
    // Add right-click context menu
    let gesture = gtk::GestureClick::new();
    gesture.set_button(3); // Right mouse button
    
    let movie_id = movie.id;
    let file_path = movie.file_path.clone();
    let movie_title = movie.title.clone();
    let db_clone = db.clone();
    let row_clone = row.clone();
    
    gesture.connect_released(move |_, _, x, y| {
        let menu_model = gtk::gio::Menu::new();
        menu_model.append(Some("▶️ Play in VLC"), Some("movie.play"));
        menu_model.append(Some("🗑️ Delete Movie Metadata"), Some("movie.delete"));
        
        let menu = gtk::PopoverMenu::from_model(Some(&menu_model));
        menu.set_parent(&row_clone);
        menu.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        
        // Create action group
        let actions = gtk::gio::SimpleActionGroup::new();
        
        let play_action = gtk::gio::SimpleAction::new("play", None);
        let file_path_clone = file_path.clone();
        let movie_title_clone = movie_title.clone();
        let db_clone2 = db_clone.clone();
        let menu_clone = menu.clone();
        play_action.connect_activate(move |_, _| {
            if !file_path_clone.is_empty() && Path::new(&file_path_clone).exists() {
                // Launch VLC
                match Command::new("vlc")
                    .arg(&file_path_clone)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(_) => {
                        // Auto-log to watch history
                        let mut db_mut = db_clone2.borrow_mut();
                        if let Some(movie) = db_mut.movies.get_mut(&movie_id) {
                            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                            let entry = WatchLogEntry {
                                date: today,
                                rating: None,
                                comments: String::from("Watched"),
                            };
                            movie.watch_log.push(entry);
                        }
                        db_mut.invalidate_result_cache();
                        db_mut.save_to_file();
                        eprintln!("Playing: {}", movie_title_clone);
                    }
                    Err(_) => {
                        // Try flatpak version
                        match Command::new("flatpak")
                            .args(["run", "org.videolan.VLC", &file_path_clone])
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn()
                        {
                            Ok(_) => {
                                let mut db_mut = db_clone2.borrow_mut();
                                if let Some(movie) = db_mut.movies.get_mut(&movie_id) {
                                    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                                    let entry = WatchLogEntry {
                                        date: today,
                                        rating: None,
                                        comments: String::from("Watched"),
                                    };
                                    movie.watch_log.push(entry);
                                }
                                db_mut.invalidate_result_cache();
                                db_mut.save_to_file();
                                eprintln!("Playing: {}", movie_title_clone);
                            }
                            Err(_) => {
                                eprintln!("VLC not found");
                            }
                        }
                    }
                }
            }
            menu_clone.popdown();
        });
        
        // Delete action
        let delete_action = gtk::gio::SimpleAction::new("delete", None);
        let db_clone3 = db_clone.clone();
        let menu_clone2 = menu.clone();
        let movie_title_clone2 = movie_title.clone();
        let row_clone2 = row_clone.clone();
        delete_action.connect_activate(move |_, _| {
            // Delete the movie from database
            let mut db_mut = db_clone3.borrow_mut();
            if db_mut.delete_movie(movie_id) {
                eprintln!("Deleted movie metadata: {}", movie_title_clone2);
                drop(db_mut);
                
                // Remove the row from UI
                if let Some(parent) = row_clone2.parent() {
                    if let Some(list_box) = parent.downcast_ref::<ListBox>() {
                        list_box.remove(&row_clone2);
                    }
                }
            } else {
                eprintln!("Failed to delete movie");
            }
            menu_clone2.popdown();
        });
        
        actions.add_action(&play_action);
        actions.add_action(&delete_action);
        menu.insert_action_group("movie", Some(&actions));
        
        menu.popup();
    });
    
    row.add_controller(gesture);
    
    row
}

fn create_movie_grid_item(movie: &Movie, poster_cache: &Rc<RefCell<HashMap<u32, Pixbuf>>>) -> gtk::FlowBoxChild {
    let child = gtk::FlowBoxChild::new();
    child.set_widget_name(&movie.id.to_string());
    
    let vbox = Box::new(Orientation::Vertical, 8);
    vbox.set_size_request(180, 320);
    
    // Poster with overlay
    let poster_container = gtk::Overlay::new();
    poster_container.set_size_request(160, 240);
    
    let poster_box = Box::new(Orientation::Vertical, 0);
    poster_box.set_size_request(160, 240);
    
    if !movie.poster_path.is_empty() && Path::new(&movie.poster_path).exists() {
        // Check if we have a cached thumbnail (safe access)
        let has_cached = {
            let cache = poster_cache.borrow();
            cache.get(&movie.id).is_some()
        };
        
        if has_cached {
            // We have SOME cached version - but it might be 60x90 for list view
            // Just load from disk at the size we need (fast enough for grid view)
            if let Ok(pixbuf) = Pixbuf::from_file(&movie.poster_path) {
                if let Some(scaled) = pixbuf.scale_simple(160, 240, gtk::gdk_pixbuf::InterpType::Bilinear) {
                    let picture = Picture::for_pixbuf(&scaled);
                    picture.set_can_shrink(true);
                    poster_box.append(&picture);
                }
            }
        } else {
            // No cache at all - load and cache at list view size (60x90) for later
            if let Ok(pixbuf) = Pixbuf::from_file(&movie.poster_path) {
                // Cache thumbnail for list view
                if let Some(thumbnail) = pixbuf.scale_simple(60, 90, gtk::gdk_pixbuf::InterpType::Bilinear) {
                    if let Ok(mut cache) = poster_cache.try_borrow_mut() {
                        cache.insert(movie.id, thumbnail);
                    }
                }
                
                // Display at grid size
                if let Some(scaled) = pixbuf.scale_simple(160, 240, gtk::gdk_pixbuf::InterpType::Bilinear) {
                    let picture = Picture::for_pixbuf(&scaled);
                    picture.set_can_shrink(true);
                    poster_box.append(&picture);
                }
            }
        }
    } else {
        let placeholder = Label::new(Some("🎬"));
        placeholder.set_markup("<span size='70000'>🎬</span>");
        poster_box.append(&placeholder);
    }
    
    poster_container.set_child(Some(&poster_box));
    
    // Add "SEEN" badge if movie has been watched
    if !movie.watch_log.is_empty() {
        let seen_label = Label::new(Some("SEEN"));
        seen_label.set_markup("<span foreground='red' weight='bold' size='x-large'>SEEN</span>");
        seen_label.set_halign(Align::Center);
        seen_label.set_valign(Align::Center);
        poster_container.add_overlay(&seen_label);
    }
    
    vbox.append(&poster_container);
    
    // Title and year
    let title_label = Label::new(Some(&format!("{} ({})", movie.title, movie.year)));
    title_label.set_wrap(true);
    title_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    title_label.set_max_width_chars(20);
    title_label.set_justify(gtk::Justification::Center);
    let escaped_title = escape_markup(&movie.title);
    title_label.set_markup(&format!("<b>{}</b>\n<small>({})</small>", escaped_title, movie.year));
    
    // Rating
    let rating_label = Label::new(Some(&format!("⭐ {:.1}/10", movie.rating)));
    rating_label.set_opacity(0.8);
    
    vbox.append(&title_label);
    vbox.append(&rating_label);
    
    child.set_child(Some(&vbox));
    child
}

fn show_api_key_dialog(window: &ApplicationWindow) -> Option<String> {
    // Try to load existing config first
    if let Some(config) = load_config() {
        if !config.tmdb_api_key.is_empty() {
            println!("Loaded API key from config");
            return Some(config.tmdb_api_key);
        }
    }
    
    let dialog = Window::builder()
        .title("TMDB API Key Required")
        .modal(true)
        .transient_for(window)
        .default_width(500)
        .default_height(220)
        .build();

    let content = Box::new(Orientation::Vertical, 12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);

    let info_label = Label::new(Some(
        "To fetch movie metadata, you need a TMDB API key.\n\
        Get one free at: https://www.themoviedb.org/settings/api\n\n\
        Enter your API key below (it will be saved for future use):"
    ));
    info_label.set_wrap(true);

    let api_entry = Entry::new();
    api_entry.set_placeholder_text(Some("Enter TMDB API key"));
    api_entry.set_visibility(false);  // Hide the key like a password

    let button_box = Box::new(Orientation::Horizontal, 8);
    button_box.set_halign(gtk::Align::End);
    let ok_btn = Button::with_label("OK");
    button_box.append(&ok_btn);

    content.append(&info_label);
    content.append(&api_entry);
    content.append(&button_box);

    dialog.set_child(Some(&content));

    let api_key = Rc::new(RefCell::new(String::new()));
    let api_key_clone = api_key.clone();
    let dialog_clone = dialog.clone();
    
    ok_btn.connect_clicked(move |_| {
        let key = api_entry.text().to_string();
        if !key.is_empty() {
            // Save the API key to config, preserving existing settings
            let mut config = load_config().unwrap_or_default();
            config.tmdb_api_key = key.clone();
            
            if let Err(e) = save_config(&config) {
                eprintln!("Warning: Could not save config: {}", e);
            } else {
                println!("API key saved to config");
            }
            *api_key_clone.borrow_mut() = key;
        }
        dialog_clone.close();
    });

    dialog.present();
    
    while dialog.is_visible() {
        gtk::glib::MainContext::default().iteration(true);
    }
    
    let key = api_key.borrow().clone();
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

// Helper function to recursively scan directories for video files
fn scan_directory_recursive(
    dir: &Path,
    video_extensions: &[&str],
    files: &mut Vec<(String, String)>,
) {
    if let Ok(entries) = read_dir(dir) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            
            if entry_path.is_dir() {
                // Recursively scan subdirectories
                scan_directory_recursive(&entry_path, video_extensions, files);
            } else if entry_path.is_file() {
                if let Some(ext) = entry_path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if video_extensions.contains(&ext_str.as_str()) {
                        if let Some(file_name) = entry_path.file_stem() {
                            let title = file_name.to_string_lossy().to_string();
                            let file_path_str = entry_path.to_string_lossy().to_string();
                            
                            let clean_title = title
                                .replace('.', " ")
                                .replace('_', " ")
                                .trim()
                                .to_string();
                            
                            files.push((clean_title, file_path_str));
                        }
                    }
                }
            }
        }
    }
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Mark's Movie Database (MMDB)")
        .default_width(1000)
        .default_height(700)
        .maximized(true)
        .build();

    let api_key = match show_api_key_dialog(&window) {
        Some(key) => key,
        None => {
            eprintln!("No API key provided. Exiting.");
            return;
        }
    };

    // Create data directory in home folder for consistent storage
    let data_dir = dirs::home_dir()
        .expect("Could not find home directory")
        .join(".movie_database");
    std::fs::create_dir_all(&data_dir).expect("Could not create data directory");
    
    let db_path = data_dir.join("movies.db").to_string_lossy().to_string();
    let posters_dir = data_dir.join("posters").to_string_lossy().to_string();
    std::fs::create_dir_all(&posters_dir).expect("Could not create posters directory");

    let db = Rc::new(RefCell::new(MovieDatabase::new(&db_path, &posters_dir, &api_key)));
    
    // Get poster cache reference for passing to create_movie_row
    let poster_cache = db.borrow().poster_cache.clone();

    let main_box = Box::new(Orientation::Vertical, 0);

    let header = Box::new(Orientation::Horizontal, 12);
    header.set_margin_start(12);
    header.set_margin_end(12);
    header.set_margin_top(12);
    header.set_margin_bottom(12);

    let title_label = Label::new(Some("📽️ Mark's Movie Database"));
    title_label.set_markup("<span size='x-large' weight='bold'>📽️ Mark's Movie Database</span>");
    
    let scan_button = Button::with_label("📁 Scan Directory");
    let add_button = Button::with_label("➕ Add Movie");
    let refresh_button = Button::with_label("🔄 Refresh Metadata");
    let refresh_all_button = Button::with_label("🔄 Refresh All");
    refresh_all_button.set_tooltip_text(Some("Refresh metadata and posters for ALL movies"));
    let edit_button = Button::with_label("✏️ Edit Metadata");
    let select_version_button = Button::with_label("🎞️ Wrong Movie?");
    let stats_button = Button::with_label("📊 Statistics");
    let settings_button = Button::with_label("⚙️ Settings");
    
    header.append(&title_label);
    header.append(&Box::new(Orientation::Horizontal, 0));
    header.set_hexpand(true);
    title_label.set_hexpand(true);
    header.append(&stats_button);
    header.append(&settings_button);
    header.append(&refresh_all_button);
    header.append(&edit_button);
    header.append(&select_version_button);
    header.append(&refresh_button);
    header.append(&scan_button);
    header.append(&add_button);

    main_box.append(&header);
    main_box.append(&Separator::new(Orientation::Horizontal));

    let status_bar = Label::new(Some("Ready"));
    status_bar.set_xalign(0.0);
    status_bar.set_margin_start(12);
    status_bar.set_margin_end(12);
    status_bar.set_margin_top(6);
    status_bar.set_margin_bottom(6);
    main_box.append(&status_bar);

    let search_box = Box::new(Orientation::Horizontal, 12);
    search_box.set_margin_start(12);
    search_box.set_margin_end(12);
    search_box.set_margin_top(12);
    search_box.set_margin_bottom(12);

    let search_entry = SearchEntry::new();
    search_entry.set_placeholder_text(Some("Search movies..."));
    search_entry.set_hexpand(true);

    let genres = StringList::new(&["All", "Action", "Comedy", "Drama", "Film Noir", "Horror", "Sci-Fi", "Thriller", "Romance"]);
    let genre_dropdown = DropDown::new(Some(genres), None::<gtk::Expression>);
    genre_dropdown.set_selected(0);

    let sort_options = StringList::new(&["Title (A-Z)", "Year (Newest)", "Year (Oldest)", "Rating (High-Low)", "Rating (Low-High)", "Date Added (Newest)", "Date Added (Oldest)"]);
    let sort_dropdown = DropDown::new(Some(sort_options), None::<gtk::Expression>);
    sort_dropdown.set_selected(0);

    search_box.append(&search_entry);
    search_box.append(&Label::new(Some("Genre:")));
    search_box.append(&genre_dropdown);
    search_box.append(&Label::new(Some("Sort:")));
    search_box.append(&sort_dropdown);
    search_box.append(&Label::new(Some("View:")));
    
    // View toggle button
    let view_toggle = Button::with_label("📋 List");
    view_toggle.set_tooltip_text(Some("Switch between list and grid view"));
    search_box.append(&view_toggle);
    
    main_box.append(&search_box);

    let scrolled = ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);
    
    // List view (default)
    let list_box = ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::Single);
    
    // Grid view (alternative)
    let grid_flow = gtk::FlowBox::new();
    grid_flow.set_selection_mode(gtk::SelectionMode::Single);
    grid_flow.set_column_spacing(12);
    grid_flow.set_row_spacing(12);
    grid_flow.set_margin_start(12);
    grid_flow.set_margin_end(12);
    grid_flow.set_margin_top(12);
    grid_flow.set_margin_bottom(12);
    grid_flow.set_homogeneous(true);
    grid_flow.set_min_children_per_line(2);
    grid_flow.set_max_children_per_line(8);
    
    // Start with list view
    scrolled.set_child(Some(&list_box));
    main_box.append(&scrolled);

    // Container for details section with toggle
    let details_container = Box::new(Orientation::Vertical, 0);
    
    // Toggle button bar
    let toggle_bar = Box::new(Orientation::Horizontal, 0);
    toggle_bar.set_margin_start(12);
    toggle_bar.set_margin_end(12);
    
    let toggle_button = Button::with_label("▼ Hide Details");
    toggle_button.set_tooltip_text(Some("Click to show/hide movie details"));
    toggle_bar.append(&toggle_button);
    details_container.append(&toggle_bar);

    let details_frame = Frame::new(Some("Movie Details"));
    details_frame.set_margin_start(12);
    details_frame.set_margin_end(12);
    details_frame.set_margin_top(0);
    details_frame.set_margin_bottom(12);

    let details_main_box = Box::new(Orientation::Horizontal, 12);
    details_main_box.set_margin_start(12);
    details_main_box.set_margin_end(12);
    details_main_box.set_margin_top(12);
    details_main_box.set_margin_bottom(12);

    // Poster display area
    let poster_display = Picture::new();
    poster_display.set_size_request(200, 300);
    poster_display.set_can_shrink(true);
    poster_display.set_halign(Align::Start);
    poster_display.set_valign(Align::Start);
    details_main_box.append(&poster_display);

    let details_box = Box::new(Orientation::Vertical, 8);
    details_box.set_hexpand(true);

    let details_label = Label::new(Some("Select a movie to view details"));
    details_label.set_xalign(0.0);
    details_label.set_wrap(true);
    details_box.append(&details_label);

    let action_box = Box::new(Orientation::Horizontal, 8);
    let play_button = Button::with_label("▶️ Play in VLC");
    let show_cast_button = Button::with_label("⭐ Show Cast");
    let watch_log_button = Button::with_label("📝 Watch Log");
    let associate_file_button = Button::with_label("📎 Associate File");
    let delete_button = Button::with_label("🗑️ Delete");
    action_box.append(&play_button);
    action_box.append(&show_cast_button);
    action_box.append(&watch_log_button);
    action_box.append(&associate_file_button);
    action_box.append(&delete_button);
    details_box.append(&action_box);

    details_main_box.append(&details_box);
    details_frame.set_child(Some(&details_main_box));
    details_container.append(&details_frame);
    
    main_box.append(&details_container);
    
    // Toggle button handler
    let details_frame_clone = details_frame.clone();
    let is_visible = Rc::new(RefCell::new(true));
    let is_visible_clone = is_visible.clone();
    toggle_button.connect_clicked(move |button| {
        let mut visible = is_visible_clone.borrow_mut();
        *visible = !*visible;
        
        if *visible {
            details_frame_clone.set_visible(true);
            button.set_label("▼ Hide Details");
        } else {
            details_frame_clone.set_visible(false);
            button.set_label("▲ Show Details");
        }
    });

    window.set_child(Some(&main_box));

    // View toggle state and handler
    let is_grid_view = Rc::new(RefCell::new(false));
    let is_grid_view_clone = is_grid_view.clone();
    let scrolled_clone = scrolled.clone();
    let list_box_clone_toggle = list_box.clone();
    let grid_flow_clone = grid_flow.clone();
    let db_clone_toggle = db.clone();
    let poster_cache_clone_toggle = poster_cache.clone();
    
    view_toggle.connect_clicked(move |button| {
        let mut is_grid = is_grid_view_clone.borrow_mut();
        *is_grid = !*is_grid;
        
        if *is_grid {
            // Switch to grid view
            button.set_label("🎞️ Grid");
            scrolled_clone.set_child(Some(&grid_flow_clone));
            
            // Populate grid with current movies
            while let Some(child) = grid_flow_clone.first_child() {
                grid_flow_clone.remove(&child);
            }
            let movies = db_clone_toggle.borrow().list_all();
            for movie in &movies {
                let item = create_movie_grid_item(&movie, &poster_cache_clone_toggle);
                grid_flow_clone.append(&item);
            }
        } else {
            // Switch to list view
            button.set_label("📋 List");
            scrolled_clone.set_child(Some(&list_box_clone_toggle));
        }
    });

    // Show window first for fast startup
    window.present();
    
    // Defer initial list population until after window is visible (prevents slow startup)
    let db_clone = db.clone();
    let list_box_clone = list_box.clone();
    let poster_cache_clone = poster_cache.clone();
    glib::idle_add_local_once(move || {
        let movies = db_clone.borrow().list_all();
        for movie in &movies {
            let row = create_movie_row(movie, &poster_cache_clone);
            list_box_clone.append(&row);
        }
    });

    // Auto-scan on startup if enabled
    let config = load_config().unwrap_or_default();
    if config.auto_scan_on_startup && !config.scan_directories.is_empty() {
        let db_clone = db.clone();
        let list_box_clone = list_box.clone();
        let status_bar_clone = status_bar.clone();
        let window_clone = window.clone();
        let poster_cache_clone = poster_cache.clone();
        
        // Ask user if they want to scan
        let dialog = gtk::AlertDialog::builder()
            .message("Auto-Scan")
            .detail(&format!(
                "Found {} configured director{}.\n\nWould you like to scan for new movies?",
                config.scan_directories.len(),
                if config.scan_directories.len() == 1 { "y" } else { "ies" }
            ))
            .buttons(vec!["Skip", "Scan Now"])
            .cancel_button(0)
            .default_button(1)
            .build();
        
        let scan_dirs = config.scan_directories.clone();
        let api_key = db_clone.borrow().tmdb_api_key.clone();
        let posters_dir = db_clone.borrow().posters_dir.clone();
        let year_cutoff = config.year_cutoff;
        
        dialog.choose(Some(&window_clone), None::<&gtk::gio::Cancellable>, move |response| {
            if let Ok(1) = response {
                // User chose "Scan Now"
                status_bar_clone.set_text("Auto-scanning configured directories...");
                
                // Spawn auto-scan in background
                let (sender, receiver) = async_channel::unbounded::<(String, String, Option<Movie>)>();
                
                let api_key_clone = api_key.clone();
                let scan_dirs_clone = scan_dirs.clone();
                let year_cutoff_clone = year_cutoff;
                
                // Extract existing file paths before spawning thread (Rc can't be sent between threads)
                let existing_paths: std::collections::HashSet<String> = db_clone.borrow()
                    .movies
                    .values()
                    .map(|m| m.file_path.clone())
                    .collect();
                
                std::thread::spawn(move || {
                    // Use tokio runtime for async operations
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    
                    runtime.block_on(async {
                        // Collect all video files first (recursively)
                        let mut files_to_process = Vec::new();
                        let video_extensions = vec!["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v"];
                        
                        for scan_dir in &scan_dirs_clone {
                            let _ = sender.send_blocking(("status".to_string(), format!("Scanning: {} (including subdirectories)...", scan_dir), None));
                            
                            let path = Path::new(scan_dir);
                            scan_directory_recursive(path, &video_extensions, &mut files_to_process);
                        }
                        
                        // Filter out files that already exist in database (using pre-extracted paths)
                        
                        let new_files: Vec<_> = files_to_process.into_iter()
                            .filter(|(_, file_path)| !existing_paths.contains(file_path))
                            .collect();
                        
                        if new_files.is_empty() {
                            let _ = sender.send_blocking(("status".to_string(), "No new movies found - all files already in database".to_string(), None));
                            let _ = sender.send_blocking(("complete".to_string(), String::new(), None));
                            return;
                        }
                        
                        let _ = sender.send_blocking(("status".to_string(), format!("Found {} new video files (skipped {} existing), fetching metadata in parallel...", new_files.len(), existing_paths.len()), None));
                        
                        // Process files in parallel batches of 10
                        let client = reqwest::Client::new();
                        let batch_size = 10;
                        
                        for batch in new_files.chunks(batch_size) {
                            let futures: Vec<_> = batch.iter()
                                .map(|(clean_title, file_path_str)| {
                                    let api_key = api_key_clone.clone();
                                    let title = clean_title.clone();
                                    let file_path = file_path_str.clone();
                                    let client = client.clone();
                                    let sender = sender.clone();
                                    let posters_dir = posters_dir.clone();
                                    
                                    async move {
                                        let _ = sender.send_blocking(("status".to_string(), format!("Fetching: {}", title), None));
                                        
                                        match fetch_movie_metadata_async(&client, &api_key, &title, file_path.clone(), posters_dir.clone(), year_cutoff_clone).await {
                                            Some(movie) => {
                                                let _ = sender.send_blocking(("add".to_string(), format!("✓ Found: {}", title), Some(movie)));
                                            }
                                            None => {
                                                // Create basic entry without metadata
                                                let movie = Movie {
                                                    id: 0,
                                                    title: title.clone(),
                                                    year: 0,
                                                    director: String::from("Unknown"),
                                                    genre: vec![String::from("Uncategorized")],
                                                    rating: 0.0,
                                                    runtime: 0,
                                                    description: String::from("Metadata not found"),
                                                    cast: vec![],
                                                    cast_details: vec![],
                                                    file_path,
                                                    poster_url: String::new(),
                                                    tmdb_id: 0,
                                                    imdb_id: String::new(),
                                                    poster_path: String::new(),
                                                    watch_log: Vec::new(),
                                                };
                                                let _ = sender.send_blocking(("add".to_string(), format!("⚠ Added without metadata: {}", title), Some(movie)));
                                            }
                                        }
                                    }
                                })
                                .collect();
                            
                            // Wait for this batch to complete
                            futures::future::join_all(futures).await;
                        }
                        
                        let _ = sender.send_blocking(("complete".to_string(), String::new(), None));
                    });
                });
        
        // Handle messages on main thread
        glib::spawn_future_local(async move {
            let mut new_movies_count = 0;
            while let Ok((msg_type, status, movie_opt)) = receiver.recv().await {
                match msg_type.as_str() {
                    "status" => {
                        status_bar_clone.set_text(&status);
                    }
                    "add" => {
                        if let Some(movie) = movie_opt {
                            // Check if movie already exists
                            let exists = db_clone.borrow().movies.values()
                                .any(|m| m.file_path == movie.file_path);
                            
                            if !exists {
                                db_clone.borrow_mut().add_movie(movie.clone());
                                new_movies_count += 1;
                                
                                // Add to UI
                                let row = create_movie_row(&movie, &poster_cache_clone);
                                list_box_clone.append(&row);
                            }
                        }
                        status_bar_clone.set_text(&status);
                    }
                    "complete" => {
                        if new_movies_count > 0 {
                            status_bar_clone.set_text(&format!("Auto-scan complete! Added {} new movies", new_movies_count));
                        } else {
                            status_bar_clone.set_text("Auto-scan complete - no new movies found");
                        }
                        break;
                    }
                    _ => {}
                }
            }
        });
        } else {
            // User chose "Skip"
            status_bar_clone.set_text("Auto-scan skipped");
        }
        });
    }

    // Helper function to refresh list with current filters and sorting
    fn refresh_movie_list(
        list_box: &ListBox,
        grid_flow: &gtk::FlowBox,
        is_grid_view: bool,
        db: &Rc<RefCell<MovieDatabase>>,
        search_query: &str,
        genre_filter: &str,
        sort_by: &str,
        poster_cache: &Rc<RefCell<HashMap<u32, Pixbuf>>>,
    ) {
        // Clear existing items from both views
        while let Some(child) = list_box.first_child() {
            list_box.remove(&child);
        }
        while let Some(child) = grid_flow.first_child() {
            grid_flow.remove(&child);
        }

        // Create cache key from current filters
        let cache_key = format!("{}|{}|{}", search_query, genre_filter, sort_by);
        
        // Check cache first - do all database access in one scope
        let results = {
            let db_ref = db.borrow();
            if let Some(cached) = db_ref.get_cached_results(&cache_key) {
                cached
            } else {
                // Need to compute - drop the borrow first
                drop(db_ref);
                
                // Cache miss - compute results
                let mut results = if search_query.is_empty() {
                    db.borrow().search_by_genre(genre_filter)
                } else {
                    db.borrow().search_by_title(search_query)
                };
                
                // Apply sorting
                match sort_by {
                    "Title (A-Z)" => {
                        results.sort_by(|a, b| a.title.cmp(&b.title));
                    }
                    "Year (Newest)" => {
                        results.sort_by(|a, b| b.year.cmp(&a.year));
                    }
                    "Year (Oldest)" => {
                        results.sort_by(|a, b| a.year.cmp(&b.year));
                    }
                    "Rating (High-Low)" => {
                        results.sort_by(|a, b| b.rating.partial_cmp(&a.rating).unwrap_or(std::cmp::Ordering::Equal));
                    }
                    "Rating (Low-High)" => {
                        results.sort_by(|a, b| a.rating.partial_cmp(&b.rating).unwrap_or(std::cmp::Ordering::Equal));
                    }
                    "Date Added (Newest)" => {
                        results.sort_by(|a, b| b.id.cmp(&a.id));
                    }
                    "Date Added (Oldest)" => {
                        results.sort_by(|a, b| a.id.cmp(&b.id));
                    }
                    _ => {}
                }
                
                // Cache the results
                db.borrow().cache_results(cache_key, results.clone());
                results
            }
        };

        // Populate the active view in batches to prevent UI freezing
        if is_grid_view {
            let batch_size = 50; // Add 50 items at a time
            let total = results.len();
            
            for (idx, chunk) in results.chunks(batch_size).enumerate() {
                for movie in chunk {
                    let item = create_movie_grid_item(movie, poster_cache);
                    grid_flow.append(&item);
                }
                
                // Force UI update between batches for large collections
                if idx < total / batch_size {
                    while gtk::glib::MainContext::default().pending() {
                        gtk::glib::MainContext::default().iteration(false);
                    }
                }
            }
        } else {
            let batch_size = 50; // Add 50 items at a time
            let total = results.len();
            
            for (idx, chunk) in results.chunks(batch_size).enumerate() {
                for movie in chunk {
                    let row = create_movie_row_with_context(movie, poster_cache, db);
                    list_box.append(&row);
                }
                
                // Force UI update between batches for large collections
                if idx < total / batch_size {
                    while gtk::glib::MainContext::default().pending() {
                        gtk::glib::MainContext::default().iteration(false);
                    }
                }
            }
        }
    }

    // Search functionality - only trigger on Enter key
    let list_box_clone = list_box.clone();
    let grid_flow_clone = grid_flow.clone();
    let db_clone = db.clone();
    let genre_dropdown_clone = genre_dropdown.clone();
    let sort_dropdown_clone = sort_dropdown.clone();
    let poster_cache_clone = poster_cache.clone();
    let is_grid_view_clone = is_grid_view.clone();
    search_entry.connect_activate(move |entry| {
        let query = entry.text();
        let selected_idx = genre_dropdown_clone.selected();
        let genres = ["All", "Action", "Comedy", "Drama", "Film Noir", "Horror", "Sci-Fi", "Thriller", "Romance"];
        let selected_genre = genres.get(selected_idx as usize).unwrap_or(&"All");
        
        let sort_idx = sort_dropdown_clone.selected();
        let sorts = ["Title (A-Z)", "Year (Newest)", "Year (Oldest)", "Rating (High-Low)", "Rating (Low-High)", "Date Added (Newest)", "Date Added (Oldest)"];
        let sort_by = sorts.get(sort_idx as usize).unwrap_or(&"Title (A-Z)");
        
        let is_grid = *is_grid_view_clone.borrow();
        refresh_movie_list(&list_box_clone, &grid_flow_clone, is_grid, &db_clone, &query.to_string(), selected_genre, sort_by, &poster_cache_clone);
    });

    // Genre filter
    let list_box_clone = list_box.clone();
    let grid_flow_clone = grid_flow.clone();
    let db_clone = db.clone();
    let search_entry_clone = search_entry.clone();
    let sort_dropdown_clone = sort_dropdown.clone();
    let poster_cache_clone = poster_cache.clone();
    let is_grid_view_clone = is_grid_view.clone();
    genre_dropdown.connect_selected_notify(move |dropdown| {
        let selected_idx = dropdown.selected();
        let genres = ["All", "Action", "Comedy", "Drama", "Film Noir", "Horror", "Sci-Fi", "Thriller", "Romance"];
        let selected_genre = genres.get(selected_idx as usize).unwrap_or(&"All");
        
        let query = search_entry_clone.text().to_string();
        let sort_idx = sort_dropdown_clone.selected();
        let sorts = ["Title (A-Z)", "Year (Newest)", "Year (Oldest)", "Rating (High-Low)", "Rating (Low-High)", "Date Added (Newest)", "Date Added (Oldest)"];
        let sort_by = sorts.get(sort_idx as usize).unwrap_or(&"Title (A-Z)");
        
        let is_grid = *is_grid_view_clone.borrow();
        refresh_movie_list(&list_box_clone, &grid_flow_clone, is_grid, &db_clone, &query, selected_genre, sort_by, &poster_cache_clone);
    });
    
    // Sort dropdown
    let list_box_clone = list_box.clone();
    let grid_flow_clone = grid_flow.clone();
    let db_clone = db.clone();
    let search_entry_clone = search_entry.clone();
    let genre_dropdown_clone = genre_dropdown.clone();
    let poster_cache_clone = poster_cache.clone();
    let is_grid_view_clone = is_grid_view.clone();
    sort_dropdown.connect_selected_notify(move |dropdown| {
        let sort_idx = dropdown.selected();
        let sorts = ["Title (A-Z)", "Year (Newest)", "Year (Oldest)", "Rating (High-Low)", "Rating (Low-High)", "Date Added (Newest)", "Date Added (Oldest)"];
        let sort_by = sorts.get(sort_idx as usize).unwrap_or(&"Title (A-Z)");
        
        let query = search_entry_clone.text().to_string();
        let selected_idx = genre_dropdown_clone.selected();
        let genres = ["All", "Action", "Comedy", "Drama", "Film Noir", "Horror", "Sci-Fi", "Thriller", "Romance"];
        let selected_genre = genres.get(selected_idx as usize).unwrap_or(&"All");
        
        let is_grid = *is_grid_view_clone.borrow();
        refresh_movie_list(&list_box_clone, &grid_flow_clone, is_grid, &db_clone, &query, selected_genre, sort_by, &poster_cache_clone);
    });

    // Movie selection
    let details_label_clone = details_label.clone();
    let poster_display_clone = poster_display.clone();
    let db_clone = db.clone();
    let selected_movie_id = Rc::new(RefCell::new(0u32));
    let selected_movie_id_clone = selected_movie_id.clone();
    
    list_box.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            // Get the movie ID from the row's widget name
            let movie_id_str = row.widget_name();
            if let Ok(movie_id) = movie_id_str.as_str().parse::<u32>() {
                *selected_movie_id_clone.borrow_mut() = movie_id;
                
                // Get the actual movie from the database by ID
                let db = db_clone.borrow();
                if let Some(movie) = db.movies.get(&movie_id) {
                    // Update poster - load full resolution then scale for display
                    if !movie.poster_path.is_empty() && Path::new(&movie.poster_path).exists() {
                        if let Ok(pixbuf) = Pixbuf::from_file(&movie.poster_path) {
                            if let Some(scaled) = pixbuf.scale_simple(200, 300, gtk::gdk_pixbuf::InterpType::Bilinear) {
                                poster_display_clone.set_pixbuf(Some(&scaled));
                            }
                        }
                    } else {
                        poster_display_clone.set_pixbuf(None);
                    }
                    
                    // Escape all text that goes into markup
                    let escaped_title = escape_markup(&movie.title);
                    let escaped_director = escape_markup(&movie.director);
                    let escaped_genre = escape_markup(&movie.genre.join(", "));
                    let escaped_description = escape_markup(&movie.description);
                    let escaped_file = escape_markup(&movie.file_path);
                    
                    // Format cast members with better visual presentation
                    let cast_display = if !movie.cast.is_empty() {
                        let cast_list: Vec<String> = movie.cast.iter()
                            .map(|name| escape_markup(name))
                            .collect();
                        cast_list.join("\n    • ")
                    } else {
                        String::from("Unknown")
                    };
                    
                    // Format IMDb ID display (with clickable link if available)
                    let imdb_display = if !movie.imdb_id.is_empty() {
                        format!("{} (https://www.imdb.com/title/{})", movie.imdb_id, movie.imdb_id)
                    } else {
                        String::from("Not available")
                    };
                    
                    let details = format!(
                        "<b>{}</b> ({})\n\n\
                        <b>Director:</b> {}\n\
                        <b>Genre:</b> {}\n\
                        <b>Rating:</b> ⭐ {:.1}/10\n\
                        <b>Runtime:</b> {} minutes\n\n\
                        <b>Starring:</b>\n    • {}\n\n\
                        <b>Description:</b>\n{}\n\n\
                        <b>File:</b> {}\n\
                        <b>TMDB ID:</b> {}\n\
                        <b>IMDb ID:</b> {}",
                        escaped_title, movie.year, escaped_director,
                        escaped_genre, movie.rating, movie.runtime,
                        cast_display, escaped_description, escaped_file,
                        movie.tmdb_id, imdb_display
                    );
                    details_label_clone.set_markup(&details);
                }
            }
        }
    });

    // Grid view selection (same logic as list)
    let details_label_clone = details_label.clone();
    let poster_display_clone = poster_display.clone();
    let db_clone = db.clone();
    let selected_movie_id_clone = selected_movie_id.clone();
    
    grid_flow.connect_child_activated(move |_, child| {
        // Get the movie ID from the child's widget name
        let movie_id_str = child.widget_name();
        if let Ok(movie_id) = movie_id_str.as_str().parse::<u32>() {
            *selected_movie_id_clone.borrow_mut() = movie_id;
            
            // Get the actual movie from the database by ID
            let db = db_clone.borrow();
            if let Some(movie) = db.movies.get(&movie_id) {
                // Update poster - load full resolution then scale for display
                if !movie.poster_path.is_empty() && Path::new(&movie.poster_path).exists() {
                    if let Ok(pixbuf) = Pixbuf::from_file(&movie.poster_path) {
                        if let Some(scaled) = pixbuf.scale_simple(200, 300, gtk::gdk_pixbuf::InterpType::Bilinear) {
                            poster_display_clone.set_pixbuf(Some(&scaled));
                        }
                    }
                } else {
                    poster_display_clone.set_pixbuf(None);
                }
                
                // Escape all text that goes into markup
                let escaped_title = escape_markup(&movie.title);
                let escaped_director = escape_markup(&movie.director);
                let escaped_genre = escape_markup(&movie.genre.join(", "));
                let escaped_description = escape_markup(&movie.description);
                let escaped_file = escape_markup(&movie.file_path);
                
                // Format cast members with better visual presentation
                let cast_display = if !movie.cast.is_empty() {
                    let cast_list: Vec<String> = movie.cast.iter()
                        .map(|name| escape_markup(name))
                        .collect();
                    cast_list.join("\n    • ")
                } else {
                    String::from("Unknown")
                };
                
                // Format IMDb ID display (with clickable link if available)
                let imdb_display = if !movie.imdb_id.is_empty() {
                    format!("{} (https://www.imdb.com/title/{})", movie.imdb_id, movie.imdb_id)
                } else {
                    String::from("Not available")
                };
                
                let details = format!(
                    "<b>{}</b> ({})\n\n\
                    <b>Director:</b> {}\n\
                    <b>Genre:</b> {}\n\
                    <b>Rating:</b> ⭐ {:.1}/10\n\
                    <b>Runtime:</b> {} minutes\n\n\
                    <b>Starring:</b>\n    • {}\n\n\
                    <b>Description:</b>\n{}\n\n\
                    <b>File:</b> {}\n\
                    <b>TMDB ID:</b> {}\n\
                    <b>IMDb ID:</b> {}",
                    escaped_title, movie.year, escaped_director,
                    escaped_genre, movie.rating, movie.runtime,
                    cast_display, escaped_description, escaped_file,
                    movie.tmdb_id, imdb_display
                );
                details_label_clone.set_markup(&details);
            }
        }
    });

    // Play button - launch VLC
    let db_clone = db.clone();
    let selected_movie_id_clone = selected_movie_id.clone();
    let status_bar_clone = status_bar.clone();
    play_button.connect_clicked(move |_| {
        let movie_id = *selected_movie_id_clone.borrow();
        if movie_id > 0 {
            let db = db_clone.borrow();
            if let Some(movie) = db.movies.get(&movie_id) {
                let file_path = movie.file_path.clone();
                let movie_title = movie.title.clone();
                drop(db);  // Release borrow early
                
                if !file_path.is_empty() && Path::new(&file_path).exists() {
                    // Try to launch VLC with suppressed output
                    match Command::new("vlc")
                        .arg(&file_path)
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                    {
                        Ok(_) => {
                            // Auto-log to watch history
                            let mut db_mut = db_clone.borrow_mut();
                            if let Some(movie) = db_mut.movies.get_mut(&movie_id) {
                                let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                                let entry = WatchLogEntry {
                                    date: today,
                                    rating: None,
                                    comments: String::from("Watched"),
                                };
                                movie.watch_log.push(entry);
                            }
                            db_mut.invalidate_result_cache();
                            db_mut.save_to_file();
                            drop(db_mut);
                            status_bar_clone.set_text(&format!("Playing: {} (logged to watch history)", movie_title));
                        }
                        Err(_) => {
                            // Try flatpak version
                            match Command::new("flatpak")
                                .args(["run", "org.videolan.VLC", &file_path])
                                .stdout(Stdio::null())
                                .stderr(Stdio::null())
                                .spawn()
                            {
                                Ok(_) => {
                                    // Auto-log to watch history
                                    let mut db_mut = db_clone.borrow_mut();
                                    if let Some(movie) = db_mut.movies.get_mut(&movie_id) {
                                        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                                        let entry = WatchLogEntry {
                                            date: today,
                                            rating: None,
                                            comments: String::from("Watched"),
                                        };
                                        movie.watch_log.push(entry);
                                    }
                                    db_mut.invalidate_result_cache();
                                    db_mut.save_to_file();
                                    drop(db_mut);
                                    status_bar_clone.set_text(&format!("Playing: {} (logged to watch history)", movie_title));
                                }
                                Err(_) => {
                                    status_bar_clone.set_text("VLC not found. Please install VLC.");
                                }
                            }
                        }
                    }
                } else {
                    status_bar_clone.set_text("No video file associated with this movie");
                }
            }
        }
    });

    // Associate File button
    let db_clone = db.clone();
    let window_clone = window.clone();
    let selected_movie_id_clone = selected_movie_id.clone();
    let details_label_clone = details_label.clone();
    let list_box_clone = list_box.clone();
    let poster_cache_clone = poster_cache.clone();
    associate_file_button.connect_clicked(move |_| {
        let movie_id = *selected_movie_id_clone.borrow();
        if movie_id == 0 {
            return;
        }
        
        let file_dialog = gtk::FileDialog::builder()
            .title("Select Movie File")
            .modal(true)
            .build();
        
        let db_clone2 = db_clone.clone();
        let details_label_clone2 = details_label_clone.clone();
        let list_box_clone2 = list_box_clone.clone();
        let poster_cache_clone2 = poster_cache_clone.clone();
        file_dialog.open(Some(&window_clone), gtk::gio::Cancellable::NONE, move |result| {
            if let Ok(file) = result {
                if let Some(path) = file.path() {
                    let file_path = path.to_string_lossy().to_string();
                    
                    // Update movie with new file path
                    let mut db = db_clone2.borrow_mut();
                    if let Some(movie) = db.movies.get_mut(&movie_id) {
                        movie.file_path = file_path.clone();
                        drop(db); // Release borrow
                        db_clone2.borrow_mut().save_to_file();
                        
                        // Refresh details display
                        let db = db_clone2.borrow();
                        if let Some(updated_movie) = db.movies.get(&movie_id) {
                            let escaped_title = escape_markup(&updated_movie.title);
                            let escaped_director = escape_markup(&updated_movie.director);
                            let escaped_genre = escape_markup(&updated_movie.genre.join(", "));
                            let escaped_description = escape_markup(&updated_movie.description);
                            let escaped_file = escape_markup(&updated_movie.file_path);
                            
                            let cast_display = if !updated_movie.cast_details.is_empty() {
                                let cast_list: Vec<String> = updated_movie.cast_details.iter()
                                    .map(|cm| {
                                        let name = escape_markup(&cm.name);
                                        let character = escape_markup(&cm.character);
                                        format!("{} ({})", name, character)
                                    })
                                    .collect();
                                cast_list.join("\n    • ")
                            } else if !updated_movie.cast.is_empty() {
                                let cast_list: Vec<String> = updated_movie.cast.iter()
                                    .map(|name| escape_markup(name))
                                    .collect();
                                cast_list.join("\n    • ")
                            } else {
                                String::from("Unknown")
                            };
                            
                            let imdb_display = if !updated_movie.imdb_id.is_empty() {
                                format!("{} (https://www.imdb.com/title/{})", updated_movie.imdb_id, updated_movie.imdb_id)
                            } else {
                                String::from("Not available")
                            };
                            
                            let details = format!(
                                "<b>{}</b> ({})\n\n\
                                <b>Director:</b> {}\n\
                                <b>Genre:</b> {}\n\
                                <b>Rating:</b> ⭐ {:.1}/10\n\
                                <b>Runtime:</b> {} minutes\n\n\
                                <b>Starring:</b>\n    • {}\n\n\
                                <b>Description:</b>\n{}\n\n\
                                <b>File:</b> {}\n\
                                <b>TMDB ID:</b> {}\n\
                                <b>IMDb ID:</b> {}",
                                escaped_title, updated_movie.year, escaped_director,
                                escaped_genre, updated_movie.rating, updated_movie.runtime,
                                cast_display, escaped_description, escaped_file,
                                updated_movie.tmdb_id, imdb_display
                            );
                            details_label_clone2.set_markup(&details);
                        }
                        
                        // Refresh movie list
                        while let Some(child) = list_box_clone2.first_child() {
                            list_box_clone2.remove(&child);
                        }
                        let movies = db_clone2.borrow().list_all();
                        for movie in &movies {
                            let row = create_movie_row(movie, &poster_cache_clone2);
                            list_box_clone2.append(&row);
                        }
                    }
                }
            }
        });
    });
    
    // Delete button
    let db_clone = db.clone();
    let list_box_clone = list_box.clone();
    let window_clone = window.clone();
    let selected_movie_id_clone = selected_movie_id.clone();
    let poster_cache_clone = poster_cache.clone();
    delete_button.connect_clicked(move |_| {
        let movie_id = *selected_movie_id_clone.borrow();
        if movie_id > 0 {
            let dialog = gtk::AlertDialog::builder()
                .message("Delete Movie")
                .detail("Are you sure you want to delete this movie?")
                .buttons(vec!["Cancel", "Delete"])
                .cancel_button(0)
                .default_button(0)
                .build();

            let db_clone2 = db_clone.clone();
            let list_box_clone2 = list_box_clone.clone();
            let poster_cache_clone2 = poster_cache_clone.clone();
            dialog.choose(Some(&window_clone), None::<&gtk::gio::Cancellable>, move |response| {
                if let Ok(1) = response {
                    if db_clone2.borrow_mut().delete_movie(movie_id) {
                        while let Some(child) = list_box_clone2.first_child() {
                            list_box_clone2.remove(&child);
                        }
                        let movies = db_clone2.borrow().list_all();
                        for movie in &movies {
                            let row = create_movie_row(movie, &poster_cache_clone2);
                            list_box_clone2.append(&row);
                        }
                    }
                }
            });
        }
    });

    // Show Cast button - display cast photos
    let db_clone = db.clone();
    let selected_movie_id_clone = selected_movie_id.clone();
    let window_clone = window.clone();
    show_cast_button.connect_clicked(move |_| {
        let movie_id = *selected_movie_id_clone.borrow();
        if movie_id > 0 {
            let db = db_clone.borrow();
            if let Some(movie) = db.movies.get(&movie_id) {
                if movie.cast_details.is_empty() {
                    let dialog = gtk::AlertDialog::builder()
                        .message("No Cast Information")
                        .detail("Cast photos are not available for this movie yet.\n\nTo get cast information:\n1. Click the \"🔄 Refresh Metadata\" button\n2. Wait for the update to complete\n3. Click \"⭐ Show Cast\" again\n\nNote: Cast photos are only available for movies scanned after the latest update.")
                        .buttons(vec!["OK"])
                        .build();
                    dialog.show(Some(&window_clone));
                    return;
                }

                // Clone the cast details for background thread
                let cast_details = movie.cast_details.clone();
                let cast_details_for_ui = cast_details.clone(); // Clone for UI thread
                let movie_title = movie.title.clone();

                // Create cast dialog
                let cast_dialog = Window::builder()
                    .title(&format!("Cast of {}", movie_title))
                    .modal(true)
                    .transient_for(&window_clone)
                    .default_width(600)
                    .default_height(500)
                    .build();

                let scroll = ScrolledWindow::new();
                scroll.set_vexpand(true);
                
                let cast_box = Box::new(Orientation::Vertical, 12);
                cast_box.set_margin_start(20);
                cast_box.set_margin_end(20);
                cast_box.set_margin_top(20);
                cast_box.set_margin_bottom(20);

                // Show dialog immediately with loading message
                let loading_label = Label::new(Some("Loading cast photos..."));
                cast_box.append(&loading_label);
                scroll.set_child(Some(&cast_box));
                cast_dialog.set_child(Some(&scroll));
                cast_dialog.present();

                // Download photos in background thread
                let (sender, receiver) = async_channel::unbounded::<(String, String, String, Vec<u8>)>();
                
                std::thread::spawn(move || {
                    for cast_member in &cast_details {
                        if !cast_member.profile_path.is_empty() {
                            if let Ok(response) = reqwest::blocking::get(&cast_member.profile_path) {
                                if let Ok(bytes) = response.bytes() {
                                    let _ = sender.send_blocking((
                                        cast_member.name.clone(),
                                        cast_member.character.clone(),
                                        cast_member.profile_path.clone(),
                                        bytes.to_vec()
                                    ));
                                    continue;
                                }
                            }
                        }
                        // Send with empty bytes if no photo
                        let _ = sender.send_blocking((
                            cast_member.name.clone(),
                            cast_member.character.clone(),
                            String::new(),
                            vec![]
                        ));
                    }
                });

                // Update UI as photos arrive
                let cast_box_clone = cast_box.clone();
                glib::spawn_future_local(async move {
                    // Remove loading message
                    while let Some(child) = cast_box_clone.first_child() {
                        cast_box_clone.remove(&child);
                    }

                    let mut count = 0;
                    let total = cast_details_for_ui.len();
                    
                    while count < total {
                        if let Ok((name, character, _profile_path, photo_bytes)) = receiver.recv().await {
                            let member_box = Box::new(Orientation::Horizontal, 12);
                            member_box.set_margin_bottom(12);

                            // Actor photo
                            let photo_box = Box::new(Orientation::Vertical, 0);
                            photo_box.set_size_request(120, 180);
                            
                            if !photo_bytes.is_empty() {
                                let loader = gtk::gdk_pixbuf::PixbufLoader::new();
                                let _ = loader.write(&photo_bytes);
                                let _ = loader.close();
                                if let Some(pixbuf) = loader.pixbuf() {
                                    if let Some(scaled_pixbuf) = pixbuf.scale_simple(120, 180, gtk::gdk_pixbuf::InterpType::Bilinear) {
                                        let picture = Picture::for_pixbuf(&scaled_pixbuf);
                                        photo_box.append(&picture);
                                    }
                                }
                            } else {
                                // Placeholder
                                let placeholder = Label::new(Some("👤"));
                                placeholder.set_markup("<span size='xx-large'>👤</span>");
                                photo_box.append(&placeholder);
                            }

                            member_box.append(&photo_box);

                            // Actor info
                            let info_box = Box::new(Orientation::Vertical, 4);
                            info_box.set_valign(Align::Center);
                            
                            let name_label = Label::new(Some(&name));
                            name_label.set_xalign(0.0);
                            name_label.set_markup(&format!("<b>{}</b>", escape_markup(&name)));
                            
                            let character_label = Label::new(Some(&character));
                            character_label.set_xalign(0.0);
                            character_label.set_markup(&format!("<i>as {}</i>", escape_markup(&character)));
                            
                            info_box.append(&name_label);
                            if !character.is_empty() {
                                info_box.append(&character_label);
                            }

                            member_box.append(&info_box);
                            cast_box_clone.append(&member_box);
                            cast_box_clone.append(&Separator::new(Orientation::Horizontal));
                            
                            count += 1;
                        }
                    }
                });
            }
        }
    });

    // Watch Log button
    let window_clone = window.clone();
    let db_clone = db.clone();
    let selected_movie_id_clone = selected_movie_id.clone();
    watch_log_button.connect_clicked(move |_| {
        let movie_id = *selected_movie_id_clone.borrow();
        if movie_id == 0 {
            return;
        }
        
        let db = db_clone.borrow();
        if let Some(movie) = db.movies.get(&movie_id) {
            let movie_title = movie.title.clone();
            let movie_year = movie.year;
            let watch_log = movie.watch_log.clone();
            drop(db);
            
            // Create watch log dialog
            let log_dialog = Window::builder()
                .title(&format!("Watch Log: {} ({})", movie_title, movie_year))
                .modal(true)
                .transient_for(&window_clone)
                .default_width(600)
                .default_height(500)
                .build();
            
            let dialog_box = Box::new(Orientation::Vertical, 12);
            dialog_box.set_margin_start(20);
            dialog_box.set_margin_end(20);
            dialog_box.set_margin_top(20);
            dialog_box.set_margin_bottom(20);
            
            let title_label = Label::new(Some(&format!("Watch History for {} ({})", movie_title, movie_year)));
            title_label.set_markup(&format!("<b>Watch History for {} ({})</b>", escape_markup(&movie_title), movie_year));
            dialog_box.append(&title_label);
            
            // Scrolled window for watch log entries
            let scroll = ScrolledWindow::new();
            scroll.set_vexpand(true);
            scroll.set_min_content_height(250);
            
            let entries_box = Box::new(Orientation::Vertical, 12);
            scroll.set_child(Some(&entries_box));
            
            // Display existing watch log entries
            if watch_log.is_empty() {
                let no_entries = Label::new(Some("No watch history yet. Add your first entry below!"));
                no_entries.set_opacity(0.6);
                entries_box.append(&no_entries);
            } else {
                for entry in &watch_log {
                    let entry_frame = Frame::new(None);
                    let entry_box = Box::new(Orientation::Vertical, 6);
                    entry_box.set_margin_start(12);
                    entry_box.set_margin_end(12);
                    entry_box.set_margin_top(8);
                    entry_box.set_margin_bottom(8);
                    
                    let date_label = Label::new(Some(&format!("📅 {}", entry.date)));
                    date_label.set_xalign(0.0);
                    date_label.set_markup(&format!("<b>📅 {}</b>", entry.date));
                    entry_box.append(&date_label);
                    
                    if let Some(rating) = entry.rating {
                        let rating_label = Label::new(Some(&format!("⭐ Personal Rating: {:.1}/10", rating)));
                        rating_label.set_xalign(0.0);
                        entry_box.append(&rating_label);
                    }
                    
                    if !entry.comments.is_empty() {
                        let comments_label = Label::new(Some(&entry.comments));
                        comments_label.set_xalign(0.0);
                        comments_label.set_wrap(true);
                        comments_label.set_margin_top(6);
                        entry_box.append(&comments_label);
                    }
                    
                    entry_frame.set_child(Some(&entry_box));
                    entries_box.append(&entry_frame);
                }
            }
            
            dialog_box.append(&scroll);
            
            // Add new entry section
            let add_section = Frame::new(Some("Add New Watch Entry"));
            let add_box = Box::new(Orientation::Vertical, 12);
            add_box.set_margin_start(12);
            add_box.set_margin_end(12);
            add_box.set_margin_top(12);
            add_box.set_margin_bottom(12);
            
            // Date entry (auto-filled with today)
            let date_box = Box::new(Orientation::Horizontal, 8);
            let date_label = Label::new(Some("Date:"));
            let date_entry = Entry::new();
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            date_entry.set_text(&today);
            date_entry.set_placeholder_text(Some("YYYY-MM-DD"));
            date_box.append(&date_label);
            date_box.append(&date_entry);
            add_box.append(&date_box);
            
            // Rating entry (optional)
            let rating_box = Box::new(Orientation::Horizontal, 8);
            let rating_label = Label::new(Some("Your Rating (0-10):"));
            let rating_entry = Entry::new();
            rating_entry.set_placeholder_text(Some("Optional - e.g., 8.5"));
            rating_box.append(&rating_label);
            rating_box.append(&rating_entry);
            add_box.append(&rating_box);
            
            // Comments (multi-line)
            let comments_label = Label::new(Some("Comments:"));
            comments_label.set_xalign(0.0);
            add_box.append(&comments_label);
            
            let comments_scroll = ScrolledWindow::new();
            comments_scroll.set_min_content_height(100);
            let comments_text = gtk::TextView::new();
            comments_text.set_wrap_mode(gtk::WrapMode::Word);
            comments_scroll.set_child(Some(&comments_text));
            add_box.append(&comments_scroll);
            
            // Add button
            let button_box = Box::new(Orientation::Horizontal, 8);
            let add_button = Button::with_label("Add Entry");
            let close_button = Button::with_label("Close");
            button_box.append(&add_button);
            button_box.append(&close_button);
            add_box.append(&button_box);
            
            add_section.set_child(Some(&add_box));
            dialog_box.append(&add_section);
            
            log_dialog.set_child(Some(&dialog_box));
            
            // Add button handler
            let db_clone2 = db_clone.clone();
            let log_dialog_clone = log_dialog.clone();
            let entries_box_clone = entries_box.clone();
            add_button.connect_clicked(move |_| {
                let date = date_entry.text().to_string();
                let rating_text = rating_entry.text().to_string();
                let rating = if rating_text.is_empty() {
                    None
                } else {
                    rating_text.parse::<f32>().ok()
                };
                
                let buffer = comments_text.buffer();
                let comments = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
                
                // Create new entry
                let entry = WatchLogEntry {
                    date,
                    rating,
                    comments,
                };
                
                // Add to movie
                let mut db = db_clone2.borrow_mut();
                if let Some(movie) = db.movies.get_mut(&movie_id) {
                    movie.watch_log.push(entry.clone());
                    db.invalidate_result_cache();
                    db.save_to_file();
                }
                drop(db);
                
                // Add to display
                let entry_frame = Frame::new(None);
                let entry_box = Box::new(Orientation::Vertical, 6);
                entry_box.set_margin_start(12);
                entry_box.set_margin_end(12);
                entry_box.set_margin_top(8);
                entry_box.set_margin_bottom(8);
                
                let date_label = Label::new(Some(&format!("📅 {}", entry.date)));
                date_label.set_xalign(0.0);
                date_label.set_markup(&format!("<b>📅 {}</b>", entry.date));
                entry_box.append(&date_label);
                
                if let Some(rating) = entry.rating {
                    let rating_label = Label::new(Some(&format!("⭐ Personal Rating: {:.1}/10", rating)));
                    rating_label.set_xalign(0.0);
                    entry_box.append(&rating_label);
                }
                
                if !entry.comments.is_empty() {
                    let comments_label = Label::new(Some(&entry.comments));
                    comments_label.set_xalign(0.0);
                    comments_label.set_wrap(true);
                    comments_label.set_margin_top(6);
                    entry_box.append(&comments_label);
                }
                
                entry_frame.set_child(Some(&entry_box));
                entries_box_clone.append(&entry_frame);
                
                // Clear inputs
                date_entry.set_text(&chrono::Local::now().format("%Y-%m-%d").to_string());
                rating_entry.set_text("");
                buffer.set_text("");
            });
            
            // Close button handler
            close_button.connect_clicked(move |_| {
                log_dialog_clone.close();
            });
            
            log_dialog.present();
        }
    });

    // Scan directory
    let window_clone = window.clone();
    let db_clone = db.clone();
    let list_box_clone = list_box.clone();
    let status_bar_clone = status_bar.clone();
    let poster_cache_clone = poster_cache.clone();
    scan_button.connect_clicked(move |_| {
        let dialog = gtk::FileDialog::new();
        dialog.set_title("Select Movie Directory");

        let db_clone2 = db_clone.clone();
        let list_box_clone2 = list_box_clone.clone();
        let status_bar_clone2 = status_bar_clone.clone();
        let poster_cache_clone2 = poster_cache_clone.clone();
        dialog.select_folder(Some(&window_clone), None::<&gtk::gio::Cancellable>, move |result| {
            if let Ok(folder) = result {
                if let Some(path) = folder.path() {
                    let path_str = path.to_string_lossy().to_string();
                    
                    let db_clone3 = db_clone2.clone();
                    let list_box_clone3 = list_box_clone2.clone();
                    let status_bar_clone3 = status_bar_clone2.clone();
                    
                    // Create async channel
                    let (sender, receiver) = async_channel::unbounded::<(String, String, Option<Movie>)>();
                    
                    // Get API key, posters_dir, year_cutoff, and existing paths before spawning thread (Rc can't be sent)
                    let api_key = db_clone3.borrow().tmdb_api_key.clone();
                    let posters_dir = db_clone3.borrow().posters_dir.clone();
                    let year_cutoff = load_config().map(|c| c.year_cutoff).unwrap_or(1966);
                    let existing_paths: std::collections::HashSet<String> = db_clone3.borrow()
                        .movies
                        .values()
                        .map(|m| m.file_path.clone())
                        .collect();
                    
                    // Spawn background thread with async runtime
                    std::thread::spawn(move || {
                        let runtime = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .unwrap();
                        
                        runtime.block_on(async {
                            // Collect all video files recursively
                            let mut files_to_process = Vec::new();
                            let video_extensions = vec!["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v"];
                            
                            let _ = sender.send_blocking(("status".to_string(), format!("Scanning: {} (including subdirectories)...", path_str), None));
                            
                            let path = Path::new(&path_str);
                            scan_directory_recursive(path, &video_extensions, &mut files_to_process);
                            
                            // Filter out files that already exist in database (using pre-extracted paths)
                            
                            let new_files: Vec<_> = files_to_process.into_iter()
                                .filter(|(_, file_path)| !existing_paths.contains(file_path))
                                .collect();
                            
                            if new_files.is_empty() {
                                let _ = sender.send_blocking(("status".to_string(), "No new movies found - all files already in database".to_string(), None));
                                let _ = sender.send_blocking(("complete".to_string(), String::new(), None));
                                return;
                            }
                            
                            let _ = sender.send_blocking(("status".to_string(), format!("Found {} new video files (skipped {} existing), fetching metadata in parallel...", new_files.len(), existing_paths.len()), None));
                            
                            // Process files in parallel batches of 10
                            let client = reqwest::Client::new();
                            let batch_size = 10;
                            
                            for batch in new_files.chunks(batch_size) {
                                let futures: Vec<_> = batch.iter()
                                    .map(|(clean_title, file_path_str)| {
                                        let api_key = api_key.clone();
                                        let title = clean_title.clone();
                                        let file_path = file_path_str.clone();
                                        let client = client.clone();
                                        let sender = sender.clone();
                                        let posters_dir = posters_dir.clone();
                                        
                                        async move {
                                            let _ = sender.send_blocking(("status".to_string(), format!("Fetching: {}", title), None));
                                            
                                            match fetch_movie_metadata_async(&client, &api_key, &title, file_path.clone(), posters_dir, year_cutoff).await {
                                                Some(movie) => {
                                                    let _ = sender.send_blocking(("add".to_string(), format!("✓ Found: {}", title), Some(movie)));
                                                }
                                                None => {
                                                    let movie = Movie {
                                                        id: 0,
                                                        title: title.clone(),
                                                        year: 0,
                                                        director: String::from("Unknown"),
                                                        genre: vec![String::from("Uncategorized")],
                                                        rating: 0.0,
                                                        runtime: 0,
                                                        description: String::from("Metadata not found"),
                                                        cast: vec![],
                                                        cast_details: vec![],
                                                        file_path,
                                                        poster_url: String::new(),
                                                        tmdb_id: 0,
                                                        imdb_id: String::new(),
                                                        poster_path: String::new(),
                                                        watch_log: Vec::new(),
                                                    };
                                                    let _ = sender.send_blocking(("add".to_string(), format!("⚠ Added without metadata: {}", title), Some(movie)));
                                                }
                                            }
                                        }
                                    })
                                    .collect();
                                
                                futures::future::join_all(futures).await;
                            }
                            
                            let _ = sender.send_blocking(("complete".to_string(), String::new(), None));
                        });
                    });
                    
                    // Handle messages on main thread using spawn_future_local
                    glib::spawn_future_local(async move {
                        while let Ok((msg_type, status, movie_opt)) = receiver.recv().await {
                            match msg_type.as_str() {
                                "status" => {
                                    status_bar_clone3.set_text(&status);
                                }
                                "add" => {
                                    if let Some(movie) = movie_opt {
                                        // Check if movie already exists
                                        let exists = db_clone3.borrow().movies.values()
                                            .any(|m| m.file_path == movie.file_path);
                                        
                                        if !exists {
                                            db_clone3.borrow_mut().add_movie(movie);
                                        }
                                    }
                                    status_bar_clone3.set_text(&status);
                                }
                                "complete" => {
                                    while let Some(child) = list_box_clone3.first_child() {
                                        list_box_clone3.remove(&child);
                                    }
                                    let movies = db_clone3.borrow().list_all();
                                    for movie in &movies {
                                        let row = create_movie_row(movie, &poster_cache_clone2);
                                        list_box_clone3.append(&row);
                                    }
                                    status_bar_clone3.set_text("Scan complete!");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    });
                }
            }
        });
    });

    // Refresh metadata
    let db_clone = db.clone();
    let list_box_clone = list_box.clone();
    let selected_movie_id_clone = selected_movie_id.clone();
    let status_bar_clone = status_bar.clone();
    let poster_cache_clone = poster_cache.clone();
    let posters_dir_clone = db.borrow().posters_dir.clone();
    refresh_button.connect_clicked(move |_| {
        let movie_id = *selected_movie_id_clone.borrow();
        if movie_id > 0 {
            let db_clone2 = db_clone.clone();
            let list_box_clone2 = list_box_clone.clone();
            let status_bar_clone2 = status_bar_clone.clone();
            let posters_dir = posters_dir_clone.clone();
            let poster_cache_clone2 = poster_cache_clone.clone();
            
            // Get the data we need before spawning thread
            let (title, file_path, api_key) = {
                let db = db_clone2.borrow();
                if let Some(movie) = db.movies.get(&movie_id) {
                    (movie.title.clone(), movie.file_path.clone(), db.tmdb_api_key.clone())
                } else {
                    return;
                }
            };
            
            let (sender, receiver) = async_channel::unbounded::<Option<(u32, Movie)>>();
            
            // Update status immediately
            status_bar_clone2.set_text(&format!("Refreshing: {}", title));
            
            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::new();
                let search_url = format!(
                    "https://api.themoviedb.org/3/search/movie?api_key={}&query={}",
                    api_key,
                    urlencoding::encode(&title)
                );
                
                if let Ok(response) = client.get(&search_url).send() {
                    if let Ok(search_response) = response.json::<TMDBSearchResponse>() {
                        if !search_response.results.is_empty() {
                            let tmdb_movie_id = search_response.results[0].id;
                            let details_url = format!(
                                "https://api.themoviedb.org/3/movie/{}?api_key={}&append_to_response=credits",
                                tmdb_movie_id, api_key
                            );
                            
                            if let Ok(details_response) = client.get(&details_url).send() {
                                if let Ok(details) = details_response.json::<TMDBMovieDetails>() {
                                    let year: u16 = details.release_date
                                        .split('-')
                                        .next()
                                        .and_then(|y| y.parse().ok())
                                        .unwrap_or(0);
                                    
                                    let director = details.credits.crew
                                        .iter()
                                        .find(|c| c.job == "Director")
                                        .map(|c| c.name.clone())
                                        .unwrap_or_else(|| "Unknown".to_string());
                                    
                                    let cast: Vec<String> = details.credits.cast
                                        .iter()
                                        .take(5)
                                        .map(|c| c.name.clone())
                                        .collect();
                                    
                                    let cast_details: Vec<CastMember> = details.credits.cast
                                        .iter()
                                        .take(5)
                                        .map(|c| CastMember {
                                            name: c.name.clone(),
                                            character: c.character.clone(),
                                            profile_path: c.profile_path.as_ref()
                                                .map(|p| format!("https://image.tmdb.org/t/p/w185{}", p))
                                                .unwrap_or_default(),
                                        })
                                        .collect();
                                    
                                    let genres: Vec<String> = details.genres
                                        .iter()
                                        .map(|g| g.name.clone())
                                        .collect();
                                    
                                    let poster_url = details.poster_path
                                        .map(|p| format!("https://image.tmdb.org/t/p/original{}", p))
                                        .unwrap_or_default();
                                    
                                    let poster_path = if !poster_url.is_empty() {
                                        download_poster(&poster_url, tmdb_movie_id, &posters_dir).unwrap_or_default()
                                    } else {
                                        String::new()
                                    };
                                    
                                    // Fetch IMDb ID
                                    let external_ids_url = format!(
                                        "https://api.themoviedb.org/3/movie/{}/external_ids?api_key={}",
                                        tmdb_movie_id, api_key
                                    );
                                    
                                    let imdb_id = if let Ok(response) = reqwest::blocking::get(&external_ids_url) {
                                        if let Ok(external_ids) = response.json::<TMDBExternalIds>() {
                                            external_ids.imdb_id.unwrap_or_default()
                                        } else {
                                            String::new()
                                        }
                                    } else {
                                        String::new()
                                    };
                                    
                                    let movie = Movie {
                                        id: 0,
                                        title: details.title,
                                        year,
                                        director,
                                        genre: if genres.is_empty() { vec!["Unknown".to_string()] } else { genres },
                                        rating: details.vote_average,
                                        runtime: details.runtime.unwrap_or(0),
                                        description: details.overview,
                                        cast,
                                        cast_details,
                                        file_path: file_path.clone(),
                                        poster_url,
                                        tmdb_id: tmdb_movie_id,
                                        imdb_id,
                                        poster_path,
        watch_log: Vec::new(),
                                    };
                                    
                                    let _ = sender.send_blocking(Some((movie_id, movie)));
                                    return;
                                }
                            }
                        }
                    }
                }
                
                let _ = sender.send_blocking(None);
            });
            
            glib::spawn_future_local(async move {
                if let Ok(movie_opt) = receiver.recv().await {
                    if let Some((old_id, new_movie)) = movie_opt {
                        db_clone2.borrow_mut().delete_movie(old_id);
                        db_clone2.borrow_mut().add_movie(new_movie);
                        
                        while let Some(child) = list_box_clone2.first_child() {
                            list_box_clone2.remove(&child);
                        }
                        let movies = db_clone2.borrow().list_all();
                        for movie in &movies {
                            let row = create_movie_row(movie, &poster_cache_clone2);
                            list_box_clone2.append(&row);
                        }
                        status_bar_clone2.set_text("Metadata refreshed!");
                    } else {
                        status_bar_clone2.set_text("Failed to refresh metadata");
                    }
                }
            });
        }
    });

    // Refresh All button - refresh metadata for ALL movies
    let window_clone = window.clone();
    let db_clone = db.clone();
    let list_box_clone = list_box.clone();
    let grid_flow_clone = grid_flow.clone();
    let status_bar_clone = status_bar.clone();
    let poster_cache_clone = poster_cache.clone();
    let posters_dir_clone = db.borrow().posters_dir.clone();
    let is_grid_view_clone = is_grid_view.clone();
    refresh_all_button.connect_clicked(move |_| {
        // Confirm with user
        let dialog = gtk::AlertDialog::builder()
            .message("Refresh All Movies")
            .detail("This will refresh metadata and download HD posters for ALL movies in your database.\n\nThis may take a while depending on your collection size. Continue?")
            .buttons(vec!["Cancel", "Refresh All"])
            .cancel_button(0)
            .default_button(1)
            .build();
        
        let db_clone2 = db_clone.clone();
        let list_box_clone2 = list_box_clone.clone();
        let grid_flow_clone2 = grid_flow_clone.clone();
        let status_bar_clone2 = status_bar_clone.clone();
        let poster_cache_clone2 = poster_cache_clone.clone();
        let is_grid_view_clone2 = is_grid_view_clone.clone();
        let posters_dir = posters_dir_clone.clone();
        
        dialog.choose(Some(&window_clone), None::<&gtk::gio::Cancellable>, move |response| {
            if let Ok(1) = response {
                status_bar_clone2.set_text("Starting refresh of all movies...");
                
                // Get all movies
                let movies: Vec<(u32, String, String)> = db_clone2.borrow()
                    .movies
                    .values()
                    .map(|m| (m.id, m.title.clone(), m.file_path.clone()))
                    .collect();
                
                let total_count = movies.len();
                let api_key = db_clone2.borrow().tmdb_api_key.clone();
                let year_cutoff = load_config().map(|c| c.year_cutoff).unwrap_or(1966);
                
                let (sender, receiver) = async_channel::unbounded::<(String, Option<(u32, Movie)>)>();
                
                // Spawn background thread to refresh all movies
                std::thread::spawn(move || {
                    let client = reqwest::blocking::Client::new();
                    let mut success_count = 0;
                    let mut fail_count = 0;
                    
                    for (i, (movie_id, title, file_path)) in movies.iter().enumerate() {
                        let progress = format!("Refreshing {}/{}: {}", i + 1, total_count, title);
                        let _ = sender.send_blocking((progress, None));
                        
                        // Search TMDB
                        let search_url = format!(
                            "https://api.themoviedb.org/3/search/movie?api_key={}&query={}",
                            api_key,
                            urlencoding::encode(title)
                        );
                        
                        if let Ok(response) = client.get(&search_url).send() {
                            if let Ok(search_response) = response.json::<TMDBSearchResponse>() {
                                if !search_response.results.is_empty() {
                                    // Prioritize movies before year_cutoff (same logic as fetch_movie_metadata_async)
                                    let tmdb_movie_id = {
                                        let before_cutoff = search_response.results.iter().find(|movie| {
                                            if let Some(ref date) = movie.release_date {
                                                if let Some(year_str) = date.split('-').next() {
                                                    if let Ok(year) = year_str.parse::<i32>() {
                                                        return year <= year_cutoff;
                                                    }
                                                }
                                            }
                                            false
                                        });
                                        
                                        if let Some(movie) = before_cutoff {
                                            movie.id
                                        } else {
                                            search_response.results[0].id
                                        }
                                    };
                                    
                                    let details_url = format!(
                                        "https://api.themoviedb.org/3/movie/{}?api_key={}&append_to_response=credits",
                                        tmdb_movie_id, api_key
                                    );
                                    
                                    if let Ok(details_response) = client.get(&details_url).send() {
                                        if let Ok(details) = details_response.json::<TMDBMovieDetails>() {
                                            // Build movie (same as refresh single)
                                            let year: u16 = details.release_date
                                                .split('-')
                                                .next()
                                                .and_then(|y| y.parse().ok())
                                                .unwrap_or(0);
                                            
                                            let director = details.credits.crew
                                                .iter()
                                                .find(|c| c.job == "Director")
                                                .map(|c| c.name.clone())
                                                .unwrap_or_else(|| "Unknown".to_string());
                                            
                                            let cast: Vec<String> = details.credits.cast
                                                .iter()
                                                .take(5)
                                                .map(|c| c.name.clone())
                                                .collect();
                                            
                                            let cast_details: Vec<CastMember> = details.credits.cast
                                                .iter()
                                                .take(5)
                                                .map(|c| CastMember {
                                                    name: c.name.clone(),
                                                    character: c.character.clone(),
                                                    profile_path: c.profile_path.as_ref()
                                                        .map(|p| format!("https://image.tmdb.org/t/p/w185{}", p))
                                                        .unwrap_or_default(),
                                                })
                                                .collect();
                                            
                                            let genres: Vec<String> = details.genres
                                                .iter()
                                                .map(|g| g.name.clone())
                                                .collect();
                                            
                                            let poster_url = details.poster_path
                                                .map(|p| format!("https://image.tmdb.org/t/p/original{}", p))
                                                .unwrap_or_default();
                                            
                                            let poster_path = if !poster_url.is_empty() {
                                                download_poster(&poster_url, tmdb_movie_id, &posters_dir).unwrap_or_default()
                                            } else {
                                                String::new()
                                            };
                                            
                                            // Fetch IMDb ID
                                            let external_ids_url = format!(
                                                "https://api.themoviedb.org/3/movie/{}/external_ids?api_key={}",
                                                tmdb_movie_id, api_key
                                            );
                                            
                                            let imdb_id = if let Ok(response) = reqwest::blocking::get(&external_ids_url) {
                                                if let Ok(external_ids) = response.json::<TMDBExternalIds>() {
                                                    external_ids.imdb_id.unwrap_or_default()
                                                } else {
                                                    String::new()
                                                }
                                            } else {
                                                String::new()
                                            };
                                            
                                            let movie = Movie {
                                                id: 0,
                                                title: details.title,
                                                year,
                                                director,
                                                genre: if genres.is_empty() { vec!["Unknown".to_string()] } else { genres },
                                                rating: details.vote_average,
                                                runtime: details.runtime.unwrap_or(0),
                                                description: details.overview,
                                                cast,
                                                cast_details,
                                                file_path: file_path.clone(),
                                                poster_url,
                                                tmdb_id: tmdb_movie_id,
                                                imdb_id,
                                                poster_path,
        watch_log: Vec::new(),
                                            };
                                            
                                            let _ = sender.send_blocking((String::new(), Some((*movie_id, movie))));
                                            success_count += 1;
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                        fail_count += 1;
                    }
                    
                    let summary = format!("Refresh complete! {} succeeded, {} failed", success_count, fail_count);
                    let _ = sender.send_blocking((summary, None));
                });
                
                // Handle updates on main thread
                glib::spawn_future_local(async move {
                    let mut updated_ids = Vec::new();
                    
                    while let Ok((status, movie_opt)) = receiver.recv().await {
                        if !status.is_empty() {
                            status_bar_clone2.set_text(&status);
                        }
                        
                        if let Some((old_id, new_movie)) = movie_opt {
                            db_clone2.borrow_mut().delete_movie(old_id);
                            db_clone2.borrow_mut().add_movie(new_movie);
                            updated_ids.push(old_id);
                        } else if status.contains("complete") {
                            // Refresh UI
                            let is_grid = *is_grid_view_clone2.borrow();
                            
                            if is_grid {
                                while let Some(child) = grid_flow_clone2.first_child() {
                                    grid_flow_clone2.remove(&child);
                                }
                                let movies = db_clone2.borrow().list_all();
                                for movie in &movies {
                                    let item = create_movie_grid_item(&movie, &poster_cache_clone2);
                                    grid_flow_clone2.append(&item);
                                }
                            } else {
                                while let Some(child) = list_box_clone2.first_child() {
                                    list_box_clone2.remove(&child);
                                }
                                let movies = db_clone2.borrow().list_all();
                                for movie in &movies {
                                    let row = create_movie_row(&movie, &poster_cache_clone2);
                                    list_box_clone2.append(&row);
                                }
                            }
                            
                            break;
                        }
                    }
                });
            }
        });
    });

    // Edit Metadata button
    let window_clone = window.clone();
    let db_clone = db.clone();
    let selected_movie_id_clone = selected_movie_id.clone();
    let details_label_clone = details_label.clone();
    let list_box_clone = list_box.clone();
    let status_bar_clone = status_bar.clone();
    let poster_cache_clone = poster_cache.clone();
    edit_button.connect_clicked(move |_| {
        let movie_id = *selected_movie_id_clone.borrow();
        if movie_id == 0 {
            status_bar_clone.set_text("Please select a movie first");
            return;
        }
        
        let movie = db_clone.borrow().movies.get(&movie_id).cloned();
        if let Some(movie) = movie {
            // Create edit dialog
            let dialog = Window::builder()
                .title(&format!("Edit Metadata: {}", movie.title))
                .modal(true)
                .transient_for(&window_clone)
                .default_width(600)
                .default_height(500)
                .build();
            
            let content = Box::new(Orientation::Vertical, 12);
            content.set_margin_start(20);
            content.set_margin_end(20);
            content.set_margin_top(20);
            content.set_margin_bottom(20);
            
            let scroll = ScrolledWindow::new();
            scroll.set_vexpand(true);
            
            let grid = Grid::new();
            grid.set_row_spacing(12);
            grid.set_column_spacing(12);
            
            // Title
            grid.attach(&Label::new(Some("Title:")), 0, 0, 1, 1);
            let title_entry = Entry::new();
            title_entry.set_text(&movie.title);
            title_entry.set_hexpand(true);
            grid.attach(&title_entry, 1, 0, 1, 1);
            
            // Year
            grid.attach(&Label::new(Some("Year:")), 0, 1, 1, 1);
            let year_entry = Entry::new();
            year_entry.set_text(&movie.year.to_string());
            grid.attach(&year_entry, 1, 1, 1, 1);
            
            // Director
            grid.attach(&Label::new(Some("Director:")), 0, 2, 1, 1);
            let director_entry = Entry::new();
            director_entry.set_text(&movie.director);
            director_entry.set_hexpand(true);
            grid.attach(&director_entry, 1, 2, 1, 1);
            
            // Genre
            grid.attach(&Label::new(Some("Genre:")), 0, 3, 1, 1);
            let genre_entry = Entry::new();
            genre_entry.set_text(&movie.genre.join(", "));
            genre_entry.set_hexpand(true);
            grid.attach(&genre_entry, 1, 3, 1, 1);
            
            // Rating
            grid.attach(&Label::new(Some("Rating (0-10):")), 0, 4, 1, 1);
            let rating_entry = Entry::new();
            rating_entry.set_text(&format!("{:.1}", movie.rating));
            grid.attach(&rating_entry, 1, 4, 1, 1);
            
            // Runtime
            grid.attach(&Label::new(Some("Runtime (min):")), 0, 5, 1, 1);
            let runtime_entry = Entry::new();
            runtime_entry.set_text(&movie.runtime.to_string());
            grid.attach(&runtime_entry, 1, 5, 1, 1);
            
            // Description
            grid.attach(&Label::new(Some("Description:")), 0, 6, 1, 1);
            let desc_text_view = gtk::TextView::new();
            desc_text_view.buffer().set_text(&movie.description);
            desc_text_view.set_wrap_mode(gtk::WrapMode::Word);
            desc_text_view.set_height_request(100);
            let desc_scroll = ScrolledWindow::new();
            desc_scroll.set_child(Some(&desc_text_view));
            desc_scroll.set_vexpand(true);
            grid.attach(&desc_scroll, 1, 6, 1, 1);
            
            // Cast
            grid.attach(&Label::new(Some("Cast (comma-separated):")), 0, 7, 1, 1);
            let cast_entry = Entry::new();
            cast_entry.set_text(&movie.cast.join(", "));
            cast_entry.set_hexpand(true);
            grid.attach(&cast_entry, 1, 7, 1, 1);
            
            scroll.set_child(Some(&grid));
            content.append(&scroll);
            
            let button_box = Box::new(Orientation::Horizontal, 8);
            button_box.set_halign(Align::End);
            let cancel_button = Button::with_label("Cancel");
            let save_button = Button::with_label("Save Changes");
            button_box.append(&cancel_button);
            button_box.append(&save_button);
            content.append(&button_box);
            
            dialog.set_child(Some(&content));
            
            let dialog_clone = dialog.clone();
            cancel_button.connect_clicked(move |_| {
                dialog_clone.close();
            });
            
            let dialog_clone = dialog.clone();
            let db_clone2 = db_clone.clone();
            let details_label_clone2 = details_label_clone.clone();
            let list_box_clone2 = list_box_clone.clone();
            let status_bar_clone2 = status_bar_clone.clone();
            let poster_cache_clone2 = poster_cache_clone.clone();
            save_button.connect_clicked(move |_| {
                // Parse and validate inputs
                let new_title = title_entry.text().to_string();
                let new_year: u16 = year_entry.text().parse().unwrap_or(movie.year);
                let new_director = director_entry.text().to_string();
                let new_genre: Vec<String> = genre_entry.text()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                let new_rating: f32 = rating_entry.text().parse().unwrap_or(movie.rating).clamp(0.0, 10.0);
                let new_runtime: u16 = runtime_entry.text().parse().unwrap_or(movie.runtime);
                
                let buffer = desc_text_view.buffer();
                let new_description = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
                
                let new_cast: Vec<String> = cast_entry.text()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                
                // Update movie
                let mut db = db_clone2.borrow_mut();
                if let Some(existing_movie) = db.movies.get_mut(&movie_id) {
                    existing_movie.title = new_title;
                    existing_movie.year = new_year;
                    existing_movie.director = new_director;
                    existing_movie.genre = if new_genre.is_empty() { vec!["Unknown".to_string()] } else { new_genre };
                    existing_movie.rating = new_rating;
                    existing_movie.runtime = new_runtime;
                    existing_movie.description = new_description;
                    existing_movie.cast = new_cast;
                }
                drop(db);
                
                db_clone2.borrow_mut().save_to_file();
                
                // Refresh UI
                let db = db_clone2.borrow();
                if let Some(updated_movie) = db.movies.get(&movie_id) {
                    let escaped_title = escape_markup(&updated_movie.title);
                    let escaped_director = escape_markup(&updated_movie.director);
                    let escaped_genre = escape_markup(&updated_movie.genre.join(", "));
                    let escaped_description = escape_markup(&updated_movie.description);
                    let escaped_file = escape_markup(&updated_movie.file_path);
                    
                    let cast_display = if !updated_movie.cast_details.is_empty() {
                        let cast_list: Vec<String> = updated_movie.cast_details.iter()
                            .map(|cm| {
                                let name = escape_markup(&cm.name);
                                let character = escape_markup(&cm.character);
                                format!("{} ({})", name, character)
                            })
                            .collect();
                        cast_list.join("\n    • ")
                    } else if !updated_movie.cast.is_empty() {
                        let cast_list: Vec<String> = updated_movie.cast.iter()
                            .map(|name| escape_markup(name))
                            .collect();
                        cast_list.join("\n    • ")
                    } else {
                        String::from("Unknown")
                    };
                    
                    let imdb_display = if !updated_movie.imdb_id.is_empty() {
                        format!("{} (https://www.imdb.com/title/{})", updated_movie.imdb_id, updated_movie.imdb_id)
                    } else {
                        String::from("Not available")
                    };
                    
                    let details = format!(
                        "<b>{}</b> ({})\n\n\
                        <b>Director:</b> {}\n\
                        <b>Genre:</b> {}\n\
                        <b>Rating:</b> ⭐ {:.1}/10\n\
                        <b>Runtime:</b> {} minutes\n\n\
                        <b>Starring:</b>\n    • {}\n\n\
                        <b>Description:</b>\n{}\n\n\
                        <b>File:</b> {}\n\
                        <b>TMDB ID:</b> {}\n\
                        <b>IMDb ID:</b> {}",
                        escaped_title, updated_movie.year, escaped_director,
                        escaped_genre, updated_movie.rating, updated_movie.runtime,
                        cast_display, escaped_description, escaped_file,
                        updated_movie.tmdb_id, imdb_display
                    );
                    details_label_clone2.set_markup(&details);
                }
                drop(db);
                
                // Refresh movie list
                while let Some(child) = list_box_clone2.first_child() {
                    list_box_clone2.remove(&child);
                }
                let movies = db_clone2.borrow().list_all();
                for movie in &movies {
                    let row = create_movie_row(movie, &poster_cache_clone2);
                    list_box_clone2.append(&row);
                }
                
                status_bar_clone2.set_text("Movie metadata updated");
                dialog_clone.close();
            });
            
            dialog.present();
        }
    });
    
    // Select Different Version button - search TMDB and let user choose
    let window_clone = window.clone();
    let db_clone = db.clone();
    let list_box_clone = list_box.clone();
    let grid_flow_clone = grid_flow.clone();
    let status_bar_clone = status_bar.clone();
    let selected_movie_id_clone = selected_movie_id.clone();
    let poster_cache_clone_select = poster_cache.clone();
    let posters_dir_clone = db.borrow().posters_dir.clone();
    let sort_dropdown_clone = sort_dropdown.clone();
    let search_entry_clone = search_entry.clone();
    let genre_dropdown_clone = genre_dropdown.clone();
    let is_grid_view_clone = is_grid_view.clone();
    select_version_button.connect_clicked(move |_| {
        let movie_id = *selected_movie_id_clone.borrow();
        if movie_id == 0 {
            status_bar_clone.set_text("Please select a movie first");
            return;
        }
        
        let db = db_clone.borrow();
        if let Some(movie) = db.movies.get(&movie_id) {
            let movie_title = movie.title.clone();
            let movie_title_for_ui = movie_title.clone(); // Clone for UI updates
            let movie_year = movie.year; // Get the year for search
            let file_path = movie.file_path.clone();
            let api_key = db.tmdb_api_key.clone();
            drop(db); // Release borrow
            
            // Create selection dialog
            let selection_dialog = Window::builder()
                .title(&format!("Select Version: {}", movie_title))
                .modal(true)
                .transient_for(&window_clone)
                .default_width(700)
                .default_height(600)
                .build();
            
            let dialog_box = Box::new(Orientation::Vertical, 12);
            dialog_box.set_margin_start(20);
            dialog_box.set_margin_end(20);
            dialog_box.set_margin_top(20);
            dialog_box.set_margin_bottom(20);
            
            let instruction_text = if movie_year > 0 {
                format!("Select the correct version of \"{}\" (showing all years, {} matches highlighted first):", movie_title, movie_year)
            } else {
                format!("Select the correct version of \"{}\" (showing all years):", movie_title)
            };
            let instruction = Label::new(Some(&instruction_text));
            instruction.set_xalign(0.0);
            dialog_box.append(&instruction);
            
            let instruction_clone = instruction.clone();
            
            let scroll = ScrolledWindow::new();
            scroll.set_vexpand(true);
            scroll.set_hexpand(true);
            
            let list_box_results = ListBox::new();
            list_box_results.set_selection_mode(gtk::SelectionMode::Single);
            scroll.set_child(Some(&list_box_results));
            dialog_box.append(&scroll);
            
            let button_box = Box::new(Orientation::Horizontal, 8);
            button_box.set_halign(Align::End);
            let cancel_button = Button::with_label("Cancel");
            let select_button = Button::with_label("Use Selected");
            button_box.append(&cancel_button);
            button_box.append(&select_button);
            dialog_box.append(&button_box);
            
            selection_dialog.set_child(Some(&dialog_box));
            
            let selection_dialog_clone = selection_dialog.clone();
            let selection_dialog_clone3 = selection_dialog.clone();
            cancel_button.connect_clicked(move |_| {
                selection_dialog_clone.close();
            });
            
            // Show loading message
            let loading_row = gtk::ListBoxRow::new();
            let loading_label = Label::new(Some("Searching TMDB..."));
            loading_row.set_child(Some(&loading_label));
            list_box_results.append(&loading_row);
            
            selection_dialog.present();
            
            // Fetch TMDB search results in background
            let list_box_results_clone = list_box_results.clone();
            let db_clone2 = db_clone.clone();
            let list_box_clone2 = list_box_clone.clone();
            let status_bar_clone2 = status_bar_clone.clone();
            let selection_dialog_clone2 = selection_dialog.clone();
            
            let (sender, receiver) = async_channel::unbounded::<Vec<(u32, String, String, f32)>>();
            
            // Check cache first - use title only for cache key to get more results
            let cache_key = movie_title.clone();
            let cached_results = db_clone2.borrow().get_cached_search(&cache_key);
            
            if let Some(results) = cached_results {
                // Use cached results immediately!
                let _ = sender.send_blocking(results);
            } else {
                // Fetch from TMDB and cache
                std::thread::spawn(move || {
                    let mut all_results = Vec::new();
                    
                    // Fetch up to 5 pages (100 results total) for comprehensive results
                    // DON'T filter by year - we want ALL matches so user can find the right one
                    for page in 1..=5 {
                        let search_url = format!(
                            "https://api.themoviedb.org/3/search/movie?api_key={}&query={}&page={}",
                            api_key,
                            urlencoding::encode(&movie_title),
                            page
                        );
                        
                        if let Ok(response) = reqwest::blocking::get(&search_url) {
                            if let Ok(search_result) = response.json::<TMDBSearchResponse>() {
                                if search_result.results.is_empty() {
                                    // No more results, stop fetching
                                    break;
                                }
                                
                                let page_results: Vec<(u32, String, String, f32)> = search_result.results.iter()
                                    .map(|r| {
                                        // Fetch basic details for each to get year
                                        let details_url = format!(
                                            "https://api.themoviedb.org/3/movie/{}?api_key={}",
                                            r.id, api_key
                                        );
                                        
                                        if let Ok(details_response) = reqwest::blocking::get(&details_url) {
                                            if let Ok(details) = details_response.json::<TMDBMovieDetails>() {
                                                let year = details.release_date
                                                    .split('-')
                                                    .next()
                                                    .and_then(|y| y.parse().ok())
                                                    .unwrap_or(0);
                                                return (r.id, details.title, year.to_string(), details.vote_average);
                                            }
                                        }
                                        (r.id, "Unknown".to_string(), "????".to_string(), 0.0)
                                    })
                                    .collect();
                                
                                all_results.extend(page_results);
                                
                                // If we got less than 20 results, we've reached the last page
                                if search_result.results.len() < 20 {
                                    break;
                                }
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    
                    // Sort results: exact year matches first, then by popularity (rating)
                    if !all_results.is_empty() {
                        all_results.sort_by(|a, b| {
                            let a_year: u16 = a.2.parse().unwrap_or(0);
                            let b_year: u16 = b.2.parse().unwrap_or(0);
                            
                            // If we have a year to match, prioritize exact matches
                            if movie_year > 0 {
                                let a_matches = a_year == movie_year;
                                let b_matches = b_year == movie_year;
                                
                                if a_matches && !b_matches {
                                    return std::cmp::Ordering::Less;
                                } else if !a_matches && b_matches {
                                    return std::cmp::Ordering::Greater;
                                }
                            }
                            
                            // Then sort by rating (higher is better)
                            b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal)
                        });
                        
                        let _ = sender.send_blocking(all_results);
                    }
                });
            }
            
            // Update UI with results
            let db_clone_for_cache = db_clone2.clone();
            let cache_key_for_save = cache_key.clone();
            let poster_cache_clone_select2 = poster_cache_clone_select.clone();
            let posters_dir = posters_dir_clone.clone();
            let sort_dropdown_clone2 = sort_dropdown_clone.clone();
            let search_entry_clone2 = search_entry_clone.clone();
            let genre_dropdown_clone2 = genre_dropdown_clone.clone();
            let grid_flow_clone2 = grid_flow_clone.clone();
            let is_grid_view_clone2 = is_grid_view_clone.clone();
            let selection_dialog_clone4 = selection_dialog_clone3.clone();
            glib::spawn_future_local(async move {
                if let Ok(results) = receiver.recv().await {
                    // Cache the results if not from cache (safe borrow)
                    if !results.is_empty() {
                        if let Ok(mut db) = db_clone_for_cache.try_borrow_mut() {
                            db.cache_search_results(
                                cache_key_for_save.clone(),
                                results.clone()
                            );
                        }
                    }
                    
                    // Remove loading message
                    while let Some(child) = list_box_results_clone.first_child() {
                        list_box_results_clone.remove(&child);
                    }
                    
                    if results.is_empty() {
                        let no_results_row = gtk::ListBoxRow::new();
                        let no_results_label = Label::new(Some("No results found"));
                        no_results_row.set_child(Some(&no_results_label));
                        list_box_results_clone.append(&no_results_row);
                        return;
                    }
                    
                    // Update instruction with result count
                    instruction_clone.set_text(&format!(
                        "Select the correct version of \"{}\" ({} results found):",
                        movie_title_for_ui, results.len()
                    ));
                    
                    // Add result rows
                    for (tmdb_id, title, year, rating) in &results {
                        let row = gtk::ListBoxRow::new();
                        row.set_widget_name(&tmdb_id.to_string());
                        
                        let row_box = Box::new(Orientation::Vertical, 4);
                        row_box.set_margin_start(12);
                        row_box.set_margin_end(12);
                        row_box.set_margin_top(8);
                        row_box.set_margin_bottom(8);
                        
                        let title_label = Label::new(Some(&format!("{} ({})", title, year)));
                        title_label.set_xalign(0.0);
                        title_label.set_markup(&format!("<b>{}</b> ({})", title, year));
                        
                        let rating_label = Label::new(Some(&format!("Rating: ⭐ {:.1}/10", rating)));
                        rating_label.set_xalign(0.0);
                        
                        row_box.append(&title_label);
                        row_box.append(&rating_label);
                        row.set_child(Some(&row_box));
                        list_box_results_clone.append(&row);
                    }
                    
                    // Select first result by default
                    if let Some(first_row) = list_box_results_clone.row_at_index(0) {
                        list_box_results_clone.select_row(Some(&first_row));
                    }
                    
                    // Handle selection
                    select_button.connect_clicked(move |_| {
                        if let Some(selected_row) = list_box_results_clone.selected_row() {
                            let tmdb_id_str = selected_row.widget_name();
                            if let Ok(tmdb_id) = tmdb_id_str.as_str().parse::<u32>() {
                                status_bar_clone2.set_text(&format!("Fetching metadata for TMDB ID {}...", tmdb_id));
                                selection_dialog_clone2.close();
                                
                                // Fetch full metadata for selected movie
                                let db_clone3 = db_clone2.clone();
                                let list_box_clone3 = list_box_clone2.clone();
                                let status_bar_clone3 = status_bar_clone2.clone();
                                let file_path_clone = file_path.clone();
                                
                                // Extract API key and posters_dir before spawning thread (Rc can't be sent)
                                let api_key = db_clone3.borrow().tmdb_api_key.clone();
                                let posters_dir = posters_dir.clone();
                                
                                let (sender2, receiver2) = async_channel::unbounded::<Option<(u32, Movie)>>();
                                
                                std::thread::spawn(move || {
                                    let details_url = format!(
                                        "https://api.themoviedb.org/3/movie/{}?api_key={}&append_to_response=credits",
                                        tmdb_id, api_key
                                    );
                                    
                                    if let Ok(response) = reqwest::blocking::get(&details_url) {
                                        if let Ok(details) = response.json::<TMDBMovieDetails>() {
                                            // Build Movie struct (same as fetch_movie_metadata_async)
                                            let year: u16 = details.release_date
                                                .split('-')
                                                .next()
                                                .and_then(|y| y.parse().ok())
                                                .unwrap_or(0);
                                            
                                            let director = details.credits.crew
                                                .iter()
                                                .find(|c| c.job == "Director")
                                                .map(|c| c.name.clone())
                                                .unwrap_or_else(|| "Unknown".to_string());
                                            
                                            let cast: Vec<String> = details.credits.cast
                                                .iter()
                                                .take(5)
                                                .map(|c| c.name.clone())
                                                .collect();
                                            
                                            let cast_details: Vec<CastMember> = details.credits.cast
                                                .iter()
                                                .take(5)
                                                .map(|c| CastMember {
                                                    name: c.name.clone(),
                                                    character: c.character.clone(),
                                                    profile_path: c.profile_path.as_ref()
                                                        .map(|p| format!("https://image.tmdb.org/t/p/w185{}", p))
                                                        .unwrap_or_default(),
                                                })
                                                .collect();
                                            
                                            let genres: Vec<String> = details.genres
                                                .iter()
                                                .map(|g| g.name.clone())
                                                .collect();
                                            
                                            let poster_url = details.poster_path
                                                .map(|p| format!("https://image.tmdb.org/t/p/original{}", p))
                                                .unwrap_or_default();
                                            
                                            let poster_path = if !poster_url.is_empty() {
                                                download_poster(&poster_url, tmdb_id, &posters_dir).unwrap_or_default()
                                            } else {
                                                String::new()
                                            };
                                            
                                            // Fetch IMDb ID
                                            let external_ids_url = format!(
                                                "https://api.themoviedb.org/3/movie/{}/external_ids?api_key={}",
                                                tmdb_id, api_key
                                            );
                                            
                                            let imdb_id = if let Ok(response) = reqwest::blocking::get(&external_ids_url) {
                                                if let Ok(external_ids) = response.json::<TMDBExternalIds>() {
                                                    external_ids.imdb_id.unwrap_or_default()
                                                } else {
                                                    String::new()
                                                }
                                            } else {
                                                String::new()
                                            };
                                            
                                            let new_movie = Movie {
                                                id: 0,
                                                title: details.title,
                                                year,
                                                director,
                                                genre: if genres.is_empty() { vec!["Unknown".to_string()] } else { genres },
                                                rating: details.vote_average,
                                                runtime: details.runtime.unwrap_or(0),
                                                description: details.overview,
                                                cast,
                                                cast_details,
                                                file_path: file_path_clone,
                                                poster_url,
                                                tmdb_id,
                                                imdb_id,
                                                poster_path,
        watch_log: Vec::new(),
                                            };
                                            
                                            let _ = sender2.send_blocking(Some((movie_id, new_movie)));
                                            return;
                                        }
                                    }
                                    let _ = sender2.send_blocking(None);
                                });
                                
                                let poster_cache_clone_select3 = poster_cache_clone_select2.clone();
                                let sort_dropdown = sort_dropdown_clone2.clone();
                                let search_entry = search_entry_clone2.clone();
                                let genre_dropdown = genre_dropdown_clone2.clone();
                                let grid_flow = grid_flow_clone2.clone();
                                let is_grid_view = is_grid_view_clone2.clone();
                                let selection_dialog = selection_dialog_clone4.clone();
                                glib::spawn_future_local(async move {
                                    if let Ok(Some((old_id, new_movie))) = receiver2.recv().await {
                                        // Do delete and add in single borrow scope with error handling
                                        match db_clone3.try_borrow_mut() {
                                            Ok(mut db_mut) => {
                                                db_mut.delete_movie(old_id);
                                                db_mut.add_movie(new_movie);
                                            }
                                            Err(e) => {
                                                eprintln!("Failed to update movie - database busy: {}", e);
                                                status_bar_clone3.set_text("Failed to update movie - try again");
                                                return;
                                            }
                                        }
                                        
                                        // Close the selection dialog
                                        selection_dialog.close();
                                        
                                        // Set sort to "Year (Newest)" - index 2
                                        sort_dropdown.set_selected(2);
                                        
                                        // Refresh list with proper sorting
                                        let search_query = search_entry.text().to_string();
                                        let genre_idx = genre_dropdown.selected();
                                        let genre_filter = if genre_idx == 0 {
                                            ""
                                        } else {
                                            match genre_idx {
                                                1 => "Action",
                                                2 => "Adventure",
                                                3 => "Animation",
                                                4 => "Comedy",
                                                5 => "Crime",
                                                6 => "Documentary",
                                                7 => "Drama",
                                                8 => "Family",
                                                9 => "Fantasy",
                                                10 => "History",
                                                11 => "Horror",
                                                12 => "Music",
                                                13 => "Mystery",
                                                14 => "Romance",
                                                15 => "Science Fiction",
                                                16 => "Thriller",
                                                17 => "War",
                                                18 => "Western",
                                                _ => "",
                                            }
                                        };
                                        
                                        refresh_movie_list(
                                            &list_box_clone3,
                                            &grid_flow,
                                            *is_grid_view.borrow(),
                                            &db_clone3,
                                            &search_query,
                                            genre_filter,
                                            "Year (Newest)",
                                            &poster_cache_clone_select3,
                                        );
                                        
                                        status_bar_clone3.set_text("Movie version updated successfully!");
                                    } else {
                                        status_bar_clone3.set_text("Failed to fetch metadata");
                                    }
                                });
                            }
                        }
                    });
                }
            });
        }
    });

    // Add movie dialog
    let window_clone = window.clone();
    let db_clone = db.clone();
    let list_box_clone = list_box.clone();
    let status_bar_clone = status_bar.clone();
    let poster_cache_clone_add = poster_cache.clone();
    let posters_dir_clone = db.borrow().posters_dir.clone();
    add_button.connect_clicked(move |_| {
        let dialog = Window::builder()
            .title("Add New Movie")
            .modal(true)
            .transient_for(&window_clone)
            .default_width(400)
            .default_height(150)
            .build();

        let content = Box::new(Orientation::Vertical, 12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);

        let grid = Grid::new();
        grid.set_row_spacing(8);
        grid.set_column_spacing(8);

        let title_entry = Entry::new();
        title_entry.set_placeholder_text(Some("Movie title to search"));
        title_entry.set_hexpand(true);

        grid.attach(&Label::new(Some("Title:")), 0, 0, 1, 1);
        grid.attach(&title_entry, 1, 0, 1, 1);
        
        // Optional file path
        let file_label = Label::new(Some("File (optional):"));
        let file_entry = Entry::new();
        file_entry.set_placeholder_text(Some("No file selected"));
        file_entry.set_editable(false);
        file_entry.set_hexpand(true);
        
        let browse_btn = Button::with_label("Browse...");
        let file_box = Box::new(Orientation::Horizontal, 4);
        file_box.append(&file_entry);
        file_box.append(&browse_btn);
        
        grid.attach(&file_label, 0, 1, 1, 1);
        grid.attach(&file_box, 1, 1, 1, 1);

        content.append(&grid);
        
        // File picker dialog
        let file_entry_clone = file_entry.clone();
        let window_clone2 = window_clone.clone();
        browse_btn.connect_clicked(move |_| {
            let file_dialog = gtk::FileDialog::builder()
                .title("Select Movie File")
                .modal(true)
                .build();
            
            let file_entry_clone2 = file_entry_clone.clone();
            file_dialog.open(Some(&window_clone2), gtk::gio::Cancellable::NONE, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        file_entry_clone2.set_text(&path.to_string_lossy());
                    }
                }
            });
        });

        let button_box = Box::new(Orientation::Horizontal, 8);
        button_box.set_halign(gtk::Align::End);
        let cancel_btn = Button::with_label("Cancel");
        let search_btn = Button::with_label("Search");
        button_box.append(&cancel_btn);
        button_box.append(&search_btn);
        content.append(&button_box);

        dialog.set_child(Some(&content));

        let dialog_clone = dialog.clone();
        cancel_btn.connect_clicked(move |_| {
            dialog_clone.close();
        });

        let dialog_clone = dialog.clone();
        let window_clone2 = window_clone.clone();
        let db_clone2 = db_clone.clone();
        let list_box_clone2 = list_box_clone.clone();
        let status_bar_clone2 = status_bar_clone.clone();
        let poster_cache_clone_add2 = poster_cache_clone_add.clone();
        let posters_dir = posters_dir_clone.clone();
        search_btn.connect_clicked(move |_| {
            let search_title = title_entry.text().to_string();
            let selected_file_path = file_entry.text().to_string();
            let file_path_to_use = if selected_file_path.is_empty() || selected_file_path == "No file selected" {
                String::new()
            } else {
                selected_file_path
            };
            
            if !search_title.is_empty() {
                dialog_clone.close();
                
                // Create selection dialog
                let selection_dialog = Window::builder()
                    .title(&format!("Select Movie: {}", search_title))
                    .modal(true)
                    .transient_for(&window_clone2)
                    .default_width(600)
                    .default_height(400)
                    .build();
                
                let dialog_box = Box::new(Orientation::Vertical, 12);
                dialog_box.set_margin_start(20);
                dialog_box.set_margin_end(20);
                dialog_box.set_margin_top(20);
                dialog_box.set_margin_bottom(20);
                
                let instruction = Label::new(Some(&format!("Select the movie to add for \"{}\":", search_title)));
                instruction.set_xalign(0.0);
                dialog_box.append(&instruction);
                
                let instruction_clone = instruction.clone();
                
                let scroll = ScrolledWindow::new();
                scroll.set_vexpand(true);
                scroll.set_hexpand(true);
                
                let list_box_results = ListBox::new();
                list_box_results.set_selection_mode(gtk::SelectionMode::Single);
                scroll.set_child(Some(&list_box_results));
                dialog_box.append(&scroll);
                
                let button_box = Box::new(Orientation::Horizontal, 8);
                button_box.set_halign(Align::End);
                let cancel_button = Button::with_label("Cancel");
                let add_selected_button = Button::with_label("Add Selected");
                button_box.append(&cancel_button);
                button_box.append(&add_selected_button);
                dialog_box.append(&button_box);
                
                selection_dialog.set_child(Some(&dialog_box));
                
                let selection_dialog_clone = selection_dialog.clone();
                cancel_button.connect_clicked(move |_| {
                    selection_dialog_clone.close();
                });
                
                // Show loading message
                let loading_row = gtk::ListBoxRow::new();
                let loading_label = Label::new(Some("Searching TMDB..."));
                loading_row.set_child(Some(&loading_label));
                list_box_results.append(&loading_row);
                
                selection_dialog.present();
                
                // Fetch TMDB search results in background
                let list_box_results_clone = list_box_results.clone();
                let db_clone3 = db_clone2.clone();
                let list_box_clone3 = list_box_clone2.clone();
                let status_bar_clone3 = status_bar_clone2.clone();
                let selection_dialog_clone2 = selection_dialog.clone();
                let search_title_for_ui = search_title.clone();
                let file_path_for_movie = file_path_to_use.clone();
                
                let api_key = db_clone3.borrow().tmdb_api_key.clone();
                
                let (sender, receiver) = async_channel::unbounded::<Vec<(u32, String, String, f32)>>();
                
                // Check cache first
                let search_title_for_cache = search_title.clone();
                let cached_results = db_clone3.borrow().get_cached_search(&search_title_for_cache);
                
                if let Some(results) = cached_results {
                    // Use cached results immediately!
                    let _ = sender.send_blocking(results);
                } else {
                    // Fetch from TMDB
                    std::thread::spawn(move || {
                        // Search TMDB
                        let search_url = format!(
                            "https://api.themoviedb.org/3/search/movie?api_key={}&query={}",
                            api_key,
                            urlencoding::encode(&search_title)
                        );
                        
                        if let Ok(response) = reqwest::blocking::get(&search_url) {
                            if let Ok(search_result) = response.json::<TMDBSearchResponse>() {
                                let results: Vec<(u32, String, String, f32)> = search_result.results.iter()
                                    // Show ALL results (up to 20)
                                    .map(|r| {
                                        let details_url = format!(
                                            "https://api.themoviedb.org/3/movie/{}?api_key={}",
                                            r.id, api_key
                                        );
                                        
                                        if let Ok(details_response) = reqwest::blocking::get(&details_url) {
                                            if let Ok(details) = details_response.json::<TMDBMovieDetails>() {
                                                let year = details.release_date
                                                    .split('-')
                                                    .next()
                                                    .and_then(|y| y.parse().ok())
                                                    .unwrap_or(0);
                                            return (r.id, details.title, year.to_string(), details.vote_average);
                                        }
                                    }
                                    (r.id, "Unknown".to_string(), "????".to_string(), 0.0)
                                })
                                .collect();
                            
                            let _ = sender.send_blocking(results);
                        }
                    }
                    });
                }
                
                // Update UI with results
                let db_clone_for_cache = db_clone3.clone();
                let search_title_for_cache2 = search_title_for_ui.clone();
                let poster_cache_clone_add3 = poster_cache_clone_add2.clone();
                let posters_dir = posters_dir.clone();
                glib::spawn_future_local(async move {
                    if let Ok(results) = receiver.recv().await {
                        // Cache the results if not from cache
                        if !results.is_empty() {
                            db_clone_for_cache.borrow_mut().cache_search_results(
                                search_title_for_cache2.clone(),
                                results.clone()
                            );
                        }
                        
                        // Remove loading message
                        while let Some(child) = list_box_results_clone.first_child() {
                            list_box_results_clone.remove(&child);
                        }
                        
                        if results.is_empty() {
                            let no_results_row = gtk::ListBoxRow::new();
                            let no_results_label = Label::new(Some("No results found"));
                            no_results_row.set_child(Some(&no_results_label));
                            list_box_results_clone.append(&no_results_row);
                            return;
                        }
                        
                        // Update instruction with result count
                        instruction_clone.set_text(&format!(
                            "Select the movie to add for \"{}\" ({} results found):",
                            search_title_for_ui, results.len()
                        ));
                        
                        // Add result rows
                        for (tmdb_id, title, year, rating) in &results {
                            let row = gtk::ListBoxRow::new();
                            row.set_widget_name(&tmdb_id.to_string());
                            
                            let row_box = Box::new(Orientation::Vertical, 4);
                            row_box.set_margin_start(12);
                            row_box.set_margin_end(12);
                            row_box.set_margin_top(8);
                            row_box.set_margin_bottom(8);
                            
                            let title_label = Label::new(Some(&format!("{} ({})", title, year)));
                            title_label.set_xalign(0.0);
                            title_label.set_markup(&format!("<b>{}</b> ({})", title, year));
                            
                            let rating_label = Label::new(Some(&format!("Rating: ⭐ {:.1}/10", rating)));
                            rating_label.set_xalign(0.0);
                            
                            row_box.append(&title_label);
                            row_box.append(&rating_label);
                            row.set_child(Some(&row_box));
                            list_box_results_clone.append(&row);
                        }
                        
                        // Select first result by default
                        if let Some(first_row) = list_box_results_clone.row_at_index(0) {
                            list_box_results_clone.select_row(Some(&first_row));
                        }
                        
                        // Handle add selected
                        let file_path_final = file_path_for_movie.clone();
                        add_selected_button.connect_clicked(move |_| {
                            if let Some(selected_row) = list_box_results_clone.selected_row() {
                                let tmdb_id_str = selected_row.widget_name();
                                if let Ok(tmdb_id) = tmdb_id_str.as_str().parse::<u32>() {
                                    status_bar_clone3.set_text(&format!("Adding movie (TMDB ID: {})...", tmdb_id));
                                    selection_dialog_clone2.close();
                                    
                                    // Fetch full metadata
                                    let db_clone4 = db_clone3.clone();
                                    let list_box_clone4 = list_box_clone3.clone();
                                    let status_bar_clone4 = status_bar_clone3.clone();
                                    
                                    let api_key = db_clone4.borrow().tmdb_api_key.clone();
                                    let posters_dir = posters_dir.clone();
                                    let (sender2, receiver2) = async_channel::unbounded::<Option<(String, Movie)>>();
                                    
                                    let file_path_clone = file_path_final.clone();
                                    std::thread::spawn(move || {
                                        let details_url = format!(
                                            "https://api.themoviedb.org/3/movie/{}?api_key={}&append_to_response=credits",
                                            tmdb_id, api_key
                                        );
                                        
                                        if let Ok(response) = reqwest::blocking::get(&details_url) {
                                            if let Ok(details) = response.json::<TMDBMovieDetails>() {
                                                let year: u16 = details.release_date
                                                    .split('-')
                                                    .next()
                                                    .and_then(|y| y.parse().ok())
                                                    .unwrap_or(0);
                                                
                                                let director = details.credits.crew
                                                    .iter()
                                                    .find(|c| c.job == "Director")
                                                    .map(|c| c.name.clone())
                                                    .unwrap_or_else(|| "Unknown".to_string());
                                                
                                                let cast: Vec<String> = details.credits.cast
                                                    .iter()
                                                    .take(5)
                                                    .map(|c| c.name.clone())
                                                    .collect();
                                                
                                                let cast_details: Vec<CastMember> = details.credits.cast
                                                    .iter()
                                                    .take(5)
                                                    .map(|c| CastMember {
                                                        name: c.name.clone(),
                                                        character: c.character.clone(),
                                                        profile_path: c.profile_path.as_ref()
                                                            .map(|p| format!("https://image.tmdb.org/t/p/w185{}", p))
                                                            .unwrap_or_default(),
                                                    })
                                                    .collect();
                                                
                                                let genres: Vec<String> = details.genres
                                                    .iter()
                                                    .map(|g| g.name.clone())
                                                    .collect();
                                                
                                                let poster_url = details.poster_path
                                                    .map(|p| format!("https://image.tmdb.org/t/p/original{}", p))
                                                    .unwrap_or_default();
                                                
                                                let poster_path = if !poster_url.is_empty() {
                                                    download_poster(&poster_url, tmdb_id, &posters_dir).unwrap_or_default()
                                                } else {
                                                    String::new()
                                                };
                                                
                                                // Fetch IMDb ID
                                                let external_ids_url = format!(
                                                    "https://api.themoviedb.org/3/movie/{}/external_ids?api_key={}",
                                                    tmdb_id, api_key
                                                );
                                                
                                                let imdb_id = if let Ok(response) = reqwest::blocking::get(&external_ids_url) {
                                                    if let Ok(external_ids) = response.json::<TMDBExternalIds>() {
                                                        external_ids.imdb_id.unwrap_or_default()
                                                    } else {
                                                        String::new()
                                                    }
                                                } else {
                                                    String::new()
                                                };
                                                
                                                let movie = Movie {
                                                    id: 0,
                                                    title: details.title.clone(),
                                                    year,
                                                    director,
                                                    genre: if genres.is_empty() { vec!["Unknown".to_string()] } else { genres },
                                                    rating: details.vote_average,
                                                    runtime: details.runtime.unwrap_or(0),
                                                    description: details.overview,
                                                    cast,
                                                    cast_details,
                                                    file_path: file_path_clone,
                                                    poster_url,
                                                    tmdb_id,
                                                    imdb_id,
                                                    poster_path,
        watch_log: Vec::new(),
                                                };
                                                
                                                let _ = sender2.send_blocking(Some((details.title, movie)));
                                                return;
                                            }
                                        }
                                        let _ = sender2.send_blocking(None);
                                    });
                                    
                                    let poster_cache_clone_add4 = poster_cache_clone_add3.clone();
                                    glib::spawn_future_local(async move {
                                        if let Ok(Some((title, movie))) = receiver2.recv().await {
                                            db_clone4.borrow_mut().add_movie(movie.clone());
                                            
                                            let row = create_movie_row(&movie, &poster_cache_clone_add4);
                                            list_box_clone4.append(&row);
                                            
                                            status_bar_clone4.set_text(&format!("Added: {}", title));
                                        } else {
                                            status_bar_clone4.set_text("Failed to fetch movie metadata");
                                        }
                                    });
                                }
                            }
                        });
                    }
                });
            }
        });

        dialog.present();
    });
    // Settings button - change API key and manage scan directories
    let window_clone = window.clone();
    let db_clone = db.clone();
    let status_bar_clone = status_bar.clone();
    settings_button.connect_clicked(move |_| {
        let dialog = Window::builder()
            .title("Settings")
            .modal(true)
            .transient_for(&window_clone)
            .default_width(600)
            .default_height(400)
            .build();

        let content = Box::new(Orientation::Vertical, 12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);

        // API Key section
        let api_label = Label::new(Some("TMDB API Key:"));
        api_label.set_xalign(0.0);
        api_label.set_markup("<b>TMDB API Key:</b>");

        let api_entry = Entry::new();
        api_entry.set_text(&db_clone.borrow().tmdb_api_key);
        api_entry.set_visibility(false);

        content.append(&api_label);
        content.append(&api_entry);
        content.append(&Separator::new(Orientation::Horizontal));

        // Scan directories section
        let scan_label = Label::new(Some("Scan Directories:"));
        scan_label.set_xalign(0.0);
        scan_label.set_markup("<b>Scan Directories:</b>");
        content.append(&scan_label);

        // Load current config
        let current_config = load_config().unwrap_or_default();
        
        // Year Cutoff section (after loading config)
        let year_label = Label::new(Some("Year Cutoff for Auto-Scan:"));
        year_label.set_xalign(0.0);
        year_label.set_markup("<b>Year Cutoff for Auto-Scan:</b>");
        
        let year_help = Label::new(Some("Auto-scan will prioritize movies released before or in this year"));
        year_help.set_xalign(0.0);
        year_help.set_opacity(0.7);
        year_help.set_wrap(true);
        
        let year_entry = Entry::new();
        year_entry.set_text(&current_config.year_cutoff.to_string());
        year_entry.set_max_length(4);
        year_entry.set_width_chars(6);
        
        content.append(&year_label);
        content.append(&year_help);
        content.append(&year_entry);
        content.append(&Separator::new(Orientation::Horizontal));
        
        // List of scan directories
        let dirs_box = Box::new(Orientation::Vertical, 4);
        let dirs_list = Rc::new(RefCell::new(current_config.scan_directories.clone()));
        
        let scrolled = ScrolledWindow::new();
        scrolled.set_vexpand(true);
        scrolled.set_min_content_height(150);
        
        let list_box = ListBox::new();
        
        // Populate existing directories
        for dir in &current_config.scan_directories {
            let row = gtk::ListBoxRow::new();
            let hbox = Box::new(Orientation::Horizontal, 8);
            hbox.set_margin_start(8);
            hbox.set_margin_end(8);
            hbox.set_margin_top(4);
            hbox.set_margin_bottom(4);
            
            let dir_label = Label::new(Some(dir));
            dir_label.set_xalign(0.0);
            dir_label.set_hexpand(true);
            
            let remove_btn = Button::with_label("Remove");
            
            hbox.append(&dir_label);
            hbox.append(&remove_btn);
            row.set_child(Some(&hbox));
            list_box.append(&row);
            
            // Remove button handler
            let dirs_list_clone = dirs_list.clone();
            let dir_to_remove = dir.clone();
            let list_box_clone = list_box.clone();
            remove_btn.connect_clicked(move |btn| {
                dirs_list_clone.borrow_mut().retain(|d| d != &dir_to_remove);
                // Remove the row from UI
                if let Some(row) = btn.parent().and_then(|p| p.parent()) {
                    list_box_clone.remove(&row);
                }
            });
        }
        
        scrolled.set_child(Some(&list_box));
        dirs_box.append(&scrolled);
        
        let add_dir_box = Box::new(Orientation::Horizontal, 8);
        let add_dir_btn = Button::with_label("➕ Add Directory");
        add_dir_box.append(&add_dir_btn);
        dirs_box.append(&add_dir_box);
        
        content.append(&dirs_box);
        
        // Add directory handler
        let window_clone2 = window_clone.clone();
        let dirs_list_clone = dirs_list.clone();
        let list_box_clone = list_box.clone();
        add_dir_btn.connect_clicked(move |_| {
            let file_dialog = gtk::FileDialog::new();
            file_dialog.set_title("Select Directory to Scan");
            
            let dirs_list_clone2 = dirs_list_clone.clone();
            let list_box_clone2 = list_box_clone.clone();
            file_dialog.select_folder(Some(&window_clone2), None::<&gtk::gio::Cancellable>, move |result| {
                if let Ok(folder) = result {
                    if let Some(path) = folder.path() {
                        let path_str = path.to_string_lossy().to_string();
                        
                        // Add to list
                        dirs_list_clone2.borrow_mut().push(path_str.clone());
                        
                        // Add to UI
                        let row = gtk::ListBoxRow::new();
                        let hbox = Box::new(Orientation::Horizontal, 8);
                        hbox.set_margin_start(8);
                        hbox.set_margin_end(8);
                        hbox.set_margin_top(4);
                        hbox.set_margin_bottom(4);
                        
                        let dir_label = Label::new(Some(&path_str));
                        dir_label.set_xalign(0.0);
                        dir_label.set_hexpand(true);
                        
                        let remove_btn = Button::with_label("Remove");
                        
                        hbox.append(&dir_label);
                        hbox.append(&remove_btn);
                        row.set_child(Some(&hbox));
                        list_box_clone2.append(&row);
                        
                        // Remove button handler
                        let dirs_list_clone3 = dirs_list_clone2.clone();
                        let path_str_clone = path_str.clone();
                        let list_box_clone3 = list_box_clone2.clone();
                        remove_btn.connect_clicked(move |btn| {
                            dirs_list_clone3.borrow_mut().retain(|d| d != &path_str_clone);
                            if let Some(row) = btn.parent().and_then(|p| p.parent()) {
                                list_box_clone3.remove(&row);
                            }
                        });
                    }
                }
            });
        });
        
        content.append(&Separator::new(Orientation::Horizontal));
        
        // Auto-scan checkbox
        let auto_scan_check = gtk::CheckButton::with_label("Automatically scan directories on startup");
        auto_scan_check.set_active(current_config.auto_scan_on_startup);
        content.append(&auto_scan_check);

        // Buttons
        let button_box = Box::new(Orientation::Horizontal, 8);
        button_box.set_halign(gtk::Align::End);
        let cancel_btn = Button::with_label("Cancel");
        let save_btn = Button::with_label("Save");
        button_box.append(&cancel_btn);
        button_box.append(&save_btn);
        content.append(&button_box);

        dialog.set_child(Some(&content));

        let dialog_clone = dialog.clone();
        cancel_btn.connect_clicked(move |_| {
            dialog_clone.close();
        });

        let dialog_clone = dialog.clone();
        let db_clone2 = db_clone.clone();
        let status_bar_clone2 = status_bar_clone.clone();
        save_btn.connect_clicked(move |_| {
            let new_key = api_entry.text().to_string();
            if !new_key.is_empty() {
                // Update database API key
                db_clone2.borrow_mut().tmdb_api_key = new_key.clone();
                
                // Parse year cutoff
                let year_cutoff = year_entry.text()
                    .to_string()
                    .parse::<i32>()
                    .unwrap_or(1966);
                
                // Save to config
                let config = Config {
                    tmdb_api_key: new_key,
                    scan_directories: dirs_list.borrow().clone(),
                    auto_scan_on_startup: auto_scan_check.is_active(),
                    year_cutoff,
                };
                if let Err(e) = save_config(&config) {
                    status_bar_clone2.set_text(&format!("Error saving config: {}", e));
                } else {
                    status_bar_clone2.set_text("Settings saved successfully");
                }
            }
            dialog_clone.close();
        });

        dialog.present();
    });

    
    // Statistics button
    let db_clone = db.clone();
    let window_clone = window.clone();
    stats_button.connect_clicked(move |_| {
        let db = db_clone.borrow();
        let movies = db.list_all();
        
        if movies.is_empty() {
            drop(db);
            let dialog = gtk::AlertDialog::builder()
                .message("No Statistics Available")
                .detail("Add some movies to your database first!")
                .buttons(vec!["OK"])
                .build();
            dialog.show(Some(&window_clone));
            return;
        }
        
        // Calculate statistics
        let total_movies = movies.len();
        let total_runtime: u32 = movies.iter().map(|m| m.runtime as u32).sum();
        let avg_runtime = if total_movies > 0 { total_runtime / total_movies as u32 } else { 0 };
        
        let avg_rating: f32 = if total_movies > 0 {
            movies.iter().map(|m| m.rating).sum::<f32>() / total_movies as f32
        } else {
            0.0
        };
        
        let oldest_year = movies.iter().filter(|m| m.year > 0).map(|m| m.year).min().unwrap_or(0);
        let newest_year = movies.iter().map(|m| m.year).max().unwrap_or(0);
        
        // Genre breakdown
        let mut genre_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for movie in &movies {
            for genre in &movie.genre {
                *genre_counts.entry(genre.clone()).or_insert(0) += 1;
            }
        }
        let mut genre_list: Vec<(String, usize)> = genre_counts.into_iter().collect();
        genre_list.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Decade breakdown
        let mut decade_counts: std::collections::HashMap<u16, usize> = std::collections::HashMap::new();
        for movie in &movies {
            if movie.year > 0 {
                let decade = (movie.year / 10) * 10;
                *decade_counts.entry(decade).or_insert(0) += 1;
            }
        }
        let mut decade_list: Vec<(u16, usize)> = decade_counts.into_iter().collect();
        decade_list.sort_by(|a, b| a.0.cmp(&b.0));
        
        // Top rated movies
        let mut top_rated = movies.clone();
        top_rated.sort_by(|a, b| b.rating.partial_cmp(&a.rating).unwrap_or(std::cmp::Ordering::Equal));
        let top_100: Vec<String> = top_rated.iter()
            .take(100)
            .map(|m| format!("{} ({}) - ⭐ {:.1}", m.title, m.year, m.rating))
            .collect();
        
        drop(db);
        
        // Create statistics dialog
        let stats_dialog = Window::builder()
            .title("📊 Database Statistics")
            .modal(true)
            .transient_for(&window_clone)
            .default_width(600)
            .default_height(500)
            .build();
        
        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        
        let stats_box = Box::new(Orientation::Vertical, 12);
        stats_box.set_margin_start(20);
        stats_box.set_margin_end(20);
        stats_box.set_margin_top(20);
        stats_box.set_margin_bottom(20);
        
        // Overview section
        let overview_label = Label::new(None);
        overview_label.set_xalign(0.0);
        overview_label.set_markup(&format!(
            "<span size='large' weight='bold'>📊 Overview</span>\n\n\
            <b>Total Movies:</b> {}\n\
            <b>Average Rating:</b> ⭐ {:.2}/10\n\
            <b>Total Runtime:</b> {} hours ({} minutes)\n\
            <b>Average Runtime:</b> {} minutes\n\
            <b>Year Range:</b> {} - {}",
            total_movies,
            avg_rating,
            total_runtime / 60,
            total_runtime,
            avg_runtime,
            oldest_year,
            newest_year
        ));
        stats_box.append(&overview_label);
        stats_box.append(&Separator::new(Orientation::Horizontal));
        
        // Top rated section
        let top_rated_label = Label::new(None);
        top_rated_label.set_xalign(0.0);
        top_rated_label.set_markup(&format!(
            "<span size='large' weight='bold'>🏆 Top 100 Rated Movies</span>\n\n{}",
            top_100.join("\n")
        ));
        stats_box.append(&top_rated_label);
        stats_box.append(&Separator::new(Orientation::Horizontal));
        
        // Genre breakdown
        let genre_text = genre_list.iter()
            .take(10)
            .map(|(genre, count)| format!("<b>{}:</b> {} movies", genre, count))
            .collect::<Vec<String>>()
            .join("\n");
        
        let genre_label = Label::new(None);
        genre_label.set_xalign(0.0);
        genre_label.set_markup(&format!(
            "<span size='large' weight='bold'>🎭 Top Genres</span>\n\n{}",
            genre_text
        ));
        stats_box.append(&genre_label);
        stats_box.append(&Separator::new(Orientation::Horizontal));
        
        // Decade breakdown
        let decade_text = decade_list.iter()
            .map(|(decade, count)| format!("<b>{}s:</b> {} movies", decade, count))
            .collect::<Vec<String>>()
            .join("\n");
        
        let decade_label = Label::new(None);
        decade_label.set_xalign(0.0);
        decade_label.set_markup(&format!(
            "<span size='large' weight='bold'>📅 By Decade</span>\n\n{}",
            decade_text
        ));
        stats_box.append(&decade_label);
        
        // Close button
        let close_button = Button::with_label("Close");
        close_button.set_halign(Align::End);
        stats_box.append(&close_button);
        
        let stats_dialog_clone = stats_dialog.clone();
        close_button.connect_clicked(move |_| {
            stats_dialog_clone.close();
        });
        
        scroll.set_child(Some(&stats_box));
        stats_dialog.set_child(Some(&scroll));
        stats_dialog.present();
    });
}

fn main() {
    // Set up panic handler to get better error messages
    std::panic::set_hook(std::boxed::Box::new(|panic_info: &std::panic::PanicHookInfo| {
        eprintln!("=== PANIC ===");
        eprintln!("{}", panic_info);
        if let Some(location) = panic_info.location() {
            eprintln!("Location: {}:{}:{}", location.file(), location.line(), location.column());
        }
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            eprintln!("Panic message: {}", s);
        }
        eprintln!("=============");
    }));
    
    let app = Application::builder()
        .application_id("com.example.moviedb")
        .build();

    app.connect_activate(build_ui);

    app.run();
}
