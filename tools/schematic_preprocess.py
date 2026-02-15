#!/usr/bin/env python3
"""Schematic image preprocessing for Claude's vision pipeline.

Claude downsamples all images to max 1568px on the long edge (~1.15 MP).
Rendering at high DPI on large areas wastes resolution — the extra pixels
are silently discarded. This tool renders small targeted crops at moderate
DPI and preprocesses them for optimal AI vision readability.

Pipeline: RGB -> Grayscale -> Denoise -> CLAHE -> Unsharp Mask -> Border Crop -> Resize

Usage:
    # Render a named region from the Wurlitzer schematic PDF
    python tools/schematic_preprocess.py render --pdf docs/verified_wurlitzer_200A_series_schematic.pdf --region preamp

    # Render a custom rectangle (normalized 0-1 coordinates)
    python tools/schematic_preprocess.py render --pdf docs/verified_wurlitzer_200A_series_schematic.pdf \
        --rect 0.05,0.3,0.45,0.7 --dpi 600

    # Enhance an existing PNG
    python tools/schematic_preprocess.py enhance input.png --output enhanced.png

    # Generate overlapping tiles from a large image
    python tools/schematic_preprocess.py tile input.png --tile-size 1400 --overlap 200
"""

import argparse
import sys
from pathlib import Path

import cv2
import numpy as np

# Claude vision constraints
MAX_LONG_EDGE = 1500  # Leave headroom below 1568px hard limit
MAX_PIXELS = 1_150_000  # 1.15 MP max

# Enhancement defaults
CLAHE_CLIP_LIMIT = 2.5
CLAHE_TILE_GRID = (8, 8)
UNSHARP_SIGMA = 1.0
UNSHARP_STRENGTH = 1.5
DENOISE_H = 8
DENOISE_TEMPLATE_WINDOW = 7
DENOISE_SEARCH_WINDOW = 21
BORDER_THRESHOLD = 240
BORDER_MARGIN = 20

# Default output directory
DEFAULT_OUTPUT_DIR = Path("schematic_tiles")

# Named regions of the verified Wurlitzer 200A schematic (page 0)
# PDF: docs/verified_wurlitzer_200A_series_schematic.pdf
# Page 0 is 17"x11" landscape (1224x792 pts). Page 1 is board assembly (not used).
# Coordinates are normalized (0-1) relative to full page: (x0, y0, x1, y1)
# Calibrated Feb 2026 against the verified 200A schematic.
NAMED_REGIONS = {
    "overview": {
        "rect": (0.0, 0.0, 1.0, 1.0),
        "dpi": 150,
        "description": "Full schematic overview (low DPI for topology)",
    },
    "preamp": {
        "rect": (0.01, 0.01, 0.40, 0.32),
        "dpi": 600,
        "description": "Full preamp circuit (TR-1, TR-2, R-10 feedback, cable connections)",
    },
    "preamp-detail": {
        "rect": (0.01, 0.04, 0.22, 0.30),
        "dpi": 900,
        "description": "Preamp Stage 1 detail (TR-1, R-1, R-2, R-3, D-1, Ce1, C-3)",
    },
    "preamp-output": {
        "rect": (0.18, 0.02, 0.42, 0.30),
        "dpi": 900,
        "description": "Preamp Stage 2 and output (TR-2, R-9, R-10, C-4, volume pot)",
    },
    "feedback-network": {
        "rect": (0.05, 0.04, 0.30, 0.26),
        "dpi": 900,
        "description": "R-10 feedback path detail (R-10, Ce1, Re1, emitter junction)",
    },
    "cable-routing": {
        "rect": (0.24, 0.18, 0.46, 0.40),
        "dpi": 600,
        "description": "Cable pin assignments and model notes (Grey/Brown jackets)",
    },
    "power-amp": {
        "rect": (0.36, 0.01, 0.76, 0.50),
        "dpi": 600,
        "description": "Power amplifier (TR-7/8 diff pair through TR-11/13 output, earphone jack)",
    },
    "tremolo": {
        "rect": (0.01, 0.30, 0.32, 0.58),
        "dpi": 600,
        "description": "Tremolo oscillator (TR-3, TR-4, LG-1 LED/LDR, 200A section only)",
    },
    "power-supply": {
        "rect": (0.36, 0.45, 0.74, 0.74),
        "dpi": 600,
        "description": "Power supply — LV regulator (IC-1) and HV filter chain to Pin 18",
    },
    "speaker-load": {
        "rect": (0.68, 0.0, 0.98, 0.38),
        "dpi": 600,
        "description": "Speaker load configurations (200A, 206A/207, 202172, 201756)",
    },
}


def enhance_image(img_gray: np.ndarray) -> np.ndarray:
    """Apply the full enhancement pipeline to a grayscale image.

    Pipeline: Denoise -> CLAHE -> Unsharp Mask
    """
    # Non-local means denoising
    denoised = cv2.fastNlMeansDenoising(
        img_gray,
        h=DENOISE_H,
        templateWindowSize=DENOISE_TEMPLATE_WINDOW,
        searchWindowSize=DENOISE_SEARCH_WINDOW,
    )

    # CLAHE contrast enhancement
    clahe = cv2.createCLAHE(clipLimit=CLAHE_CLIP_LIMIT, tileGridSize=CLAHE_TILE_GRID)
    enhanced = clahe.apply(denoised)

    # Unsharp mask sharpening
    blurred = cv2.GaussianBlur(enhanced, (0, 0), UNSHARP_SIGMA)
    sharpened = cv2.addWeighted(enhanced, 1.0 + UNSHARP_STRENGTH, blurred, -UNSHARP_STRENGTH, 0)

    return sharpened


def crop_white_borders(img: np.ndarray, threshold: int = BORDER_THRESHOLD,
                       margin: int = BORDER_MARGIN) -> np.ndarray:
    """Remove white borders from a grayscale image, keeping a small margin."""
    # Find non-white pixels
    mask = img < threshold
    if not mask.any():
        return img

    rows = np.any(mask, axis=1)
    cols = np.any(mask, axis=0)
    rmin, rmax = np.where(rows)[0][[0, -1]]
    cmin, cmax = np.where(cols)[0][[0, -1]]

    # Add margin
    h, w = img.shape[:2]
    rmin = max(0, rmin - margin)
    rmax = min(h - 1, rmax + margin)
    cmin = max(0, cmin - margin)
    cmax = min(w - 1, cmax + margin)

    return img[rmin:rmax + 1, cmin:cmax + 1]


def resize_for_claude(img: np.ndarray) -> np.ndarray:
    """Resize image to fit within Claude's vision constraints.

    Max 1500px on long edge, max 1.15 MP total.
    Uses INTER_AREA for downsampling (best quality for schematic line art).
    """
    h, w = img.shape[:2]
    total_pixels = h * w

    # Calculate scale factor
    scale = 1.0
    long_edge = max(h, w)
    if long_edge > MAX_LONG_EDGE:
        scale = min(scale, MAX_LONG_EDGE / long_edge)
    if total_pixels > MAX_PIXELS:
        scale = min(scale, (MAX_PIXELS / total_pixels) ** 0.5)

    if scale >= 1.0:
        return img

    new_w = max(1, int(w * scale))
    new_h = max(1, int(h * scale))
    return cv2.resize(img, (new_w, new_h), interpolation=cv2.INTER_AREA)


def process_image(img: np.ndarray) -> np.ndarray:
    """Full preprocessing pipeline: grayscale -> enhance -> crop borders -> resize."""
    # Convert to grayscale if needed
    if len(img.shape) == 3:
        gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
    else:
        gray = img

    enhanced = enhance_image(gray)
    cropped = crop_white_borders(enhanced)
    resized = resize_for_claude(cropped)
    return resized


def render_from_pdf(pdf_path: str, rect: tuple[float, float, float, float],
                    dpi: int = 600, page_num: int = 0) -> np.ndarray:
    """Render a region from the PDF and return as numpy array.

    Args:
        pdf_path: Path to the PDF file
        rect: Normalized coordinates (x0, y0, x1, y1) where 0-1 maps to full page
        dpi: Render resolution
        page_num: PDF page number (0-indexed)

    Returns:
        Grayscale numpy array of the rendered region
    """
    try:
        import fitz  # PyMuPDF
    except ImportError:
        print("Error: PyMuPDF not installed. Run: pip install pymupdf", file=sys.stderr)
        sys.exit(1)

    doc = fitz.open(pdf_path)
    page = doc[page_num]
    page_rect = page.rect

    # Convert normalized coords to page coords
    x0 = page_rect.x0 + rect[0] * page_rect.width
    y0 = page_rect.y0 + rect[1] * page_rect.height
    x1 = page_rect.x0 + rect[2] * page_rect.width
    y1 = page_rect.y0 + rect[3] * page_rect.height
    clip = fitz.Rect(x0, y0, x1, y1)

    # Render at specified DPI
    mat = fitz.Matrix(dpi / 72, dpi / 72)
    pix = page.get_pixmap(matrix=mat, clip=clip)

    # Convert to numpy array
    img = np.frombuffer(pix.samples, dtype=np.uint8)
    if pix.n == 4:  # RGBA
        img = img.reshape(pix.h, pix.w, 4)
        img = cv2.cvtColor(img, cv2.COLOR_RGBA2GRAY)
    elif pix.n == 3:  # RGB
        img = img.reshape(pix.h, pix.w, 3)
        img = cv2.cvtColor(img, cv2.COLOR_RGB2GRAY)
    else:  # Already grayscale
        img = img.reshape(pix.h, pix.w)

    doc.close()
    return img


def cmd_render(args: argparse.Namespace) -> None:
    """Handle the 'render' subcommand."""
    if args.region:
        if args.region not in NAMED_REGIONS:
            print(f"Error: Unknown region '{args.region}'. Available: {', '.join(NAMED_REGIONS)}", file=sys.stderr)
            sys.exit(1)
        region = NAMED_REGIONS[args.region]
        rect = region["rect"]
        dpi = args.dpi or region["dpi"]
        default_name = f"{args.region}_{dpi}dpi.png"
    elif args.rect:
        rect = tuple(float(x) for x in args.rect.split(","))
        if len(rect) != 4:
            print("Error: --rect must be 4 comma-separated values: x0,y0,x1,y1", file=sys.stderr)
            sys.exit(1)
        dpi = args.dpi or 600
        default_name = f"custom_{dpi}dpi.png"
    else:
        print("Error: Must specify --region or --rect", file=sys.stderr)
        sys.exit(1)

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = Path(args.output) if args.output else output_dir / default_name

    print(f"Rendering rect={rect} at {dpi} DPI from page {args.page}...")
    raw = render_from_pdf(args.pdf, rect, dpi, args.page)
    print(f"  Raw render: {raw.shape[1]}x{raw.shape[0]} ({raw.shape[0]*raw.shape[1]:,} pixels)")

    result = process_image(raw)
    print(f"  After processing: {result.shape[1]}x{result.shape[0]} ({result.shape[0]*result.shape[1]:,} pixels)")

    cv2.imwrite(str(output_path), result)
    print(f"  Saved: {output_path}")


def cmd_enhance(args: argparse.Namespace) -> None:
    """Handle the 'enhance' subcommand."""
    input_path = Path(args.input)
    if not input_path.exists():
        print(f"Error: File not found: {input_path}", file=sys.stderr)
        sys.exit(1)

    output_path = Path(args.output) if args.output else input_path.with_stem(input_path.stem + "_enhanced")

    img = cv2.imread(str(input_path), cv2.IMREAD_UNCHANGED)
    if img is None:
        print(f"Error: Could not read image: {input_path}", file=sys.stderr)
        sys.exit(1)

    print(f"Input: {img.shape[1]}x{img.shape[0]} ({img.shape[0]*img.shape[1]:,} pixels)")
    result = process_image(img)
    print(f"Output: {result.shape[1]}x{result.shape[0]} ({result.shape[0]*result.shape[1]:,} pixels)")

    cv2.imwrite(str(output_path), result)
    print(f"Saved: {output_path}")


def cmd_tile(args: argparse.Namespace) -> None:
    """Handle the 'tile' subcommand."""
    input_path = Path(args.input)
    if not input_path.exists():
        print(f"Error: File not found: {input_path}", file=sys.stderr)
        sys.exit(1)

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    img = cv2.imread(str(input_path), cv2.IMREAD_UNCHANGED)
    if img is None:
        print(f"Error: Could not read image: {input_path}", file=sys.stderr)
        sys.exit(1)

    # Convert to grayscale first
    if len(img.shape) == 3:
        gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
    else:
        gray = img

    h, w = gray.shape[:2]
    tile_size = args.tile_size
    overlap = args.overlap
    step = tile_size - overlap

    print(f"Input: {w}x{h}, tile_size={tile_size}, overlap={overlap}, step={step}")

    tile_num = 0
    stem = input_path.stem
    for y in range(0, h, step):
        for x in range(0, w, step):
            y1 = min(y + tile_size, h)
            x1 = min(x + tile_size, w)
            tile = gray[y:y1, x:x1]

            # Skip mostly-white tiles
            if np.mean(tile) > BORDER_THRESHOLD:
                continue

            enhanced = enhance_image(tile)
            resized = resize_for_claude(enhanced)

            tile_path = output_dir / f"{stem}_tile_{tile_num:03d}_r{y}_c{x}.png"
            cv2.imwrite(str(tile_path), resized)
            tile_num += 1

    print(f"Generated {tile_num} tiles in {output_dir}/")


def cmd_list_regions(args: argparse.Namespace) -> None:
    """Handle the 'regions' subcommand."""
    print("Named schematic regions:")
    print()
    for name, info in NAMED_REGIONS.items():
        rect = info["rect"]
        print(f"  {name:20s}  DPI={info['dpi']:4d}  rect=({rect[0]:.2f},{rect[1]:.2f},{rect[2]:.2f},{rect[3]:.2f})")
        print(f"  {'':20s}  {info['description']}")
        print()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Preprocess schematic images for Claude's vision pipeline",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    # render subcommand
    p_render = subparsers.add_parser("render", help="Render a region from a PDF and preprocess it")
    p_render.add_argument("--pdf", required=True, help="Path to the PDF file")
    p_render.add_argument("--region", help="Named region (use 'regions' command to list)")
    p_render.add_argument("--rect", help="Custom rect as x0,y0,x1,y1 (normalized 0-1)")
    p_render.add_argument("--dpi", type=int, help="Override DPI (default: region-specific or 600)")
    p_render.add_argument("--page", type=int, default=0, help="PDF page number (default: 0)")
    p_render.add_argument("--output", help="Output file path")
    p_render.add_argument("--output-dir", default=str(DEFAULT_OUTPUT_DIR), help="Output directory (default: schematic_tiles/)")
    p_render.set_defaults(func=cmd_render)

    # enhance subcommand
    p_enhance = subparsers.add_parser("enhance", help="Preprocess an existing PNG image")
    p_enhance.add_argument("input", help="Input image path")
    p_enhance.add_argument("--output", help="Output file path")
    p_enhance.set_defaults(func=cmd_enhance)

    # tile subcommand
    p_tile = subparsers.add_parser("tile", help="Generate overlapping tiles from a large image")
    p_tile.add_argument("input", help="Input image path")
    p_tile.add_argument("--tile-size", type=int, default=1400, help="Tile size in pixels (default: 1400)")
    p_tile.add_argument("--overlap", type=int, default=200, help="Overlap between tiles (default: 200)")
    p_tile.add_argument("--output-dir", default=str(DEFAULT_OUTPUT_DIR), help="Output directory (default: schematic_tiles/)")
    p_tile.set_defaults(func=cmd_tile)

    # regions subcommand
    p_regions = subparsers.add_parser("regions", help="List named schematic regions")
    p_regions.set_defaults(func=cmd_list_regions)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
