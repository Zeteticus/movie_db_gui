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
