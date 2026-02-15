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
    python tools/schematic_preprocess.py render --pdf docs/verified_wurlitzer_200A_series_schematic.pdf \\
        --rect 0.05,0.3,0.45,0.7 --dpi 600

    # Enhance an existing PNG
    python tools/schematic_preprocess.py enhance input.png --output enhanced.png

    # Generate overlapping tiles from a large image
    python tools/schematic_preprocess.py tile input.png --tile-size 1400 --overlap 200

    # Detect text/annotation regions in a schematic area
    python tools/schematic_preprocess.py detect-text \\
        --pdf docs/verified_wurlitzer_200A_series_schematic.pdf --region preamp \\
        --output-dir /tmp/text_detect/

    # Detect text from an existing image file
    python tools/schematic_preprocess.py detect-text --input preamp_600dpi.png \\
        --output-dir /tmp/text_detect/ --min-area 200

    # OCR a schematic region (requires: pip install easyocr)
    python tools/schematic_preprocess.py ocr \\
        --pdf docs/verified_wurlitzer_200A_series_schematic.pdf --region preamp-detail \\
        --output /tmp/ocr_results.json

    # OCR with annotated output image
    python tools/schematic_preprocess.py ocr --input preamp-detail_900dpi.png \\
        --annotate /tmp/ocr_annotated.png --min-confidence 0.5
"""

import argparse
import json
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


def _resolve_rect_and_dpi(args: argparse.Namespace) -> tuple[tuple[float, ...], int]:
    """Resolve region/rect/dpi from args. Returns (rect, dpi)."""
    if hasattr(args, "region") and args.region:
        if args.region not in NAMED_REGIONS:
            print(f"Error: Unknown region '{args.region}'. Available: {', '.join(NAMED_REGIONS)}", file=sys.stderr)
            sys.exit(1)
        region = NAMED_REGIONS[args.region]
        rect = region["rect"]
        dpi = getattr(args, "dpi", None) or region["dpi"]
    elif hasattr(args, "rect") and args.rect:
        rect = tuple(float(x) for x in args.rect.split(","))
        if len(rect) != 4:
            print("Error: --rect must be 4 comma-separated values: x0,y0,x1,y1", file=sys.stderr)
            sys.exit(1)
        dpi = getattr(args, "dpi", None) or 600
    else:
        rect = None
        dpi = getattr(args, "dpi", None) or 600
    return rect, dpi


def _load_grayscale(args: argparse.Namespace) -> np.ndarray:
    """Load a grayscale image from either --input file or --pdf + --region/--rect.

    Shared by detect-text and ocr subcommands.
    """
    input_path = getattr(args, "input", None)
    pdf_path = getattr(args, "pdf", None)

    if input_path:
        path = Path(input_path)
        if not path.exists():
            print(f"Error: File not found: {path}", file=sys.stderr)
            sys.exit(1)
        img = cv2.imread(str(path), cv2.IMREAD_UNCHANGED)
        if img is None:
            print(f"Error: Could not read image: {path}", file=sys.stderr)
            sys.exit(1)
        if len(img.shape) == 3:
            img = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
        return img
    elif pdf_path:
        rect, dpi = _resolve_rect_and_dpi(args)
        if rect is None:
            print("Error: --pdf requires --region or --rect", file=sys.stderr)
            sys.exit(1)
        page = getattr(args, "page", 0)
        return render_from_pdf(pdf_path, rect, dpi, page)
    else:
        print("Error: Must specify --input or --pdf (with --region/--rect)", file=sys.stderr)
        sys.exit(1)


def detect_text_regions(
    img_gray: np.ndarray,
    kernel_w: int = 15,
    kernel_h: int = 5,
    min_area: int = 100,
    max_area: int = 50000,
    margin: int = 8,
) -> list[dict]:
    """Detect text/annotation regions in a grayscale schematic image.

    Uses adaptive thresholding + morphological dilation to cluster nearby
    text characters into bounding boxes.

    Returns list of {x, y, w, h} dicts sorted top-to-bottom, left-to-right.
    """
    # Adaptive threshold to get binary text mask (white text on black bg)
    binary = cv2.adaptiveThreshold(
        img_gray, 255, cv2.ADAPTIVE_THRESH_GAUSSIAN_C, cv2.THRESH_BINARY_INV, 15, 8
    )

    # Dilate to merge nearby characters into text blocks
    kernel = cv2.getStructuringElement(cv2.MORPH_RECT, (kernel_w, kernel_h))
    dilated = cv2.dilate(binary, kernel, iterations=1)

    # Find contours of the merged text blocks
    contours, _ = cv2.findContours(dilated, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)

    img_h, img_w = img_gray.shape[:2]
    regions = []
    for cnt in contours:
        x, y, w, h = cv2.boundingRect(cnt)
        area = w * h

        # Filter by area
        if area < min_area or area > max_area:
            continue

        # Filter out very extreme aspect ratios (likely wire segments, not text)
        aspect = w / h if h > 0 else 0
        if aspect > 30 or aspect < 0.03:
            continue

        # Expand with margin, clamped to image bounds
        x0 = max(0, x - margin)
        y0 = max(0, y - margin)
        x1 = min(img_w, x + w + margin)
        y1 = min(img_h, y + h + margin)

        regions.append({"x": x0, "y": y0, "w": x1 - x0, "h": y1 - y0})

    # Sort top-to-bottom, then left-to-right (with row tolerance)
    if regions:
        avg_h = sum(r["h"] for r in regions) / len(regions)
        row_tolerance = avg_h * 0.6
        regions.sort(key=lambda r: (round(r["y"] / row_tolerance) * row_tolerance, r["x"]))

    return regions


def _import_easyocr():
    """Import easyocr with graceful fallback."""
    try:
        import easyocr
        return easyocr
    except ImportError:
        print("Error: easyocr not installed. Run: pip install easyocr", file=sys.stderr)
        sys.exit(1)


def run_ocr(
    img_gray: np.ndarray,
    min_confidence: float = 0.3,
) -> list[dict]:
    """Run OCR on a grayscale image using easyocr.

    Returns list of {text, confidence, bbox: {x, y, w, h}} dicts.
    """
    easyocr = _import_easyocr()

    reader = easyocr.Reader(["en"], gpu=False, verbose=False)
    results = reader.readtext(img_gray)

    detections = []
    for bbox_pts, text, conf in results:
        if conf < min_confidence:
            continue

        # Convert polygon points to bounding rect
        pts = np.array(bbox_pts, dtype=np.int32)
        x, y, w, h = cv2.boundingRect(pts)

        detections.append({
            "text": text,
            "confidence": round(float(conf), 4),
            "bbox": {"x": int(x), "y": int(y), "w": int(w), "h": int(h)},
        })

    # Sort same way as detect_text_regions
    if detections:
        avg_h = sum(d["bbox"]["h"] for d in detections) / len(detections)
        row_tolerance = max(avg_h * 0.6, 1)
        detections.sort(key=lambda d: (
            round(d["bbox"]["y"] / row_tolerance) * row_tolerance,
            d["bbox"]["x"],
        ))

    return detections


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


def cmd_detect_text(args: argparse.Namespace) -> None:
    """Handle the 'detect-text' subcommand."""
    img = _load_grayscale(args)
    print(f"Input: {img.shape[1]}x{img.shape[0]} ({img.shape[0]*img.shape[1]:,} pixels)")

    regions = detect_text_regions(
        img,
        kernel_w=args.kernel_w,
        kernel_h=args.kernel_h,
        min_area=args.min_area,
        max_area=args.max_area,
        margin=args.margin,
    )
    print(f"Detected {len(regions)} text regions")

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    # Save annotated overview
    annotated = cv2.cvtColor(img, cv2.COLOR_GRAY2BGR)
    for i, r in enumerate(regions):
        cv2.rectangle(annotated, (r["x"], r["y"]), (r["x"] + r["w"], r["y"] + r["h"]), (0, 0, 255), 2)
        # Put index number above the box
        label_y = max(r["y"] - 4, 12)
        cv2.putText(annotated, str(i), (r["x"], label_y), cv2.FONT_HERSHEY_SIMPLEX, 0.4, (0, 0, 255), 1)
    # Resize the color annotated image to fit Claude's vision limits
    h, w = annotated.shape[:2]
    scale = 1.0
    if max(h, w) > MAX_LONG_EDGE:
        scale = min(scale, MAX_LONG_EDGE / max(h, w))
    if h * w > MAX_PIXELS:
        scale = min(scale, (MAX_PIXELS / (h * w)) ** 0.5)
    if scale < 1.0:
        annotated = cv2.resize(annotated, (max(1, int(w * scale)), max(1, int(h * scale))), interpolation=cv2.INTER_AREA)
    overview_path = output_dir / "detected_regions.png"
    cv2.imwrite(str(overview_path), annotated)
    print(f"  Overview: {overview_path}")

    # Save manifest JSON
    manifest = {"region_count": len(regions), "source_size": {"w": img.shape[1], "h": img.shape[0]}, "regions": regions}
    manifest_path = output_dir / "detected_regions.json"
    manifest_path.write_text(json.dumps(manifest, indent=2))
    print(f"  Manifest: {manifest_path}")

    # Save individual enhanced crops
    for i, r in enumerate(regions):
        crop = img[r["y"]:r["y"] + r["h"], r["x"]:r["x"] + r["w"]]
        enhanced = enhance_image(crop)
        crop_path = output_dir / f"text_region_{i:03d}.png"
        cv2.imwrite(str(crop_path), enhanced)

    print(f"  Crops: {len(regions)} files in {output_dir}/")


def cmd_ocr(args: argparse.Namespace) -> None:
    """Handle the 'ocr' subcommand."""
    img = _load_grayscale(args)
    print(f"Input: {img.shape[1]}x{img.shape[0]} ({img.shape[0]*img.shape[1]:,} pixels)")

    # Enhance before OCR for better recognition
    enhanced = enhance_image(img)

    detections = run_ocr(enhanced, min_confidence=args.min_confidence)
    print(f"Detected {len(detections)} text elements")

    # Output JSON
    result_json = json.dumps(detections, indent=2)
    if args.output:
        output_path = Path(args.output)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(result_json)
        print(f"Results: {output_path}")
    else:
        print(result_json)

    # Optional annotated image
    if args.annotate:
        annotated = cv2.cvtColor(enhanced, cv2.COLOR_GRAY2BGR)
        for det in detections:
            b = det["bbox"]
            cv2.rectangle(annotated, (b["x"], b["y"]), (b["x"] + b["w"], b["y"] + b["h"]), (0, 255, 0), 2)
            label = f"{det['text']} ({det['confidence']:.2f})"
            label_y = max(b["y"] - 4, 12)
            cv2.putText(annotated, label, (b["x"], label_y), cv2.FONT_HERSHEY_SIMPLEX, 0.35, (0, 255, 0), 1)

        annotate_path = Path(args.annotate)
        annotate_path.parent.mkdir(parents=True, exist_ok=True)
        cv2.imwrite(str(annotate_path), annotated)
        print(f"Annotated: {annotate_path}")


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

    # detect-text subcommand
    p_detect = subparsers.add_parser("detect-text", help="Detect text/annotation regions in a schematic")
    p_detect.add_argument("--input", help="Input image file")
    p_detect.add_argument("--pdf", help="PDF file (use with --region or --rect)")
    p_detect.add_argument("--region", help="Named region")
    p_detect.add_argument("--rect", help="Custom rect as x0,y0,x1,y1 (normalized 0-1)")
    p_detect.add_argument("--dpi", type=int, help="Override DPI")
    p_detect.add_argument("--page", type=int, default=0, help="PDF page number (default: 0)")
    p_detect.add_argument("--output-dir", default="/tmp/detect_text", help="Output directory")
    p_detect.add_argument("--kernel-w", type=int, default=15, help="Dilation kernel width (default: 15)")
    p_detect.add_argument("--kernel-h", type=int, default=5, help="Dilation kernel height (default: 5)")
    p_detect.add_argument("--min-area", type=int, default=100, help="Min region area in pixels (default: 100)")
    p_detect.add_argument("--max-area", type=int, default=50000, help="Max region area in pixels (default: 50000)")
    p_detect.add_argument("--margin", type=int, default=8, help="Margin around detected regions (default: 8)")
    p_detect.set_defaults(func=cmd_detect_text)

    # ocr subcommand
    p_ocr = subparsers.add_parser("ocr", help="OCR text from a schematic (requires easyocr)")
    p_ocr.add_argument("--input", help="Input image file")
    p_ocr.add_argument("--pdf", help="PDF file (use with --region or --rect)")
    p_ocr.add_argument("--region", help="Named region")
    p_ocr.add_argument("--rect", help="Custom rect as x0,y0,x1,y1 (normalized 0-1)")
    p_ocr.add_argument("--dpi", type=int, help="Override DPI")
    p_ocr.add_argument("--page", type=int, default=0, help="PDF page number (default: 0)")
    p_ocr.add_argument("--output", help="Output JSON file (default: stdout)")
    p_ocr.add_argument("--annotate", help="Save annotated image with OCR results to this path")
    p_ocr.add_argument("--min-confidence", type=float, default=0.3, help="Min OCR confidence (default: 0.3)")
    p_ocr.set_defaults(func=cmd_ocr)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
