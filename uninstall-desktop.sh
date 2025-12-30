#!/bin/bash
# Uninstallation script for Movie Database desktop integration

set -e

echo "Uninstalling Movie Database desktop integration..."

# Colors
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

# Remove desktop entry
echo -e "${BLUE}Removing desktop entry...${NC}"
rm -f ~/.local/share/applications/movie-database.desktop
rm -f ~/Desktop/movie-database.desktop

# Remove icons
echo -e "${BLUE}Removing icons...${NC}"
rm -f ~/.local/share/icons/hicolor/scalable/apps/movie-database.svg
rm -f ~/.local/share/icons/hicolor/128x128/apps/movie-database.png

# Update caches
echo -e "${BLUE}Updating caches...${NC}"
if command -v gtk-update-icon-cache &> /dev/null; then
    gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor 2>/dev/null || true
fi

if command -v update-desktop-database &> /dev/null; then
    update-desktop-database ~/.local/share/applications 2>/dev/null || true
fi

echo ""
echo -e "${RED}✓ Uninstallation complete!${NC}"
echo "Movie Database has been removed from your application menu."
