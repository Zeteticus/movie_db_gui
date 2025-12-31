# 📽️ Mark's Movie Database (MMDB)

A powerful, fast, and beautiful GTK4 movie collection manager built in Rust. Automatically fetches metadata from TMDB, including posters, cast photos, ratings, and IMDb references.

![Version](https://img.shields.io/badge/version-0.1.0-blue)
![License](https://img.shields.io/badge/license-MIT-green)
![Rust](https://img.shields.io/badge/rust-1.70+-orange)

## ✨ Features

### 🎬 Comprehensive Metadata
- **Automatic TMDB integration** - Fetches titles, years, directors, genres, ratings, and descriptions
- **High-quality posters** - Downloaded and cached locally
- **Cast information** - Top 5 actors with character names and photos
- **IMDb IDs** - Direct reference to IMDb entries
- **Runtime & release year** - Complete movie information

### 🔍 Smart Search & Organization
- **Instant search** - Find movies by title (press Enter to search)
- **Genre filtering** - Action, Comedy, Drama, Film Noir, Horror, Sci-Fi, Thriller, Romance
- **Multiple sort options**:
  - Title (A-Z)
  - Year (Newest/Oldest)
  - Rating (High-Low/Low-High)
  - Date Added (Newest/Oldest)
- **Combined filters** - Search + Genre + Sort work together

### 🎞️ Advanced Features
- **"Wrong Movie?" fix** - Choose correct version for remakes/reboots (e.g., The Thing 1982 vs 2011)
- **Parallel scanning** - Process 10 movies simultaneously for fast imports
- **Recursive directory scanning** - Automatically finds movies in subdirectories
- **Smart duplicate detection** - Skips already-scanned movies on rescans
- **Cast photo viewer** - Scrollable dialog with actor headshots and character names
- **VLC integration** - One-click playback
- **Desktop integration** - Application launcher with icon

### 📊 Statistics & Analytics
- **Collection overview** - Total movies, average rating, total runtime
- **Top 100 rated movies** - Your best films at a glance
- **Genre breakdown** - See your collection distribution
- **Decade analysis** - Movies by era (1950s, 1960s, etc.)

### ⚙️ Configuration & Management
- **Persistent settings** - Auto-scan directories and preferences
- **Metadata refresh** - Update all movies or selected ones
- **Manual movie addition** - Add movies without files
- **Delete management** - Remove from database (files stay safe)
- **Auto-scan on startup** - Optional quick check for new movies

## 📸 Screenshots

```
┌─────────────────────────────────────────────────────────────┐
│ 📽️ Mark's Movie Database    [📊][⚙️][🎞️][🔄][📁][➕]    │
├─────────────────────────────────────────────────────────────┤
│ [Search: matrix    ] [Genre: All ▼] [Sort: Rating ▼]      │
├─────────────────────────────────────────────────────────────┤
│ Movie List              │  Movie Details                    │
│ ─────────────────────  │  ───────────────────────────────  │
│ The Matrix (1999)       │  The Matrix (1999)                │
│ The Dark Knight (2008)  │                                   │
│ Inception (2010)        │  [Poster]                         │
│ Interstellar (2014)     │                                   │
│ ...                     │  Director: Wachowski Sisters      │
│                         │  Genre: Action, Sci-Fi            │
│                         │  Rating: ⭐ 8.7/10                │
│                         │  Runtime: 136 minutes             │
│                         │                                   │
│                         │  Starring:                        │
│                         │    • Keanu Reeves                 │
│                         │    • Laurence Fishburne           │
│                         │    • Carrie-Anne Moss             │
│                         │                                   │
│   [▶ Play] [⭐ Cast]    │  IMDb ID: tt0133093               │
└─────────────────────────────────────────────────────────────┘
```

## 🚀 Quick Start

### Prerequisites

- **Rust 1.70+** - [Install Rust](https://rustup.rs/)
- **GTK4** - GUI toolkit
- **TMDB API Key** - [Get free key](https://www.themoviedb.org/settings/api)

#### Install GTK4 on Ubuntu/Debian:
```bash
sudo apt update
sudo apt install libgtk-4-dev build-essential
```

#### Install GTK4 on Fedora:
```bash
sudo dnf install gtk4-devel gcc
```

#### Install GTK4 on Arch:
```bash
sudo pacman -S gtk4 base-devel
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
./install-desktop.sh
```

### First Run Setup

1. **Enter your TMDB API key** when prompted
2. **Add scan directories** in Settings (⚙️)
3. **Scan your movie collection** (📁 Scan Directory)
4. **Enjoy!** 🎬

## 📖 Usage Guide

### Adding Movies

#### Automatic Scanning (Recommended)
1. Click **📁 Scan Directory**
2. Select your movie folder
3. Wait for parallel metadata fetch
4. Movies appear with full metadata!

**Supported formats:** MP4, MKV, AVI, MOV, WMV, FLV, WEBM, M4V

#### Manual Addition
1. Click **➕ Add Movie**
2. Search for title
3. Select from results
4. Add to database

### Searching Movies

1. Type movie title in search box
2. **Press Enter** to search
3. Combine with genre filter and sort
4. Click movie to see details

**Pro tip:** Use genre + sort for browsing (e.g., "Horror" + "Rating High-Low")

### Fixing Wrong Metadata

Got the 2011 remake instead of the 1982 original?

1. Select the movie
2. Click **🎞️ Wrong Movie?**
3. See all TMDB versions with years and ratings
4. Select the correct one
5. Metadata updates!

### Viewing Statistics

1. Click **📊 Statistics**
2. See overview, top 100, genres, decades
3. Analyze your collection!

### Playing Movies

1. Select a movie
2. Click **▶ Play**
3. Opens in VLC (or default player)

### Viewing Cast

1. Select a movie
2. Click **⭐ Show Cast**
3. See actor photos with character names
4. Scroll through full cast list

## ⚙️ Configuration

### Settings Dialog

Access via **⚙️ Settings** button:

- **TMDB API Key** - Your API key
- **Scan Directories** - Folders to auto-scan
- **Auto-scan on startup** - Check for new movies on launch

### Files & Locations

```
~/.config/movie-database/
├── config.json          # Settings
└── movies.db            # Movie database

~/.local/share/movie-database/
└── posters/             # Cached poster images
    ├── 278.jpg          # Shawshank Redemption
    ├── 155.jpg          # Dark Knight
    └── ...
```

## 🎯 Keyboard Shortcuts

| Action | Shortcut |
|--------|----------|
| Search | Type + **Enter** |
| Refresh Metadata | Click 🔄 |
| Settings | Click ⚙️ |
| Statistics | Click 📊 |

## 🛠️ Technical Details

### Built With

- **Rust** - Fast, safe systems programming
- **GTK4** - Modern, beautiful UI toolkit
- **TMDB API** - Comprehensive movie database
- **Tokio** - Async runtime for parallel operations
- **Serde** - JSON serialization

### Architecture

```
┌─────────────────────────────────────────┐
│           GTK4 User Interface           │
├─────────────────────────────────────────┤
│        Movie Database (HashMap)         │
├─────────────────────────────────────────┤
│   TMDB API Client (Async + Parallel)   │
├─────────────────────────────────────────┤
│  Local Storage (JSON + Cached Images)  │
└─────────────────────────────────────────┘
```

### Performance

- **Parallel scanning**: 10 movies at once
- **Smart caching**: Posters stored locally
- **Instant search**: HashMap-based lookup (O(1))
- **Fast sorting**: Efficient in-memory operations
- **Duplicate detection**: Skips existing movies on rescan

**Benchmark (100 movies):**
- First scan: ~30 seconds
- Rescan (no new movies): < 2 seconds
- Search: Instant
- Sort: < 5ms

## 🤝 Contributing

Contributions welcome! Here are some ideas:

### Feature Ideas
- [ ] Export to CSV/Excel
- [ ] Custom collections/playlists
- [ ] Watched/unwatched tracking
- [ ] Personal ratings
- [ ] Dark mode theme
- [ ] Import from other databases
- [ ] Backup/restore functionality
- [ ] Advanced search (by actor, director, year range)

### How to Contribute

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Commit changes: `git commit -m 'Add amazing feature'`
4. Push to branch: `git push origin feature/amazing-feature`
5. Open a Pull Request

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- **TMDB** - For the excellent free API
- **GTK Team** - For the beautiful UI toolkit
- **Rust Community** - For the amazing language and ecosystem
- **Classic film lovers** - Who inspired the "Wrong Movie?" feature

## 🐛 Troubleshooting

### "No metadata found"
- Check your TMDB API key in Settings
- Verify internet connection
- Check TMDB API status: https://status.themoviedb.org/

### "Movies not appearing"
- Ensure files are in supported formats (MP4, MKV, AVI, etc.)
- Check file permissions
- Look for error messages in terminal

### "UI freezes during scan"
- This is fixed in the latest version!
- Update to get parallel scanning

### "Wrong movie metadata"
- Use the **🎞️ Wrong Movie?** button
- Search for correct version by year
- Select and apply

### "Posters not loading"
- Check internet connection
- Verify poster directory permissions
- Try refreshing metadata

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/Zeteticus/marks-movie-database/issues)
- **Email**: ascensus1125@gmail.com
- **Discussions**: [GitHub Discussions](https://github.com/Zeteticus/marks-movie-database/discussions)

## 🗺️ Roadmap

### Version 0.2.0 (Planned)
- [ ] Watched/unwatched tracking
- [ ] Personal ratings overlay
- [ ] Custom collections
- [ ] Dark mode

### Version 0.3.0 (Future)
- [ ] Export functionality
- [ ] Advanced search filters
- [ ] Backup/restore
- [ ] Multi-language support

## 📊 Project Stats

- **Lines of Code**: ~2,500
- **Dependencies**: 16
- **Supported Formats**: 8 video formats
- **API Integrations**: TMDB (+ IMDb references)
- **Database Size**: ~1KB per movie (metadata only)

---

*For movie collectors who appreciate quality metadata and beautiful organization.*

🎬 Happy collecting! 📽️
