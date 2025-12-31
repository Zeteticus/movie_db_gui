# 📽️ Mark's Movie Database (MMDB)

A powerful, fast, and beautiful GTK4 movie collection manager built in Rust. Automatically fetches comprehensive metadata from TMDB, including posters, cast photos, ratings, and IMDb references.

![Version](https://img.shields.io/badge/version-0.1.0-blue)
![License](https://img.shields.io/badge/license-MIT-green)
![Rust](https://img.shields.io/badge/rust-1.70+-orange)

## ✨ Features

### 🎬 Comprehensive Metadata
- **Automatic TMDB integration** - Fetches titles, years, directors, genres, ratings, and descriptions
- **High-quality posters** - Downloaded and cached locally for offline viewing
- **Cast information** - Top 5 actors with character names and professional headshots
- **IMDb IDs** - Direct reference to IMDb entries for cross-referencing
- **Full details** - Runtime, release year, plot summaries, and more

### 🔍 Smart Search & Organization
- **Optimized search** - Press Enter to search (no lag while typing)
- **Genre filtering** - Action, Comedy, Drama, Film Noir, Horror, Sci-Fi, Thriller, Romance
- **7 sort options**:
  - Title (A-Z)
  - Year (Newest/Oldest)
  - Rating (High-Low/Low-High)
  - Date Added (Newest/Oldest)
- **Combined filters** - Search + Genre + Sort work together seamlessly

### 🎞️ Advanced Features
- **"Wrong Movie?" fix** - Choose from up to 20 TMDB results for remakes/reboots (e.g., The Thing 1982 vs 2011)
- **Enhanced "Add Movie"** - Search and select from 20 results, with optional file association
- **File association** - Browse and attach movie files when adding OR associate files with existing movies
- **Parallel scanning** - Process 10 movies simultaneously for blazing-fast imports
- **Recursive directory scanning** - Automatically finds movies in subdirectories
- **Smart duplicate detection** - 60x faster rescans by skipping existing movies
- **Cast photo viewer** - Scrollable dialog with actor headshots and character names
- **VLC integration** - One-click playback
- **Desktop integration** - Application launcher with custom icon

### 📊 Statistics & Analytics
- **Collection overview** - Total movies, average rating, total runtime, year range
- **Top 100 rated movies** - Your best films ranked and ready to view
- **Genre breakdown** - Top 10 genres with movie counts
- **Decade analysis** - Distribution across eras (1950s, 1960s, etc.)

### ⚙️ Configuration & Management
- **Persistent settings** - Auto-scan directories and preferences saved
- **Metadata refresh** - Update all movies or individual selections
- **Manual movie addition** - Add movies with or without files, select exact version
- **File management** - Associate files when adding or later via "Associate File" button
- **Delete management** - Remove from database (files stay safe)
- **Auto-scan on startup** - Optional quick check for new movies

## 📸 Screenshots

```
┌─────────────────────────────────────────────────────────────────────┐
│ 📽️ Mark's Movie Database  [📊][⚙️][✏️][🎞️][🔄][📁][➕]         │
├─────────────────────────────────────────────────────────────────────┤
│ [Search: matrix ⏎] [Genre: All ▼] [Sort: Rating (High-Low) ▼]     │
├─────────────────────────────────────────────────────────────────────┤
│ Movie List              │  Movie Details                            │
│ ─────────────────────  │  ───────────────────────────────────────  │
│ The Matrix (1999)       │  The Matrix (1999)                        │
│ The Dark Knight (2008)  │                                           │
│ Inception (2010)        │  [Poster Image]                           │
│ Interstellar (2014)     │                                           │
│ Blade Runner (1982)     │  Director: Wachowski Sisters              │
│ ...                     │  Genre: Action, Sci-Fi                    │
│                         │  Rating: ⭐ 8.7/10                        │
│                         │  Runtime: 136 minutes                     │
│                         │                                           │
│                         │  Starring:                                │
│                         │    • Keanu Reeves (Neo)                   │
│                         │    • Laurence Fishburne (Morpheus)        │
│                         │    • Carrie-Anne Moss (Trinity)           │
│                         │                                           │
│                         │  Description:                             │
│                         │  Set in the 22nd century...               │
│                         │                                           │
│                         │  File: /movies/matrix.mp4                 │
│                         │  TMDB ID: 603                             │
│  [▶ Play] [⭐ Cast]     │  IMDb ID: tt0133093                       │
│  [📎 File] [🗑️ Delete] │  (https://www.imdb.com/title/tt0133093)  │
└─────────────────────────────────────────────────────────────────────┘
```

## 🚀 Quick Start

### Prerequisites

- **Rust 1.70+** - [Install Rust](https://rustup.rs/)
- **GTK4** - GUI toolkit (instructions below)
- **TMDB API Key** - [Get free key](https://www.themoviedb.org/settings/api)
- **VLC Player** (optional) - For movie playback

#### Install GTK4

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install libgtk-4-dev build-essential
```

**Fedora:**
```bash
sudo dnf install gtk4-devel gcc
```

**Arch Linux:**
```bash
sudo pacman -S gtk4 base-devel
```

**macOS:**
```bash
brew install gtk4
```

### Installation

1. **Clone the repository**
```bash
git clone https://github.com/yourusername/marks-movie-database.git
cd marks-movie-database
```

2. **Build the application**
```bash
cargo build --release
```

3. **Run it**
```bash
cargo run --release
```

4. **Optional: Install desktop integration**
```bash
cp target/release/movie-database ~/.local/bin/
cp movie-database.desktop ~/.local/share/applications/
```

### First Run Setup

1. **Enter your TMDB API key** when prompted
   - Get one free at https://www.themoviedb.org/settings/api
   
2. **Add scan directories** in Settings (⚙️)
   - Click Settings
   - Add one or more movie directories
   - Enable "Auto-scan on startup" (optional)

3. **Scan your collection** (📁 Scan Directory)
   - Or use auto-scan if enabled
   - Wait for parallel metadata fetch
   - Review results

4. **Enjoy!** 🎬

## 📖 Usage Guide

### Adding Movies

#### Automatic Scanning (Recommended)
1. Click **📁 Scan Directory**
2. Select your movie folder
3. Wait for parallel metadata fetch (10 movies at a time)
4. Movies appear with full metadata!

**Supported formats:** MP4, MKV, AVI, MOV, WMV, FLV, WEBM, M4V

**Performance:**
- First scan: ~30 seconds for 100 movies
- Rescan: < 2 seconds (skips existing movies - 60x faster!)

#### Manual Addition with File
1. Click **➕ Add Movie**
2. Enter movie title
3. **(Optional)** Click **Browse** to select file
4. Click **Search**
5. Choose from up to 20 TMDB results
6. Click **Add Selected**

**Perfect for:**
- Movies you already have files for
- Choosing exact version (1982 vs 2011)
- Adding movies to wishlist

#### Manual Addition without File
1. Click **➕ Add Movie**
2. Enter movie title (skip file selection)
3. Click **Search**
4. Select from results
5. Add to collection
6. Associate file later when you get it!

### Associating Files

#### New Movie with File
```
➕ Add Movie → Type title → Browse (select file) → Search → Add Selected
```

#### Existing Movie
```
Select movie → 📎 Associate File → Browse → Select file → Done!
```

**Use cases:**
- Added movie without file, got it later
- File moved to new location
- Wrong file associated during scan
- Fix broken file paths

### Searching Movies

1. Type movie title in search box
2. **Press Enter** to search (optimized - no lag!)
3. Combine with genre filter and sort
4. Click movie to see full details

**Pro tip:** Use genre + sort for browsing (e.g., "Horror" + "Rating High-Low")

### Fixing Wrong Metadata

Got the 2011 remake instead of the 1982 original?

1. Select the movie
2. Click **🎞️ Wrong Movie?**
3. See up to 20 TMDB versions with years and ratings
4. Select the correct one
5. Metadata updates instantly!

**Shows ALL results** (not just 10) - much better for finding obscure films!

### Viewing Statistics

1. Click **📊 Statistics**
2. See:
   - Collection overview (totals, averages)
   - Top 100 rated movies
   - Genre breakdown (top 10)
   - Decade distribution
3. Analyze and enjoy your collection!

### Playing Movies

1. Select a movie
2. Click **▶ Play**
3. Opens in VLC (or default player)

**Note:** File must be associated for playback to work.

### Viewing Cast

1. Select a movie
2. Click **⭐ Show Cast**
3. See actor photos with character names
4. Scroll through full cast list

## ⚙️ Configuration

### Settings Dialog

Access via **⚙️ Settings** button:

- **TMDB API Key** - Your API key for metadata
- **Scan Directories** - Folders to auto-scan on startup
- **Auto-scan on startup** - Automatically check for new movies

### Files & Locations

```
~/.config/movie-database/
├── config.json          # API key and settings
└── movies.db            # Movie database (JSON)

~/.local/share/movie-database/
└── posters/             # Cached poster images
    ├── 278.jpg          # Shawshank Redemption
    ├── 155.jpg          # Dark Knight
    └── ...
```

### Backup Your Database

```bash
cp ~/.config/movie-database/movies.db ~/movies_backup.db
```

## 🎯 Keyboard Shortcuts

| Action | Shortcut |
|--------|----------|
| Search | Type + **Enter** ⏎ |
| Refresh Metadata | Click 🔄 |
| Statistics | Click 📊 |
| Settings | Click ⚙️ |

## 🛠️ Technical Details

### Built With

- **Rust** - Fast, safe systems programming
- **GTK4** - Modern, beautiful UI toolkit
- **TMDB API** - Comprehensive movie database
- **Tokio** - Async runtime for parallel operations
- **Serde** - JSON serialization/deserialization
- **Reqwest** - HTTP client for API calls

### Architecture

```
┌─────────────────────────────────────────┐
│           GTK4 User Interface           │
│   (Search, Filters, Details, Dialogs)  │
├─────────────────────────────────────────┤
│     Movie Database (HashMap + JSON)    │
│    (In-memory + Persistent storage)    │
├─────────────────────────────────────────┤
│   TMDB API Client (Async + Parallel)   │
│     (Metadata, Posters, IMDb IDs)      │
├─────────────────────────────────────────┤
│  Local Storage (JSON + Cached Images)  │
│    (Config, Database, Poster cache)    │
└─────────────────────────────────────────┘
```

### Performance

- **Parallel scanning**: 10 movies at once
- **Smart caching**: Posters stored locally
- **Instant search**: HashMap-based lookup (O(1))
- **Fast sorting**: Efficient in-memory operations
- **Duplicate detection**: Skips existing movies on rescan
- **Optimized search**: No lag while typing (Enter to search)

**Benchmarks (100 movies):**
- First scan: ~30 seconds
- Rescan (no new movies): < 2 seconds (60x faster!)
- Search: Instant
- Sort: < 5ms
- "Wrong Movie?" search: ~4 seconds (fetches 20 results)

### API Usage

**Per movie scan:**
- 1 search call (find TMDB ID)
- 1 details call (get metadata + cast)
- 1 external IDs call (get IMDb ID)
- 1 poster download

**"Wrong Movie?" feature:**
- 1 search call
- Up to 20 detail calls (for year/rating display)
- 1 full metadata call (for selected version)

**Rate limiting:** None with free TMDB API key

## 🐛 Troubleshooting

### "No metadata found"
- ✓ Check your TMDB API key in Settings
- ✓ Verify internet connection
- ✓ Check TMDB API status: https://status.themoviedb.org/
- ✓ Try different search terms (original vs English title)

### "Movies not appearing"
- ✓ Ensure files are in supported formats (MP4, MKV, AVI, etc.)
- ✓ Check file permissions
- ✓ Look for error messages in terminal
- ✓ Verify directory is added in Settings

### "UI freezes during scan"
- This is fixed in current version!
- Update to get parallel scanning (10 at once)

### "Wrong movie metadata"
- ✓ Use the **🎞️ Wrong Movie?** button
- ✓ Search shows up to 20 results now
- ✓ Select correct version by year
- ✓ Metadata updates automatically

### "Posters not loading"
- ✓ Check internet connection
- ✓ Verify poster directory permissions: `~/.local/share/movie-database/posters/`
- ✓ Try refreshing metadata
- ✓ Check available disk space

### "Can't play movie"
- ✓ Verify file exists at path shown in details
- ✓ Install VLC media player
- ✓ Use **📎 Associate File** if file moved
- ✓ Check file permissions

### "Search is slow"
- Current version fixed! Press Enter to search (no lag while typing)

## 🤝 Contributing

Contributions welcome! Here are some ideas:

### Feature Ideas
- [ ] Edit Metadata dialog (button already in UI)
- [ ] Export to CSV/Excel
- [ ] Custom collections/playlists
- [ ] Watched/unwatched tracking
- [ ] Personal ratings overlay
- [ ] Dark mode theme
- [ ] Import from other databases
- [ ] Backup/restore functionality
- [ ] Advanced search (by actor, director, year range)
- [ ] Batch file association
- [ ] Drag & drop file association

### How to Contribute

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Commit changes: `git commit -m 'Add amazing feature'`
4. Push to branch: `git push origin feature/amazing-feature`
5. Open a Pull Request

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- **TMDB** - For the excellent free API and comprehensive database
- **GTK Team** - For the beautiful, modern UI toolkit
- **Rust Community** - For the amazing language and ecosystem
- **Classic film lovers** - Who inspired the "Wrong Movie?" feature
- **Open source contributors** - Who make projects like this possible

## 🗺️ Roadmap

### Version 0.2.0 (Planned)
- [ ] Edit Metadata dialog (manual field editing)
- [ ] Watched/unwatched tracking
- [ ] Personal ratings overlay
- [ ] Custom collections/playlists
- [ ] Dark mode support

### Version 0.3.0 (Future)
- [ ] Export functionality (CSV, Excel)
- [ ] Advanced search filters
- [ ] Backup/restore
- [ ] Multi-language support
- [ ] Batch operations

## 📊 Project Stats

- **Lines of Code**: ~2,700
- **Dependencies**: 16
- **Supported Formats**: 8 video formats
- **API Integrations**: TMDB (+ IMDb ID references)
- **Database Size**: ~1-2KB per movie (metadata only, posters cached separately)

## 🎬 Current Features Summary

### Metadata & Content
✅ TMDB metadata with posters, cast, ratings  
✅ IMDb ID integration  
✅ Cast photos with character names  
✅ Local poster caching  

### Search & Organization
✅ Optimized search (Enter to search - no lag!)  
✅ Genre filtering (8 genres)  
✅ 7 sort options  
✅ Combined filters  

### Adding Movies
✅ Auto-scan with parallel processing  
✅ Manual add with 20 TMDB results  
✅ File association during add  
✅ Add without file (wishlist)  

### Managing Movies
✅ "Wrong Movie?" with 20 results  
✅ Associate file button (existing movies)  
✅ Refresh metadata  
✅ Delete movies  
✅ VLC playback integration  

### Statistics & Analysis
✅ Collection overview  
✅ Top 100 rated movies  
✅ Genre breakdown  
✅ Decade distribution  

### Performance
✅ 60x faster rescans (duplicate detection)  
✅ Parallel scanning (10 at once)  
✅ Instant search  
✅ No typing lag  

---

**Made with ❤️ for movie collectors who appreciate quality metadata and beautiful organization.**

🎬 Happy collecting! 📽️
