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
struct CastMember {
    name: String,
    #[serde(default)]
    profile_path: String,  // TMDB profile photo URL
    #[serde(default)]
    character: String,     // Character name
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
        .map(|p| format!("https://image.tmdb.org/t/p/w500{}", p))
        .unwrap_or_default();
    
    let poster_path = if !poster_url.is_empty() {
        download_poster(&poster_url, movie_id).unwrap_or_default()
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
    
    // Store the movie ID in the row's name property for later retrieval
    row.set_widget_name(&movie.id.to_string());
    
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

    let db = Rc::new(RefCell::new(MovieDatabase::new("movies.db", &api_key)));

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
    let show_cast_button = Button::with_label("⭐ Show Cast");
    let associate_file_button = Button::with_label("📎 Associate File");
    let delete_button = Button::with_label("🗑️ Delete");
    action_box.append(&play_button);
    action_box.append(&show_cast_button);
    action_box.append(&associate_file_button);
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
                                                    cast_details: vec![],
                                                    file_path,
                                                    poster_url: String::new(),
                                                    tmdb_id: 0,
                                                    imdb_id: String::new(),
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

    // Helper function to refresh list with current filters and sorting
    fn refresh_movie_list(
        list_box: &ListBox,
        db: &Rc<RefCell<MovieDatabase>>,
        search_query: &str,
        genre_filter: &str,
        sort_by: &str,
    ) {
        while let Some(child) = list_box.first_child() {
            list_box.remove(&child);
        }

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

        for movie in &results {
            let row = create_movie_row(movie);
            list_box.append(&row);
        }
    }

    // Search functionality - only trigger on Enter key
    let list_box_clone = list_box.clone();
    let db_clone = db.clone();
    let genre_dropdown_clone = genre_dropdown.clone();
    let sort_dropdown_clone = sort_dropdown.clone();
    search_entry.connect_activate(move |entry| {
        let query = entry.text();
        let selected_idx = genre_dropdown_clone.selected();
        let genres = ["All", "Action", "Comedy", "Drama", "Film Noir", "Horror", "Sci-Fi", "Thriller", "Romance"];
        let selected_genre = genres.get(selected_idx as usize).unwrap_or(&"All");
        
        let sort_idx = sort_dropdown_clone.selected();
        let sorts = ["Title (A-Z)", "Year (Newest)", "Year (Oldest)", "Rating (High-Low)", "Rating (Low-High)", "Date Added (Newest)", "Date Added (Oldest)"];
        let sort_by = sorts.get(sort_idx as usize).unwrap_or(&"Title (A-Z)");
        
        refresh_movie_list(&list_box_clone, &db_clone, &query.to_string(), selected_genre, sort_by);
    });

    // Genre filter
    let list_box_clone = list_box.clone();
    let db_clone = db.clone();
    let search_entry_clone = search_entry.clone();
    let sort_dropdown_clone = sort_dropdown.clone();
    genre_dropdown.connect_selected_notify(move |dropdown| {
        let selected_idx = dropdown.selected();
        let genres = ["All", "Action", "Comedy", "Drama", "Film Noir", "Horror", "Sci-Fi", "Thriller", "Romance"];
        let selected_genre = genres.get(selected_idx as usize).unwrap_or(&"All");
        
        let query = search_entry_clone.text().to_string();
        let sort_idx = sort_dropdown_clone.selected();
        let sorts = ["Title (A-Z)", "Year (Newest)", "Year (Oldest)", "Rating (High-Low)", "Rating (Low-High)", "Date Added (Newest)", "Date Added (Oldest)"];
        let sort_by = sorts.get(sort_idx as usize).unwrap_or(&"Title (A-Z)");
        
        refresh_movie_list(&list_box_clone, &db_clone, &query, selected_genre, sort_by);
    });
    
    // Sort dropdown
    let list_box_clone = list_box.clone();
    let db_clone = db.clone();
    let search_entry_clone = search_entry.clone();
    let genre_dropdown_clone = genre_dropdown.clone();
    sort_dropdown.connect_selected_notify(move |dropdown| {
        let sort_idx = dropdown.selected();
        let sorts = ["Title (A-Z)", "Year (Newest)", "Year (Oldest)", "Rating (High-Low)", "Rating (Low-High)", "Date Added (Newest)", "Date Added (Oldest)"];
        let sort_by = sorts.get(sort_idx as usize).unwrap_or(&"Title (A-Z)");
        
        let query = search_entry_clone.text().to_string();
        let selected_idx = genre_dropdown_clone.selected();
        let genres = ["All", "Action", "Comedy", "Drama", "Film Noir", "Horror", "Sci-Fi", "Thriller", "Romance"];
        let selected_genre = genres.get(selected_idx as usize).unwrap_or(&"All");
        
        refresh_movie_list(&list_box_clone, &db_clone, &query, selected_genre, sort_by);
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

    // Associate File button
    let db_clone = db.clone();
    let window_clone = window.clone();
    let selected_movie_id_clone = selected_movie_id.clone();
    let details_label_clone = details_label.clone();
    let list_box_clone = list_box.clone();
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
                            let row = create_movie_row(movie);
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
                    
                    // Get API key and existing paths before spawning thread (Rc can't be sent)
                    let api_key = db_clone3.borrow().tmdb_api_key.clone();
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
                                                        cast_details: vec![],
                                                        file_path,
                                                        poster_url: String::new(),
                                                        tmdb_id: 0,
                                                        imdb_id: String::new(),
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
                                        .map(|p| format!("https://image.tmdb.org/t/p/w500{}", p))
                                        .unwrap_or_default();
                                    
                                    let poster_path = if !poster_url.is_empty() {
                                        download_poster(&poster_url, tmdb_movie_id).unwrap_or_default()
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

    // Edit Metadata button
    let window_clone = window.clone();
    let db_clone = db.clone();
    let selected_movie_id_clone = selected_movie_id.clone();
    let details_label_clone = details_label.clone();
    let list_box_clone = list_box.clone();
    let status_bar_clone = status_bar.clone();
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
                    let row = create_movie_row(movie);
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
    let status_bar_clone = status_bar.clone();
    let selected_movie_id_clone = selected_movie_id.clone();
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
            let file_path = movie.file_path.clone();
            let api_key = db.tmdb_api_key.clone();
            drop(db); // Release borrow
            
            // Create selection dialog
            let selection_dialog = Window::builder()
                .title(&format!("Select Version: {}", movie_title))
                .modal(true)
                .transient_for(&window_clone)
                .default_width(600)
                .default_height(400)
                .build();
            
            let dialog_box = Box::new(Orientation::Vertical, 12);
            dialog_box.set_margin_start(20);
            dialog_box.set_margin_end(20);
            dialog_box.set_margin_top(20);
            dialog_box.set_margin_bottom(20);
            
            let instruction = Label::new(Some(&format!("Select the correct version of \"{}\":", movie_title)));
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
            
            std::thread::spawn(move || {
                // Search TMDB
                let search_url = format!(
                    "https://api.themoviedb.org/3/search/movie?api_key={}&query={}",
                    api_key,
                    urlencoding::encode(&movie_title)
                );
                
                if let Ok(response) = reqwest::blocking::get(&search_url) {
                    if let Ok(search_result) = response.json::<TMDBSearchResponse>() {
                        let results: Vec<(u32, String, String, f32)> = search_result.results.iter()
                            // Show ALL results (TMDB returns up to 20 per page by default)
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
                        
                        let _ = sender.send_blocking(results);
                    }
                }
            });
            
            // Update UI with results
            glib::spawn_future_local(async move {
                if let Ok(results) = receiver.recv().await {
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
                                
                                // Extract API key before spawning thread (Rc can't be sent)
                                let api_key = db_clone3.borrow().tmdb_api_key.clone();
                                
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
                                                .map(|p| format!("https://image.tmdb.org/t/p/w500{}", p))
                                                .unwrap_or_default();
                                            
                                            let poster_path = if !poster_url.is_empty() {
                                                download_poster(&poster_url, tmdb_id).unwrap_or_default()
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
                                            };
                                            
                                            let _ = sender2.send_blocking(Some((movie_id, new_movie)));
                                            return;
                                        }
                                    }
                                    let _ = sender2.send_blocking(None);
                                });
                                
                                glib::spawn_future_local(async move {
                                    if let Ok(Some((old_id, new_movie))) = receiver2.recv().await {
                                        db_clone3.borrow_mut().delete_movie(old_id);
                                        db_clone3.borrow_mut().add_movie(new_movie);
                                        
                                        // Refresh list
                                        while let Some(child) = list_box_clone3.first_child() {
                                            list_box_clone3.remove(&child);
                                        }
                                        
                                        let movies = db_clone3.borrow().list_all();
                                        for movie in &movies {
                                            let row = create_movie_row(movie);
                                            list_box_clone3.append(&row);
                                        }
                                        
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
                
                // Update UI with results
                glib::spawn_future_local(async move {
                    if let Ok(results) = receiver.recv().await {
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
                                                    .map(|p| format!("https://image.tmdb.org/t/p/w500{}", p))
                                                    .unwrap_or_default();
                                                
                                                let poster_path = if !poster_url.is_empty() {
                                                    download_poster(&poster_url, tmdb_id).unwrap_or_default()
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
                                                };
                                                
                                                let _ = sender2.send_blocking(Some((details.title, movie)));
                                                return;
                                            }
                                        }
                                        let _ = sender2.send_blocking(None);
                                    });
                                    
                                    glib::spawn_future_local(async move {
                                        if let Ok(Some((title, movie))) = receiver2.recv().await {
                                            db_clone4.borrow_mut().add_movie(movie.clone());
                                            
                                            let row = create_movie_row(&movie);
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

    window.present();
}

fn main() {
    let app = Application::builder()
        .application_id("com.example.moviedb")
        .build();

    app.connect_activate(build_ui);

    app.run();
}
