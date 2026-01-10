# My MovieDB v1.0.2

A powerful GTK4-based movie collection manager with TMDB integration.

![Version](https://img.shields.io/badge/version-1.0.2-blue)
![Platform](https://img.shields.io/badge/platform-Linux-lightgrey)

## Features

### 🎬 Core Features
- **TMDB Integration** - Automatic metadata fetching from The Movie Database
- **HD Poster Downloads** - High-quality movie posters with automatic caching
- **Cast Photos** - View cast members with their photos in detail dialogs
- **Watch History** - Automatic logging when playing movies
- **SEEN Badge** - Visual indicator for watched movies

### 🔍 Search & Filter
- **Dual Search** - Search by movie title OR cast member name
- **Genre Filtering** - Filter by Action, Drama, Comedy, etc.
- **Multiple Sort Options** - By title, year, rating, or date added
- **Instant Results** - Cached results for fast switching

### 🖼️ Views
- **List View** - Compact view with thumbnails
- **Grid View** - Beautiful poster grid layout
- **Easy Toggle** - Switch views with one click

### 🎯 User Interface
- **Right-Click Menu**:
  - Play in VLC (auto-logs watch history)
  - View Details (cast photos, full info)
  - Delete Movie Metadata
- **Keyboard Shortcuts**:
  - `Ctrl+F` - Focus search
  - `Space` - Play selected movie
  - `Delete` - Delete movie metadata
  - `Escape` - Close dialogs

### ⚡ Performance
- **100x Memory Reduction** - Efficient thumbnail-only caching
- **Instant VLC Launch** - Async database operations
- **Batched Loading** - Smooth UI even with large collections
- **Result Caching** - Instant filter/sort switching

## Requirements

- **Operating System**: Linux (Ubuntu, Fedora, etc.)
- **GTK4**: Version 4.10 or higher
- **VLC Media Player**: For playing movies
- **TMDB API Key**: Free from https://www.themoviedb.org/settings/api

## Installation

### From Source

1. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Install GTK4 dependencies**:
   
   **Ubuntu/Debian:**
   ```bash
   sudo apt install libgtk-4-dev build-essential
   ```
   
   **Fedora:**
   ```bash
   sudo dnf install gtk4-devel gcc
   ```
   
   **Arch:**
   ```bash
   sudo pacman -S gtk4 base-devel
   ```

3. **Install VLC**:
   ```bash
   sudo apt install vlc        # Ubuntu/Debian
   sudo dnf install vlc        # Fedora
   sudo pacman -S vlc          # Arch
   ```

4. **Clone and build**:
   ```bash
   cd movie_db_gui
   cargo build --release
   ```

5. **Run**:
   ```bash
   cargo run --release
   ```

## Quick Start

1. Launch MMDB - enter your TMDB API key when prompted
2. Click **"📁 Scan Directory"** to scan your movie folder
3. Browse, search, and enjoy your collection!

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+F` | Focus search box |
| `Space` | Play selected movie |
| `Delete` | Delete movie metadata |
| `Escape` | Close dialogs |

## File Locations

```
~/.movie_database/
├── movies.db           # Movie database (JSON)
├── posters/            # Movie posters
└── cast_photos/        # Cast member photos
```

## Version 1.0.2 Release Notes

Latest release with improved scanning workflow:
- ⚡ Auto-scan configured directories (no more file picker!)
- 🎨 Renamed to "My MovieDB"
- ✨ TMDB integration with cast photos
- 🔍 Dual search (title/cast)
- ⚡ Performance optimizations
- 🎨 Grid/list views
- ⌨️ Keyboard shortcuts
- 📊 Statistics dashboard
- 🎯 Year filtering
- 📝 Watch history tracking

## Credits

- **TMDB**: Movie metadata and images
- **GTK4**: UI framework
- **VLC**: Media playback

---

**Made with ❤️ for movie collectors**
