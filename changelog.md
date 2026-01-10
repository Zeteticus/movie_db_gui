# My MovieDB - Version 1.0.1 Changelog

**Release Date:** January 6, 2026

## Changes in v1.0.1

### 🎨 Branding Update
- **Renamed Application**: "Mark's Movie Database (MMDB)" → "My MovieDB"
  - Window title updated
  - Header title updated
  - About dialog updated
  - All documentation updated

### 📝 Version Updates
- Version bumped to 1.0.1
- Cargo.toml updated
- README updated with new name
- About dialog shows v1.0.1

## No Functional Changes

This is a purely cosmetic release focused on branding consistency. All features from v1.0.0 remain unchanged:

- ✅ TMDB integration with cast photos
- ✅ Dual search (title/cast)
- ✅ Performance optimizations
- ✅ Grid/list views
- ✅ Keyboard shortcuts
- ✅ Statistics dashboard
- ✅ Year filtering
- ✅ Watch history tracking

## Upgrade Instructions

Simply replace your existing binary with the new v1.0.1 build. Your database and settings are fully compatible.

```bash
cargo build --release
cp target/release/movie-database ~/my-moviedb-v1.0.1
```

---

**Previous Release:** v1.0.0 (First stable release)
**Current Release:** v1.0.1 (Branding update)

# My MovieDB - Version 1.0.2 Changelog

**Release Date:** January 6, 2026

## Changes in v1.0.2

### ⚡ Improved Scanning Workflow
- **Auto-Scan Configured Directories**: Scan Directory button now automatically scans all configured directories from Settings
  - No more file picker dialog
  - One-click scanning of all your movie folders
  - Scans multiple directories in sequence
  - Shows progress for each directory
  - Skips invalid/missing directories with warnings

### 📋 User Experience Improvements
- **Helpful Error Messages**:
  - "⚠️ No settings found. Please configure scan directories in Settings first."
  - "⚠️ No scan directories configured. Please add directories in Settings."
  - "⚠️ Skipping invalid directory: /path/to/missing"
- **Better Status Updates**:
  - Shows count of directories being scanned
  - Progress updates for each directory
  - Clear completion message

### 🔧 Workflow Changes

**Before (v1.0.1):**
1. Click "📁 Scan Directory"
2. File picker opens
3. Select ONE folder
4. Scan that folder

**Now (v1.0.2):**
1. Configure directories in ⚙️ Settings (one-time setup)
2. Click "📁 Scan Directory"
3. ALL configured folders scan automatically!

### Benefits
- ✅ Faster workflow - no dialog delays
- ✅ Scan multiple directories with one click
- ✅ Consistent behavior - always scans your configured folders
- ✅ Better for regular use - configure once, scan forever

## Upgrade Instructions

Simply replace your existing binary with the new v1.0.2 build. Your database and settings are fully compatible.

```bash
cargo build --release
cp target/release/movie-database ~/my-moviedb-v1.0.2
```

**Note:** Make sure you have directories configured in Settings before using the Scan Directory button.

---

**Previous Release:** v1.0.1 (Branding update)
**Current Release:** v1.0.2 (Improved scanning workflow)
