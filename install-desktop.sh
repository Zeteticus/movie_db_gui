#!/bin/bash
# Installation script for Movie Database desktop integration

set -e

echo "Installing Movie Database desktop integration..."

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get the script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Create necessary directories
echo -e "${BLUE}Creating directories...${NC}"
mkdir -p ~/.local/share/applications
mkdir -p ~/.local/share/icons/hicolor/scalable/apps
mkdir -p ~/.local/share/icons/hicolor/128x128/apps

# Install icon
echo -e "${BLUE}Installing icon...${NC}"
cp "${SCRIPT_DIR}/icons/movie-database.svg" ~/.local/share/icons/hicolor/scalable/apps/

# Convert SVG to PNG for better compatibility (if inkscape is available)
if command -v inkscape &> /dev/null; then
    echo -e "${BLUE}Converting icon to PNG (128x128)...${NC}"
    inkscape "${SCRIPT_DIR}/icons/movie-database.svg" \
        --export-type=png \
        --export-filename=~/.local/share/icons/hicolor/128x128/apps/movie-database.png \
        -w 128 -h 128 &> /dev/null || {
        echo "Warning: PNG conversion failed, using SVG only"
    }
elif command -v convert &> /dev/null; then
    echo -e "${BLUE}Converting icon to PNG (128x128) using ImageMagick...${NC}"
    convert -background none \
        "${SCRIPT_DIR}/icons/movie-database.svg" \
        -resize 128x128 \
        ~/.local/share/icons/hicolor/128x128/apps/movie-database.png || {
        echo "Warning: PNG conversion failed, using SVG only"
    }
else
    echo "Note: Install 'inkscape' or 'imagemagick' for PNG icon conversion"
fi

# Install desktop entry
echo -e "${BLUE}Installing desktop entry...${NC}"

# Update the Exec path to the actual location
EXEC_PATH="${SCRIPT_DIR}/target/release/movie-database"
sed "s|Exec=.*|Exec=${EXEC_PATH}|" "${SCRIPT_DIR}/movie-database.desktop" \
    > ~/.local/share/applications/movie-database.desktop

# Make desktop file executable
chmod +x ~/.local/share/applications/movie-database.desktop

# Update icon cache
echo -e "${BLUE}Updating icon cache...${NC}"
if command -v gtk-update-icon-cache &> /dev/null; then
    gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor || true
fi

# Update desktop database
echo -e "${BLUE}Updating desktop database...${NC}"
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database ~/.local/share/applications || true
fi

echo ""
echo -e "${GREEN}✓ Installation complete!${NC}"
echo ""
echo "The Movie Database app should now appear in your application menu."
echo "You can also:"
echo "  • Add it to favorites/taskbar by right-clicking the icon"
echo "  • Create a desktop shortcut by copying:"
echo "    cp ~/.local/share/applications/movie-database.desktop ~/Desktop/"
echo "    chmod +x ~/Desktop/movie-database.desktop"
echo ""
echo "To uninstall, run: ./uninstall-desktop.sh"
