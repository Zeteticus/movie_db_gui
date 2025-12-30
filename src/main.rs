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
use std::io::{BufRead, BufReader, Write, Read};
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
}

fn default_auto_scan() -> bool {
    true  // Enable by default
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
struct Movie {
    id: u32,
    title: String,
    year: u16,
    director: String,
    genre: Vec<String>,
    rating: f32,
    runtime: u16,
    description: String,
    cast: Vec<String>,
    file_path: String,
    poster_url: String,
    tmdb_id: u32,
    #[serde(default)]
    poster_path: String,  // Local cached poster path
}

#[derive(Debug, Deserialize)]
struct TMDBSearchResponse {
    results: Vec<TMDBMovie>,
}

#[derive(Debug, Deserialize)]
struct TMDBMovie {
    id: u32,
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
}

#[derive(Debug, Deserialize)]
struct TMDBCrew {
    name: String,
    job: String,
}

struct MovieDatabase {
    movies: HashMap<u32, Movie>,
    next_id: u32,
    data_file: String,
    tmdb_api_key: String,
}

fn download_poster(poster_url: &str, movie_id: u32) -> Option<String> {
    if poster_url.is_empty() {
        return None;
    }
    
    // Create posters directory if it doesn't exist
    let posters_dir = "posters";
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
    
    let movie_id = search_response.results[0].id;
    
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
    
    let genres: Vec<String> = details.genres
        .iter()
        .map(|g| g.name.clone())
        .collect();
    
    let poster_url = details.poster_path
        .map(|p| format!("https://image.tmdb.org/t/p/w500{}", p))
        .unwrap_or_default();
    
    let poster_path = if !poster_url.is_empty() {
        download_poster(&poster_url, movie_id).unwrap_or_default()
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
        file_path,
        poster_url,
        tmdb_id: movie_id,
        poster_path,
    })
}

impl MovieDatabase {
    fn new(data_file: &str, api_key: &str) -> Self {
        let mut db = MovieDatabase {
            movies: HashMap::new(),
            next_id: 1,
            data_file: data_file.to_string(),
            tmdb_api_key: api_key.to_string(),
        };
        db.load_from_file();
        db
    }

    fn add_movie(&mut self, mut movie: Movie) {
        movie.id = self.next_id;
        self.movies.insert(self.next_id, movie);
        self.next_id += 1;
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
            self.save_to_file();
            true
        } else {
            false
        }
    }

    fn save_to_file(&self) {
        let mut file = File::create(&self.data_file).expect("Unable to create file");
        for movie in self.movies.values() {
            let json = serde_json::to_string(movie).unwrap();
            writeln!(file, "{}", json).expect("Unable to write to file");
        }
    }

    fn load_from_file(&mut self) {
        if !Path::new(&self.data_file).exists() {
            return;
        }

        let file = File::open(&self.data_file).expect("Unable to open file");
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

    fn list_all(&self) -> Vec<Movie> {
        let mut movies: Vec<Movie> = self.movies.values().cloned().collect();
        movies.sort_by(|a, b| a.title.cmp(&b.title));
        movies
    }
}

fn create_movie_row(movie: &Movie) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let hbox = Box::new(Orientation::Horizontal, 12);
    hbox.set_margin_start(12);
    hbox.set_margin_end(12);
    hbox.set_margin_top(8);
    hbox.set_margin_bottom(8);

    // Add poster thumbnail
    let poster_box = Box::new(Orientation::Vertical, 0);
    poster_box.set_size_request(60, 90);
    
    if !movie.poster_path.is_empty() && Path::new(&movie.poster_path).exists() {
        if let Ok(pixbuf) = Pixbuf::from_file_at_scale(&movie.poster_path, 60, 90, true) {
            let picture = Picture::for_pixbuf(&pixbuf);
            picture.set_can_shrink(true);
            poster_box.append(&picture);
        }
    } else {
        // Placeholder for missing poster
        let placeholder = Label::new(Some("🎬"));
        placeholder.set_markup("<span size='xx-large'>🎬</span>");
        poster_box.append(&placeholder);
    }
    
    hbox.append(&poster_box);

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
        .title("Movie Database Manager")
        .default_width(1000)
        .default_height(700)
        .build();

    let api_key = match show_api_key_dialog(&window) {
        Some(key) => key,
        None => {
            eprintln!("No API key provided. Exiting.");
            return;
        }
    };

    let db = Rc::new(RefCell::new(MovieDatabase::new("movies.db", &api_key)));

    let main_box = Box::new(Orientation::Vertical, 0);

    let header = Box::new(Orientation::Horizontal, 12);
    header.set_margin_start(12);
    header.set_margin_end(12);
    header.set_margin_top(12);
    header.set_margin_bottom(12);

    let title_label = Label::new(Some("🎬 Movie Database"));
    title_label.set_markup("<span size='x-large' weight='bold'>🎬 Movie Database</span>");
    
    let scan_button = Button::with_label("📁 Scan Directory");
    let add_button = Button::with_label("➕ Add Movie");
    let refresh_button = Button::with_label("🔄 Refresh Metadata");
    let settings_button = Button::with_label("⚙️ Settings");
    
    header.append(&title_label);
    header.append(&Box::new(Orientation::Horizontal, 0));
    header.set_hexpand(true);
    title_label.set_hexpand(true);
    header.append(&settings_button);
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

    let genres = StringList::new(&["All", "Action", "Comedy", "Drama", "Horror", "Sci-Fi", "Thriller", "Romance"]);
    let genre_dropdown = DropDown::new(Some(genres), None::<gtk::Expression>);
    genre_dropdown.set_selected(0);

    search_box.append(&search_entry);
    search_box.append(&Label::new(Some("Genre:")));
    search_box.append(&genre_dropdown);
    main_box.append(&search_box);

    let scrolled = ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);
    
    let list_box = ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::Single);
    scrolled.set_child(Some(&list_box));
    main_box.append(&scrolled);

    let details_frame = Frame::new(Some("Movie Details"));
    details_frame.set_margin_start(12);
    details_frame.set_margin_end(12);
    details_frame.set_margin_top(12);
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
    let delete_button = Button::with_label("🗑️ Delete");
    action_box.append(&play_button);
    action_box.append(&delete_button);
    details_box.append(&action_box);

    details_main_box.append(&details_box);
    details_frame.set_child(Some(&details_main_box));
    main_box.append(&details_frame);

    window.set_child(Some(&main_box));

    // Populate initial list
    let db_clone = db.clone();
    let movies = db_clone.borrow().list_all();
    for movie in &movies {
        let row = create_movie_row(movie);
        list_box.append(&row);
    }

    // Auto-scan on startup if enabled
    let config = load_config().unwrap_or_default();
    if config.auto_scan_on_startup && !config.scan_directories.is_empty() {
        let db_clone = db.clone();
        let list_box_clone = list_box.clone();
        let status_bar_clone = status_bar.clone();
        let window_clone = window.clone();
        
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
        
        dialog.choose(Some(&window_clone), None::<&gtk::gio::Cancellable>, move |response| {
            if let Ok(1) = response {
                // User chose "Scan Now"
                status_bar_clone.set_text("Auto-scanning configured directories...");
                
                // Spawn auto-scan in background
                let (sender, receiver) = async_channel::unbounded::<(String, String, Option<Movie>)>();
                
                let api_key_clone = api_key.clone();
                let scan_dirs_clone = scan_dirs.clone();
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
                        
                        let _ = sender.send_blocking(("status".to_string(), format!("Found {} video files, fetching metadata in parallel...", files_to_process.len()), None));
                        
                        // Process files in parallel batches of 10
                        let client = reqwest::Client::new();
                        let batch_size = 10;
                        
                        for batch in files_to_process.chunks(batch_size) {
                            let futures: Vec<_> = batch.iter()
                                .map(|(clean_title, file_path_str)| {
                                    let api_key = api_key_clone.clone();
                                    let title = clean_title.clone();
                                    let file_path = file_path_str.clone();
                                    let client = client.clone();
                                    let sender = sender.clone();
                                    
                                    async move {
                                        let _ = sender.send_blocking(("status".to_string(), format!("Fetching: {}", title), None));
                                        
                                        match fetch_movie_metadata_async(&client, &api_key, &title, file_path.clone()).await {
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
                                                    file_path,
                                                    poster_url: String::new(),
                                                    tmdb_id: 0,
                                                    poster_path: String::new(),
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
                                let row = create_movie_row(&movie);
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

    // Search functionality
    let list_box_clone = list_box.clone();
    let db_clone = db.clone();
    let genre_dropdown_clone = genre_dropdown.clone();
    search_entry.connect_search_changed(move |entry| {
        let query = entry.text();
        let selected_idx = genre_dropdown_clone.selected();
        let genres = ["All", "Action", "Comedy", "Drama", "Horror", "Sci-Fi", "Thriller", "Romance"];
        let selected_genre = genres.get(selected_idx as usize).unwrap_or(&"All");
        
        while let Some(child) = list_box_clone.first_child() {
            list_box_clone.remove(&child);
        }

        let results = if query.is_empty() {
            db_clone.borrow().search_by_genre(selected_genre)
        } else {
            db_clone.borrow().search_by_title(&query.to_string())
        };

        for movie in &results {
            let row = create_movie_row(movie);
            list_box_clone.append(&row);
        }
    });

    // Genre filter
    let list_box_clone = list_box.clone();
    let db_clone = db.clone();
    let search_entry_clone = search_entry.clone();
    genre_dropdown.connect_selected_notify(move |dropdown| {
        let selected_idx = dropdown.selected();
        let genres = ["All", "Action", "Comedy", "Drama", "Horror", "Sci-Fi", "Thriller", "Romance"];
        let selected_genre = genres.get(selected_idx as usize).unwrap_or(&"All");
        
        while let Some(child) = list_box_clone.first_child() {
            list_box_clone.remove(&child);
        }

        let query = search_entry_clone.text().to_string();
        let results = if query.is_empty() {
            db_clone.borrow().search_by_genre(selected_genre)
        } else {
            db_clone.borrow().search_by_title(&query)
        };

        for movie in &results {
            let row = create_movie_row(movie);
            list_box_clone.append(&row);
        }
    });

    // Movie selection
    let details_label_clone = details_label.clone();
    let poster_display_clone = poster_display.clone();
    let db_clone = db.clone();
    let selected_movie_id = Rc::new(RefCell::new(0u32));
    let selected_movie_id_clone = selected_movie_id.clone();
    
    list_box.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            let index = row.index() as usize;
            let movies = db_clone.borrow().list_all();
            if let Some(movie) = movies.get(index) {
                *selected_movie_id_clone.borrow_mut() = movie.id;
                
                // Update poster
                if !movie.poster_path.is_empty() && Path::new(&movie.poster_path).exists() {
                    if let Ok(pixbuf) = Pixbuf::from_file_at_scale(&movie.poster_path, 200, 300, true) {
                        poster_display_clone.set_pixbuf(Some(&pixbuf));
                    }
                } else {
                    poster_display_clone.set_pixbuf(None);
                }
                
                // Escape all text that goes into markup
                let escaped_title = escape_markup(&movie.title);
                let escaped_director = escape_markup(&movie.director);
                let escaped_genre = escape_markup(&movie.genre.join(", "));
                let escaped_cast = escape_markup(&movie.cast.join(", "));
                let escaped_description = escape_markup(&movie.description);
                let escaped_file = escape_markup(&movie.file_path);
                
                let details = format!(
                    "<b>{}</b> ({})\n\n\
                    <b>Director:</b> {}\n\
                    <b>Genre:</b> {}\n\
                    <b>Rating:</b> ⭐ {:.1}/10\n\
                    <b>Runtime:</b> {} minutes\n\
                    <b>Cast:</b> {}\n\n\
                    <b>Description:</b>\n{}\n\n\
                    <b>File:</b> {}\n\
                    <b>TMDB ID:</b> {}",
                    escaped_title, movie.year, escaped_director,
                    escaped_genre, movie.rating, movie.runtime,
                    escaped_cast, escaped_description, escaped_file,
                    movie.tmdb_id
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
                if !movie.file_path.is_empty() && Path::new(&movie.file_path).exists() {
                    // Try to launch VLC with suppressed output
                    match Command::new("vlc")
                        .arg(&movie.file_path)
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                    {
                        Ok(_) => {
                            status_bar_clone.set_text(&format!("Playing: {}", movie.title));
                        }
                        Err(_) => {
                            // Try flatpak version
                            match Command::new("flatpak")
                                .args(["run", "org.videolan.VLC", &movie.file_path])
                                .stdout(Stdio::null())
                                .stderr(Stdio::null())
                                .spawn()
                            {
                                Ok(_) => {
                                    status_bar_clone.set_text(&format!("Playing: {}", movie.title));
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

    // Delete button
    let db_clone = db.clone();
    let list_box_clone = list_box.clone();
    let window_clone = window.clone();
    let selected_movie_id_clone = selected_movie_id.clone();
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
            dialog.choose(Some(&window_clone), None::<&gtk::gio::Cancellable>, move |response| {
                if let Ok(1) = response {
                    if db_clone2.borrow_mut().delete_movie(movie_id) {
                        while let Some(child) = list_box_clone2.first_child() {
                            list_box_clone2.remove(&child);
                        }
                        let movies = db_clone2.borrow().list_all();
                        for movie in &movies {
                            let row = create_movie_row(movie);
                            list_box_clone2.append(&row);
                        }
                    }
                }
            });
        }
    });

    // Scan directory
    let window_clone = window.clone();
    let db_clone = db.clone();
    let list_box_clone = list_box.clone();
    let status_bar_clone = status_bar.clone();
    scan_button.connect_clicked(move |_| {
        let dialog = gtk::FileDialog::new();
        dialog.set_title("Select Movie Directory");

        let db_clone2 = db_clone.clone();
        let list_box_clone2 = list_box_clone.clone();
        let status_bar_clone2 = status_bar_clone.clone();
        
        dialog.select_folder(Some(&window_clone), None::<&gtk::gio::Cancellable>, move |result| {
            if let Ok(folder) = result {
                if let Some(path) = folder.path() {
                    let path_str = path.to_string_lossy().to_string();
                    
                    let db_clone3 = db_clone2.clone();
                    let list_box_clone3 = list_box_clone2.clone();
                    let status_bar_clone3 = status_bar_clone2.clone();
                    
                    // Create async channel
                    let (sender, receiver) = async_channel::unbounded::<(String, String, Option<Movie>)>();
                    
                    // Get API key before spawning thread
                    let api_key = db_clone3.borrow().tmdb_api_key.clone();
                    
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
                            
                            let _ = sender.send_blocking(("status".to_string(), format!("Found {} video files, fetching metadata in parallel...", files_to_process.len()), None));
                            
                            // Process files in parallel batches of 10
                            let client = reqwest::Client::new();
                            let batch_size = 10;
                            
                            for batch in files_to_process.chunks(batch_size) {
                                let futures: Vec<_> = batch.iter()
                                    .map(|(clean_title, file_path_str)| {
                                        let api_key = api_key.clone();
                                        let title = clean_title.clone();
                                        let file_path = file_path_str.clone();
                                        let client = client.clone();
                                        let sender = sender.clone();
                                        
                                        async move {
                                            let _ = sender.send_blocking(("status".to_string(), format!("Fetching: {}", title), None));
                                            
                                            match fetch_movie_metadata_async(&client, &api_key, &title, file_path.clone()).await {
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
                                                        file_path,
                                                        poster_url: String::new(),
                                                        tmdb_id: 0,
                                                        poster_path: String::new(),
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
                                        let row = create_movie_row(movie);
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
    refresh_button.connect_clicked(move |_| {
        let movie_id = *selected_movie_id_clone.borrow();
        if movie_id > 0 {
            let db_clone2 = db_clone.clone();
            let list_box_clone2 = list_box_clone.clone();
            let status_bar_clone2 = status_bar_clone.clone();
            
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
                                    
                                    let genres: Vec<String> = details.genres
                                        .iter()
                                        .map(|g| g.name.clone())
                                        .collect();
                                    
                                    let poster_url = details.poster_path
                                        .map(|p| format!("https://image.tmdb.org/t/p/w500{}", p))
                                        .unwrap_or_default();
                                    
                                    let poster_path = if !poster_url.is_empty() {
                                        download_poster(&poster_url, tmdb_movie_id).unwrap_or_default()
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
                                        file_path: file_path.clone(),
                                        poster_url,
                                        tmdb_id: tmdb_movie_id,
                                        poster_path,
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
                            let row = create_movie_row(movie);
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

    // Add movie dialog
    let window_clone = window.clone();
    let db_clone = db.clone();
    let list_box_clone = list_box.clone();
    let status_bar_clone = status_bar.clone();
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

        content.append(&grid);

        let button_box = Box::new(Orientation::Horizontal, 8);
        button_box.set_halign(gtk::Align::End);
        let cancel_btn = Button::with_label("Cancel");
        let add_btn = Button::with_label("Search & Add");
        button_box.append(&cancel_btn);
        button_box.append(&add_btn);
        content.append(&button_box);

        dialog.set_child(Some(&content));

        let dialog_clone = dialog.clone();
        cancel_btn.connect_clicked(move |_| {
            dialog_clone.close();
        });

        let dialog_clone = dialog.clone();
        let db_clone2 = db_clone.clone();
        let list_box_clone2 = list_box_clone.clone();
        let status_bar_clone2 = status_bar_clone.clone();
        add_btn.connect_clicked(move |_| {
            let title = title_entry.text().to_string();
            if !title.is_empty() {
                dialog_clone.close();
                
                let db_clone3 = db_clone2.clone();
                let list_box_clone3 = list_box_clone2.clone();
                let status_bar_clone3 = status_bar_clone2.clone();
                
                // Get API key before spawning thread
                let api_key = db_clone3.borrow().tmdb_api_key.clone();
                
                let (sender, receiver) = async_channel::unbounded::<Option<(String, Movie)>>();
                
                // Update status immediately
                status_bar_clone3.set_text(&format!("Searching for: {}", title));
                
                let title_clone = title.clone();
                std::thread::spawn(move || {
                    let client = reqwest::blocking::Client::new();
                    let search_url = format!(
                        "https://api.themoviedb.org/3/search/movie?api_key={}&query={}",
                        api_key,
                        urlencoding::encode(&title_clone)
                    );
                    
                    if let Ok(response) = client.get(&search_url).send() {
                        if let Ok(search_response) = response.json::<TMDBSearchResponse>() {
                            if !search_response.results.is_empty() {
                                let movie_id = search_response.results[0].id;
                                let details_url = format!(
                                    "https://api.themoviedb.org/3/movie/{}?api_key={}&append_to_response=credits",
                                    movie_id, api_key
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
                                        
                                        let genres: Vec<String> = details.genres
                                            .iter()
                                            .map(|g| g.name.clone())
                                            .collect();
                                        
                                        let poster_url = details.poster_path
                                            .map(|p| format!("https://image.tmdb.org/t/p/w500{}", p))
                                            .unwrap_or_default();
                                        
                                        let poster_path = if !poster_url.is_empty() {
                                            download_poster(&poster_url, movie_id).unwrap_or_default()
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
                                            file_path: String::new(),
                                            poster_url,
                                            tmdb_id: movie_id,
                                            poster_path,
                                        };
                                        
                                        let _ = sender.send_blocking(Some((details.title, movie)));
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    
                    let _ = sender.send_blocking(None);
                });
                
                glib::spawn_future_local(async move {
                    if let Ok(result) = receiver.recv().await {
                        if let Some((title, movie)) = result {
                            db_clone3.borrow_mut().add_movie(movie);
                            
                            while let Some(child) = list_box_clone3.first_child() {
                                list_box_clone3.remove(&child);
                            }
                            let movies = db_clone3.borrow().list_all();
                            for movie in &movies {
                                let row = create_movie_row(movie);
                                list_box_clone3.append(&row);
                            }
                            status_bar_clone3.set_text(&format!("✓ Added: {}", title));
                        } else {
                            status_bar_clone3.set_text(&format!("❌ Movie not found: {}", title));
                        }
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
                
                // Save to config
                let config = Config {
                    tmdb_api_key: new_key,
                    scan_directories: dirs_list.borrow().clone(),
                    auto_scan_on_startup: auto_scan_check.is_active(),
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

    window.present();
}

fn main() {
    let app = Application::builder()
        .application_id("com.example.moviedb")
        .build();

    app.connect_activate(build_ui);

    app.run();
}
