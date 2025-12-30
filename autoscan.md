# Auto-Scan on Startup

## Overview

Your movie database now automatically scans configured directories for new movies every time you launch the app!

## How It Works

### Setup (One Time)

1. **Launch the app**
2. **Click ⚙️ Settings**
3. **Add directories** to scan:
   - Click "➕ Add Directory"
   - Select your movie folder (e.g., `/home/ascensus/Movies`)
   - Repeat for each folder you want to monitor
4. **Enable auto-scan** (checkbox at bottom)
5. **Click Save**

### Every Subsequent Launch

1. **App opens**
2. **Automatically scans** all configured directories
3. **Finds new video files**
4. **Fetches metadata** from TMDB
5. **Downloads posters**
6. **Adds to database**
7. **Updates the list** in real-time

You see progress in the status bar:
```
Auto-scanning configured directories...
Scanning: /home/ascensus/Movies
Checking: Inception
✓ Found: Inception
Auto-scan complete! Added 3 new movies
```

## Configuration

### Config File Structure

Your config now includes scan directories:

```json
{
  "tmdb_api_key": "your_key_here",
  "scan_directories": [
    "/home/ascensus/Movies",
    "/media/external/Films",
    "/mnt/nas/Cinema"
  ],
  "auto_scan_on_startup": true
}
```

### Settings Dialog

The enhanced **⚙️ Settings** dialog now has three sections:

#### 1. TMDB API Key
- View/change your API key
- Hidden for security

#### 2. Scan Directories
- **List of directories** currently being monitored
- **Add Directory** button - browse and add new folders
- **Remove** button - remove folders you no longer want scanned
- Changes are saved immediately

#### 3. Auto-Scan on Startup
- **Checkbox** to enable/disable auto-scan
- Enabled by default
- If disabled, you can still manually scan with "📁 Scan Directory"

## Use Cases

### Home Media Server
```
Directories:
- /home/user/Movies
- /media/external/TV-Shows
- /mnt/nas/Documentaries

Result: Every new file in these folders is automatically cataloged
```

### Multiple Storage Locations
```
Directories:
- /home/user/Downloads/Movies
- /media/ssd/Cinema
- /media/hdd/Archive

Result: All your movie locations in one database
```

### Shared Network Drives
```
Directories:
- /mnt/smb-share/Family-Movies
- /mnt/nfs/Media-Server

Result: Network storage automatically indexed
```

## Behavior Details

### What Gets Scanned

**File types recognized:**
- MP4, MKV, AVI, MOV
- WMV, FLV, WebM, M4V

**What's checked:**
- Only video files in the configured directories
- **Not recursive** - subdirectories are not scanned
- Files are compared against existing database entries

### Duplicate Detection

**Smart duplicate handling:**
- Checks file paths against existing database
- If file already exists, it's skipped
- Same movie in different locations = separate entries

**Example:**
```
/home/user/Movies/Inception.mkv      ← Added
/media/external/Inception.mkv        ← Added (different path)
/home/user/Movies/Inception.mkv      ← Skipped (already exists)
```

### Performance Considerations

**Initial scan:**
- Can take a while with many files
- ~2 seconds per movie (TMDB API calls)
- 100 movies = ~3-4 minutes

**Subsequent launches:**
- Only new files are processed
- If no new files, scan completes instantly

**Background operation:**
- Scanning happens in a background thread
- UI remains responsive
- You can browse while scanning

## Disabling Auto-Scan

If you don't want auto-scan:

### Option 1: Checkbox
1. Open Settings
2. Uncheck "Automatically scan directories on startup"
3. Save

### Option 2: Edit Config
```bash
nano ~/.config/movie-database/config.json
# Change: "auto_scan_on_startup": false
```

### Option 3: Remove Directories
1. Open Settings
2. Click "Remove" next to each directory
3. Save

## Workflow Examples

### Scenario 1: Download New Movie
```
1. Download movie to ~/Movies/
2. Close torrent client
3. [Later] Launch movie database
4. Auto-scan finds the new file
5. Metadata fetched automatically
6. Movie appears in list
```

### Scenario 2: Adding to Multiple Locations
```
1. Copy movies to external drive
2. Mount drive at /media/external
3. Open Settings → Add Directory → /media/external
4. Save
5. Close and reopen app
6. All movies from external drive are added
```

### Scenario 3: Network Share
```
1. Mount network share: /mnt/nas/Movies
2. Add to scan directories
3. Launch app anytime
4. Network movies automatically indexed
```

## Status Messages

During auto-scan, you'll see:

```
Auto-scanning configured directories...
├─ Scanning: /home/user/Movies
├─ Checking: The Matrix
├─ ✓ Found: The Matrix
├─ Checking: Blade Runner
├─ ✓ Found: Blade Runner
└─ Auto-scan complete! Added 2 new movies
```

If no new movies:
```
Auto-scan complete - no new movies found
```

## Troubleshooting

### "Auto-scan not working"

**Check config:**
```bash
cat ~/.config/movie-database/config.json
# Verify scan_directories is not empty
# Verify auto_scan_on_startup is true
```

**Check directory permissions:**
```bash
ls -la /path/to/movie/directory
# Should be readable by your user
```

**Check for typos:**
```bash
# Make sure paths exist
cd /path/from/config
```

### "Some movies not found"

**Possible causes:**
1. Filename doesn't match TMDB database
2. Movie not in TMDB (very obscure/foreign films)
3. TMDB API temporarily unavailable

**Solutions:**
- Rename file to match TMDB title
- Use "➕ Add Movie" to manually search
- Check TMDB website to verify movie exists

### "Scan takes too long"

**For large collections:**
```
100 movies × 2 seconds = 3-4 minutes
500 movies × 2 seconds = 15-20 minutes (first time)
```

**Tips:**
- Be patient on first scan
- Subsequent scans only check new files
- Consider disabling auto-scan and using manual scan
- Split into smaller directories

### "Directories not being scanned"

**Common issues:**
```bash
# Wrong path - use absolute paths
❌ ~/Movies
✅ /home/ascensus/Movies

# Directory doesn't exist
ls /path/to/directory

# No read permissions
chmod +r /path/to/directory
```

## Advanced: Subdirectory Scanning

Currently, the app **does not** scan subdirectories. If you have:

```
Movies/
├── Action/
├── Comedy/
└── Drama/
```

You need to add each subdirectory separately, or move files to the parent directory.

**Future enhancement**: Recursive scanning could be added with a checkbox:
```
☑ Scan subdirectories recursively
```

## Performance Optimization

### Rate Limiting

TMDB API has rate limits:
- ~40 requests per 10 seconds
- Auto-scan respects these limits
- Scanning pauses if limit reached

### Caching

Once a movie is in the database:
- ✅ Metadata cached
- ✅ Poster cached
- ✅ No re-download needed
- Only new files trigger API calls

### Memory Usage

During scanning:
- Moderate memory usage (~50-100MB)
- Posters stored on disk, not in RAM
- Database kept in memory for speed

## Integration with Manual Scan

You can still use manual scan:

**Auto-scan:**
- Runs on startup
- Scans all configured directories
- Automatic and convenient

**Manual scan (📁 button):**
- Runs on demand
- Choose any directory
- One-time operation
- Useful for external drives, temporary locations

**Both methods:**
- Check for duplicates
- Fetch metadata
- Download posters
- Update database

## Command Line Testing

Want to see auto-scan in action?

```bash
# Add a movie file
cp movie.mp4 ~/Movies/

# Launch app
cargo run --release

# Watch terminal for:
# "Loaded API key from config"
# "Auto-scanning configured directories..."
# "✓ Found: Movie Name"
```

## Configuration Tips

### Organizing Your Movies

**Option 1: Single directory**
```
/home/user/Movies/
├── movie1.mkv
├── movie2.mp4
└── movie3.avi
```
Add: `/home/user/Movies`

**Option 2: Multiple locations**
```
/home/user/Movies/       ← Local storage
/media/external/Films/   ← External drive
/mnt/nas/Cinema/         ← Network share
```
Add all three

**Option 3: By genre (requires multiple adds)**
```
/home/user/Movies/Action/
/home/user/Movies/Comedy/
/home/user/Movies/Drama/
```
Add each subdirectory

## Future Enhancements

Potential improvements:

1. **Recursive scanning** - Scan subdirectories automatically
2. **Watch mode** - Monitor directories for changes in real-time
3. **Scheduled scanning** - Auto-scan at specific times
4. **Exclude patterns** - Ignore certain files/folders
5. **Scan progress bar** - Visual progress indicator
6. **Scan history** - Log of what was added when
7. **Dry run mode** - Preview what would be added

## Philosophical Note

Auto-scan represents a shift from **active management** to **ambient awareness**. Instead of explicitly telling the app "scan this folder now," you declare **intentions** (these are my movie folders) and **policies** (scan on startup). The system then operates according to these rules, requiring minimal ongoing attention.

This is an example of **declarative programming** applied to UI: you specify *what* you want (movies from these folders), not *how* to achieve it (manually scanning each time). The app infers the appropriate actions from your configuration.

The duplicate detection embodies **idempotence**: scanning the same directory multiple times produces the same result as scanning once. This makes the system robust and predictable—you can re-scan without fear of creating duplicates.

## Conclusion

Your movie database now:
- ✅ Automatically discovers new movies
- ✅ Scans configured directories on startup
- ✅ Updates in real-time as new files appear
- ✅ Requires zero ongoing maintenance
- ✅ Configurable via Settings dialog

Set it up once, then forget about it! Your movie collection stays up-to-date automatically.
