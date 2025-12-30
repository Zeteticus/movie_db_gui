// Cargo.toml dependencies:
// [dependencies]
// gtk = { version = "0.7", package = "gtk4", features = ["v4_10"] }
// reqwest = { version = "0.11", features = ["blocking", "json"] }
// serde = { version = "1.0", features = ["derive"] }
// serde_json = "1.0"
// urlencoding = "2.1"

use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box, Button, Entry, Label, ListBox, ScrolledWindow, 
          Orientation, SearchEntry, DropDown, Grid, Frame, Separator, StringList, Window};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::{File, read_dir};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::rc::Rc;
use serde::{Deserialize, Serialize};
use gtk::glib;

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

    let vbox = Box::new(Orientation::Vertical, 4);
    
    let title_label = Label::new(Some(&format!("{} ({})", movie.title, movie.year)));
    title_label.set_xalign(0.0);
    title_label.set_markup(&format!("<b>{}</b> ({})", movie.title, movie.year));
    
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
    let dialog = Window::builder()
        .title("TMDB API Key Required")
        .modal(true)
        .transient_for(window)
        .default_width(500)
        .default_height(200)
        .build();

    let content = Box::new(Orientation::Vertical, 12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);

    let info_label = Label::new(Some(
        "To fetch movie metadata, you need a TMDB API key.\n\
        Get one free at: https://www.themoviedb.org/settings/api\n\n\
        Enter your API key below:"
    ));
    info_label.set_wrap(true);

    let api_entry = Entry::new();
    api_entry.set_placeholder_text(Some("Enter TMDB API key"));

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
        *api_key_clone.borrow_mut() = api_entry.text().to_string();
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
    
    header.append(&title_label);
    header.append(&Box::new(Orientation::Horizontal, 0));
    header.set_hexpand(true);
    title_label.set_hexpand(true);
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

    let details_box = Box::new(Orientation::Vertical, 8);
    details_box.set_margin_start(12);
    details_box.set_margin_end(12);
    details_box.set_margin_top(12);
    details_box.set_margin_bottom(12);

    let details_label = Label::new(Some("Select a movie to view details"));
    details_label.set_xalign(0.0);
    details_label.set_wrap(true);
    details_box.append(&details_label);

    let action_box = Box::new(Orientation::Horizontal, 8);
    let delete_button = Button::with_label("🗑️ Delete");
    action_box.append(&delete_button);
    details_box.append(&action_box);

    details_frame.set_child(Some(&details_box));
    main_box.append(&details_frame);

    window.set_child(Some(&main_box));

    // Populate initial list
    let db_clone = db.clone();
    let movies = db_clone.borrow().list_all();
    for movie in &movies {
        let row = create_movie_row(movie);
        list_box.append(&row);
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
    let db_clone = db.clone();
    let selected_movie_id = Rc::new(RefCell::new(0u32));
    let selected_movie_id_clone = selected_movie_id.clone();
    
    list_box.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            let index = row.index() as usize;
            let movies = db_clone.borrow().list_all();
            if let Some(movie) = movies.get(index) {
                *selected_movie_id_clone.borrow_mut() = movie.id;
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
                    movie.title, movie.year, movie.director,
                    movie.genre.join(", "), movie.rating, movie.runtime,
                    movie.cast.join(", "), movie.description, movie.file_path,
                    movie.tmdb_id
                );
                details_label_clone.set_markup(&details);
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
                    
                    // Spawn background thread
                    std::thread::spawn(move || {
                        let video_extensions = vec!["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v"];
                        
                        if let Ok(entries) = read_dir(&path_str) {
                            for entry in entries.flatten() {
                                let entry_path = entry.path();
                                if entry_path.is_file() {
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
                                                
                                                let _ = sender.send_blocking(("status".to_string(), format!("Fetching: {}", clean_title), None));
                                                
                                                // Fetch metadata in background thread
                                                let client = reqwest::blocking::Client::new();
                                                let search_url = format!(
                                                    "https://api.themoviedb.org/3/search/movie?api_key={}&query={}",
                                                    api_key,
                                                    urlencoding::encode(&clean_title)
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
                                                                        file_path: file_path_str,
                                                                        poster_url,
                                                                        tmdb_id: movie_id,
                                                                    };
                                                                    
                                                                    let _ = sender.send_blocking(("add".to_string(), format!("✓ Added: {}", clean_title), Some(movie)));
                                                                    continue;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                
                                                // If we get here, metadata fetch failed
                                                let movie = Movie {
                                                    id: 0,
                                                    title: clean_title.clone(),
                                                    year: 0,
                                                    director: String::from("Unknown"),
                                                    genre: vec![String::from("Uncategorized")],
                                                    rating: 0.0,
                                                    runtime: 0,
                                                    description: String::from("Metadata not found"),
                                                    cast: vec![],
                                                    file_path: file_path_str,
                                                    poster_url: String::new(),
                                                    tmdb_id: 0,
                                                };
                                                let _ = sender.send_blocking(("add".to_string(), format!("⚠ Added without metadata: {}", clean_title), Some(movie)));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                        let _ = sender.send_blocking(("complete".to_string(), String::new(), None));
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

    window.present();
}

fn main() {
    let app = Application::builder()
        .application_id("com.example.moviedb")
        .build();

    app.connect_activate(build_ui);

    app.run();
}
