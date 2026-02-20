"""ML training data extraction pipeline orchestrator.

Runs all stages in sequence:
1. Extract notes (basic-pitch + OBM injection)
2. Score isolation
3. Extract harmonics
4. Render model notes (via Rust preamp-bench)
5. Compute residuals -> training_data.npz
6. Train MLP
7. Export weights to Rust

Usage:
    python pipeline.py                     # Full pipeline (stages 1-5)
    python pipeline.py --obm-only          # OBM notes only (fast test)
    python pipeline.py --from-stage 3      # Resume from stage 3
    python pipeline.py --train             # Run stages 1-7 (including training)
    python pipeline.py --dry-run           # Show what would be done
"""

import argparse
import json
import os
import sys
import time


def stage_extract_notes(args):
    """Stage 1: Extract note events from recordings."""
    from extract_notes import extract_obm_notes, extract_polyphonic_notes, find_recordings

    all_notes = []

    print("Extracting OBM isolated notes...")
    obm_notes = extract_obm_notes()
    all_notes.extend(obm_notes)

    if not args.obm_only:
        recordings = find_recordings(args.input_dir)
        print(f"\nFound {len(recordings)} polyphonic recordings")
        for rec_path in recordings:
            try:
                from extract_notes import extract_polyphonic_notes
                notes = extract_polyphonic_notes(rec_path)
                all_notes.extend(notes)
            except Exception as e:
                print(f"  ERROR: {e}")

    output_path = os.path.join(os.path.dirname(__file__), "notes.json")
    with open(output_path, 'w') as f:
        json.dump(all_notes, f, indent=2)
    print(f"\nStage 1 complete: {len(all_notes)} notes -> notes.json")
    return all_notes


def stage_score_isolation(args):
    """Stage 2: Score isolation quality."""
    from score_isolation import score_notes, print_summary

    input_path = os.path.join(os.path.dirname(__file__), "notes.json")
    with open(input_path) as f:
        notes = json.load(f)

    score_notes(notes)
    print_summary(notes)

    output_path = os.path.join(os.path.dirname(__file__), "scored_notes.json")
    with open(output_path, 'w') as f:
        json.dump(notes, f, indent=2)
    print(f"\nStage 2 complete: {len(notes)} notes scored -> scored_notes.json")
    return notes


def stage_extract_harmonics(args):
    """Stage 3: Extract harmonic features."""
    from extract_harmonics import extract_all_harmonics, print_summary

    input_path = os.path.join(os.path.dirname(__file__), "scored_notes.json")
    with open(input_path) as f:
        notes = json.load(f)

    features = extract_all_harmonics(notes, min_tier="bronze")
    print_summary(features)

    output_path = os.path.join(os.path.dirname(__file__), "harmonics.json")
    with open(output_path, 'w') as f:
        json.dump(features, f, indent=2)
    print(f"\nStage 3 complete: {len(features)} features -> harmonics.json")
    return features


def stage_render_model(args):
    """Stage 4: Render model notes and extract features."""
    from render_model_notes import collect_unique_pairs, render_all_notes, extract_model_features

    input_path = os.path.join(os.path.dirname(__file__), "harmonics.json")
    with open(input_path) as f:
        real_features = json.load(f)

    pairs = collect_unique_pairs(real_features)
    print(f"Unique (midi, velocity) pairs: {len(pairs)}")

    ml_data_dir = os.path.join(os.path.dirname(__file__), "ml_data")
    render_dir = os.path.join(ml_data_dir, "renders")

    wav_paths = render_all_notes(pairs, render_dir)
    if not wav_paths:
        print("ERROR: No notes rendered")
        sys.exit(1)

    model_features = extract_model_features(wav_paths, pairs)

    output_data = {}
    for (midi, vel), feat in model_features.items():
        output_data[f"{midi}_{vel}"] = feat

    output_path = os.path.join(os.path.dirname(__file__), "model_harmonics.json")
    with open(output_path, 'w') as f:
        json.dump(output_data, f, indent=2)
    print(f"\nStage 4 complete: {len(output_data)} model features -> model_harmonics.json")
    return output_data


def stage_compute_residuals(args):
    """Stage 5: Compute residuals and assemble training data."""
    from compute_residuals import assemble_dataset, print_dataset_summary
    import numpy as np

    base_dir = os.path.dirname(__file__)
    with open(os.path.join(base_dir, "harmonics.json")) as f:
        real_features = json.load(f)
    with open(os.path.join(base_dir, "model_harmonics.json")) as f:
        model_features = json.load(f)

    inputs, targets, mask, weights, note_ids, filter_stats = assemble_dataset(
        real_features, model_features)

    print_dataset_summary(inputs, targets, mask, weights, note_ids, filter_stats)

    output_dir = os.path.join(base_dir, "ml_data")
    os.makedirs(output_dir, exist_ok=True)
    output_path = os.path.join(output_dir, "training_data.npz")
    np.savez(output_path, inputs=inputs, targets=targets, mask=mask, weights=weights)
    print(f"\nStage 5 complete: {inputs.shape[0]} observations -> ml_data/training_data.npz")


def stage_train(args):
    """Stage 6: Train MLP."""
    from train_mlp import WurliMLP, load_data, train, evaluate, export_weights
    import torch

    base_dir = os.path.dirname(__file__)
    data_path = os.path.join(base_dir, "ml_data", "training_data.npz")

    print(f"Loading data from {data_path}...")
    train_data, val_data, target_means, target_stds = load_data(data_path)

    n_train = train_data[0].shape[0]
    n_val = val_data[0].shape[0]
    print(f"  Train: {n_train}, Val: {n_val}")

    hidden = getattr(args, 'hidden', 8)
    from train_mlp import N_OUTPUTS
    model = WurliMLP(n_inputs=2, n_hidden=hidden, n_outputs=N_OUTPUTS)
    n_params = sum(p.numel() for p in model.parameters())
    print(f"\nModel: {n_params} parameters (hidden={hidden})")

    best_val = train(model, train_data, val_data,
                     epochs=2000, lr=3e-3, patience=150)
    print(f"\n  Best validation loss: {best_val:.4f}")

    evaluate(model, val_data, target_means, target_stds)

    export_path = os.path.join(base_dir, "ml_data", "model_weights.json")
    export_weights(model, target_means, target_stds, hidden, export_path)

    ckpt_path = os.path.join(base_dir, "ml_data", "model_checkpoint.pt")
    torch.save({
        'model_state': model.state_dict(),
        'target_means': torch.tensor(target_means, dtype=torch.float64),
        'target_stds': torch.tensor(target_stds, dtype=torch.float64),
        'best_val_loss': best_val,
        'hidden_size': hidden,
    }, ckpt_path)
    print(f"\nStage 6 complete: model trained -> ml_data/model_weights.json")


def stage_export_rust(args):
    """Stage 7: Export weights to Rust."""
    from generate_rust_weights import generate_rust_weights

    base_dir = os.path.dirname(__file__)
    weights_path = os.path.join(base_dir, "ml_data", "model_weights.json")
    output_path = os.path.join(base_dir, "..", "crates", "openwurli-dsp", "src", "mlp_weights.rs")

    generate_rust_weights(weights_path, output_path)
    print(f"\nStage 7 complete: Rust weights -> {output_path}")


STAGES = [
    (1, "Extract notes", stage_extract_notes),
    (2, "Score isolation", stage_score_isolation),
    (3, "Extract harmonics", stage_extract_harmonics),
    (4, "Render model notes", stage_render_model),
    (5, "Compute residuals", stage_compute_residuals),
    (6, "Train MLP", stage_train),
    (7, "Export to Rust", stage_export_rust),
]


def main():
    parser = argparse.ArgumentParser(description="OpenWurli ML training pipeline")
    parser.add_argument("--input-dir",
                        default=os.path.join(os.path.dirname(__file__), "input"),
                        help="Directory containing recordings")
    parser.add_argument("--obm-only", action="store_true",
                        help="Only use OBM isolated notes (fast test)")
    parser.add_argument("--from-stage", type=int, default=1,
                        help="Resume from this stage number (1-7)")
    parser.add_argument("--through-stage", type=int, default=5,
                        help="Stop after this stage (default: 5, use 7 for full pipeline)")
    parser.add_argument("--train", action="store_true",
                        help="Run through stage 7 (train + export)")
    parser.add_argument("--hidden", type=int, default=8,
                        help="MLP hidden layer size (for stage 6)")
    parser.add_argument("--dry-run", action="store_true",
                        help="Show what would be done without executing")
    args = parser.parse_args()

    if args.train:
        args.through_stage = 7

    print("=" * 70)
    print("  OpenWurli ML Training Pipeline")
    print("=" * 70)

    if args.dry_run:
        print("\nDRY RUN -- stages that would execute:")
        for num, name, _ in STAGES:
            if num > args.through_stage:
                status = "SKIP (beyond --through-stage)"
            elif num < args.from_stage:
                status = "SKIP (before --from-stage)"
            else:
                status = "RUN"
            print(f"  Stage {num}: {name} [{status}]")
        return

    total_start = time.time()

    for num, name, func in STAGES:
        if num > args.through_stage:
            break
        if num < args.from_stage:
            print(f"\nStage {num}: {name} [SKIPPED]")
            continue

        print(f"\n{'=' * 70}")
        print(f"  Stage {num}: {name}")
        print(f"{'=' * 70}")

        stage_start = time.time()
        func(args)
        elapsed = time.time() - stage_start
        print(f"  Stage {num} took {elapsed:.1f}s")

    total_elapsed = time.time() - total_start
    print(f"\n{'=' * 70}")
    print(f"  Pipeline complete in {total_elapsed:.1f}s")
    print(f"{'=' * 70}")


if __name__ == "__main__":
    main()
