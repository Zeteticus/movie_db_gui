# movie_db_gui
Movie Database
# New Features: Posters & VLC Integration

## Overview

Your movie database now includes two major enhancements:

1. **Movie Poster Display** - Visual thumbnails and full-size posters
2. **VLC Integration** - One-click playback of your movies

## What's New

### 1. Movie Posters

**List View Thumbnails**
- Each movie in the list now shows a 60x90px poster thumbnail
- Missing posters show a 🎬 emoji placeholder

**Details Panel**
- Large 200x300px poster displayed when you select a movie
- Posters are automatically downloaded from TMDB
- Cached locally in the `posters/` directory

**How It Works:**
```
Movie Selection → Download from TMDB → Cache in posters/ → Display in UI
```

The poster images are:
- Downloaded once and cached permanently
- Stored as `posters/poster_{tmdb_id}.jpg`
- Automatically loaded when the app starts
- Not re-downloaded unless you refresh metadata

### 2. VLC Playback

**Play Button**
- New "▶️ Play in VLC" button in the details panel
- Click to instantly open the movie in VLC
- Works with both system-installed and Flatpak VLC

**Status Updates**
- Shows "Playing: {Movie Title}" when launched successfully
- Shows error message if VLC isn't found
- Warns if no video file is associated with the movie

**Compatibility:**
The app tries multiple VLC launch methods:
1. System VLC: `vlc /path/to/movie.mp4`
2. Flatpak VLC: `flatpak run org.videolan.VLC /path/to/movie.mp4`

## Installation Requirements

### VLC Media Player

**Ubuntu/Mint (APT)**:
```bash
sudo apt install vlc
```

**Flatpak** (if you prefer):
```bash
flatpak install flathub org.videolan.VLC
```

**Check if VLC is installed:**
```bash
which vlc
# or
flatpak list | grep VLC
```

### Build Dependencies

The app now requires `gdk-pixbuf` for image handling:

```bash
# Already installed with GTK4, but if you get errors:
sudo apt install libgdk-pixbuf2.0-dev
```

## Using the New Features

### Viewing Posters

1. **Start the app**:
   ```bash
   cargo run --release
   ```

2. **Scan or add movies** - posters download automatically

3. **Browse your collection** - thumbnails appear in the list

4. **Click a movie** - see the full-size poster in the details panel

### Playing Movies

1. **Select a movie** from the list

2. **Click "▶️ Play in VLC"** button

3. **VLC launches** and starts playing immediately

**Keyboard Shortcut** (Future Enhancement):
You could add a keybinding like `Space` to play the selected movie.

## File Structure

After using the app, you'll have:

```
~/movie_db_gui/
├── Cargo.toml
├── src/
│   └── main.rs
├── movies.db              # Movie metadata
├── posters/               # ← New! Downloaded posters
│   ├── poster_550.jpg     # Fight Club
│   ├── poster_13.jpg      # Forrest Gump
│   └── ...
└── target/
    └── release/
        └── movie-database
```

## Technical Details

### Poster Caching Strategy

**Why cache?**
- Avoids re-downloading images every time
- Works offline once downloaded
- Reduces TMDB API load
- Faster UI performance

**Cache invalidation:**
Posters are cached permanently. To refresh:
1. Delete the `posters/` directory
2. Click "🔄 Refresh Metadata" for each movie
3. Posters will be re-downloaded

### VLC Launch Logic

```rust
fn play_movie(file_path: &str) {
    // Try system VLC first
    if Command::new("vlc").arg(file_path).spawn().is_ok() {
        return;
    }
    
    // Fall back to Flatpak
    if Command::new("flatpak")
        .args(["run", "org.videolan.VLC", file_path])
        .spawn()
        .is_ok()
    {
        return;
    }
    
    // Show error if neither works
    show_error("VLC not found");
}
```

### Image Handling

**Download**:
- Uses `reqwest::blocking::get()` to fetch images
- Saves as JPEG in `posters/` directory
- Uses TMDB ID for unique filenames

**Display**:
- `Pixbuf::from_file_at_scale()` for thumbnails (60x90)
- `Pixbuf::from_file_at_scale()` for details (200x300)
- Automatic aspect ratio preservation

**Format Support**:
GTK's Pixbuf supports:
- JPEG (from TMDB)
- PNG
- GIF
- BMP
- TIFF

## Memory Considerations

**Poster Cache Size:**
- Average poster: ~50-100 KB
- 100 movies: ~5-10 MB
- 1000 movies: ~50-100 MB

This is negligible for modern systems.

**Runtime Memory:**
- Only visible posters are loaded into memory
- Pixbufs are automatically freed when not displayed
- List thumbnails are small (60x90) = minimal memory

## Troubleshooting

### "VLC not found" Error

**Check VLC installation:**
```bash
vlc --version
```

**Install VLC:**
```bash
sudo apt install vlc
```

**Test VLC from command line:**
```bash
vlc /path/to/your/movie.mp4
```

### Posters Not Displaying

**Check poster directory:**
```bash
ls -lh posters/
```

**Check file permissions:**
```bash
chmod 755 posters/
chmod 644 posters/*.jpg
```

**Re-download posters:**
1. Delete `posters/` directory
2. Refresh metadata for each movie

### Build Errors

**Missing gdk-pixbuf:**
```bash
sudo apt install libgdk-pixbuf2.0-dev
```

**Outdated GTK:**
```bash
sudo apt install libgtk-4-dev
```

## Future Enhancement Ideas

### Additional Features You Could Add

1. **Poster Gallery View**
   - Grid layout showing only posters
   - Click to see details
   - Like Netflix/Plex interface

2. **Custom Posters**
   - Right-click → "Set Custom Poster"
   - Use your own images

3. **Backdrop Images**
   - Show movie backdrops in details panel
   - Cinematic full-width display

4. **Video Preview**
   - Embed video thumbnails
   - GIF previews from trailers

5. **Keyboard Shortcuts**
   - `Space` = Play
   - `Delete` = Delete movie
   - `F5` = Refresh metadata

6. **Multiple Players**
   - Support MPV, Kodi, etc.
   - User-configurable in settings

7. **Playlist Mode**
   - Select multiple movies
   - Play in sequence

8. **Watch History**
   - Track when you played each movie
   - "Resume playback" feature

## Philosophical Reflection

The addition of posters transforms the application from a *database* (abstract, textual) to a *collection* (concrete, visual). This shift mirrors how we actually experience cinema - not as metadata, but as images, faces, compositions.

The VLC integration bridges another ontological gap: from *information about* films to the films *themselves*. With one click, you move from the map (the database entry) to the territory (the actual movie).

This is the phenomenology of media libraries: they're not just storage systems, but interfaces between our desire to watch and the act of watching. The poster serves as what Heidegger might call *ready-to-hand* - it's not an object to contemplate, but an invitation to action.

## Performance Notes

**Poster Downloads:**
- Happen in background threads
- Don't block the UI
- Take ~100-500ms per poster

**VLC Launch:**
- Spawns VLC as separate process
- Doesn't block the app
- VLC manages video playback

**Image Scaling:**
- Done once at load time
- Cached in memory while visible
- GPU-accelerated by GTK

## Conclusion

Your movie database is now visually rich and functionally complete. You can:
- Browse your collection with beautiful posters
- Play any movie with one click
- Enjoy a Netflix-like browsing experience

The combination of visual appeal (posters) and functional power (VLC integration) creates an application that's both pleasant to use and genuinely useful.

Enjoy your enhanced movie collection!
