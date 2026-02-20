"""Train MLP v2 for per-note parameter corrections.

Architecture: Input(2) -> Linear(H) -> ReLU -> Linear(H) -> ReLU -> Linear(11)
Trained with masked Huber loss weighted by isolation tier.

v2 target vector (11 dims):
  [0:5]   freq_offsets H2-H6 (cents)
  [5:10]  decay_offsets H2-H6 (ratio, >1 = model decays too fast)
  [10]    ds_correction (displacement scale multiplier from H2/H1 ratio)

Usage:
    python train_mlp.py
    python train_mlp.py --epochs 2000 --lr 1e-3 --hidden 8
    python train_mlp.py --export ml_data/model_weights.json
"""

import argparse
import json
import os
import sys
import numpy as np
import torch
import torch.nn as nn


N_FREQ = 5
N_DECAY = 5
N_OUTPUTS = N_FREQ + N_DECAY + 1  # 11
DS_IDX = N_FREQ + N_DECAY          # 10


class WurliMLP(nn.Module):
    """Small MLP for per-note parameter corrections (v2)."""

    def __init__(self, n_inputs=2, n_hidden=8, n_outputs=N_OUTPUTS):
        super().__init__()
        self.net = nn.Sequential(
            nn.Linear(n_inputs, n_hidden),
            nn.ReLU(),
            nn.Linear(n_hidden, n_hidden),
            nn.ReLU(),
            nn.Linear(n_hidden, n_outputs),
        )

    def forward(self, x):
        return self.net(x)


def masked_huber_loss(pred, target, mask, weights, delta=5.0):
    """Huber loss computed only on valid (masked) entries, weighted by tier.

    Huber is more robust to outliers than MSE -- important for noisy decay targets.
    """
    diff = pred - target
    abs_diff = diff.abs()
    # Huber: quadratic for |diff| < delta, linear beyond
    loss = torch.where(abs_diff < delta, 0.5 * diff ** 2, delta * (abs_diff - 0.5 * delta))

    # Apply mask: zero out invalid entries
    loss = loss * mask.float()

    # Weight by isolation tier (per-observation)
    # weights shape: (N,), need to broadcast to (N, n_targets)
    loss = loss * weights.unsqueeze(1)

    # Normalize by number of valid entries
    n_valid = mask.float().sum()
    if n_valid > 0:
        return loss.sum() / n_valid
    return torch.tensor(0.0)


def load_data(data_path, no_split=False, mask_decay_h3=False):
    """Load and preprocess training data.

    Returns train/val split as torch tensors.
    If no_split=True (for small datasets), train=val=all data.
    If mask_decay_h3=True, zero out the decay_H3 mask (too noisy to train on).
    """
    d = np.load(data_path)
    inputs = d['inputs']    # (N, 2) already normalized [0,1]
    targets = d['targets']  # (N, 11)
    mask = d['mask']        # (N, 11) bool
    weights = d['weights']  # (N,)

    n_targets = targets.shape[1]

    # Optionally mask decay_H3 (index N_FREQ+1 = 6) — often too noisy
    if mask_decay_h3:
        h3_decay_idx = N_FREQ + 1  # decay_H3
        n_masked = mask[:, h3_decay_idx].sum()
        mask[:, h3_decay_idx] = False
        print(f"  Masked decay_H3 (index {h3_decay_idx}): {n_masked} entries zeroed")

    # Clip extreme decay outliers (>20x ratio is certainly noise)
    decay_slice = slice(N_FREQ, N_FREQ + N_DECAY)
    targets[:, decay_slice] = np.clip(targets[:, decay_slice], -20.0, 20.0)

    # Clip ds_correction to [0.5, 2.0] (broader than runtime [0.7, 1.5] to let training explore)
    targets[:, DS_IDX] = np.clip(targets[:, DS_IDX], 0.5, 2.0)

    # Per-target standardization (mean/std from valid entries only)
    target_means = np.zeros(n_targets)
    target_stds = np.ones(n_targets)
    for i in range(n_targets):
        valid = mask[:, i]
        if valid.sum() > 1:
            target_means[i] = targets[valid, i].mean()
            target_stds[i] = max(targets[valid, i].std(), 1e-6)

    targets_norm = (targets - target_means) / target_stds

    def to_tensors(idx):
        return (
            torch.tensor(inputs[idx], dtype=torch.float32),
            torch.tensor(targets_norm[idx], dtype=torch.float32),
            torch.tensor(mask[idx], dtype=torch.bool),
            torch.tensor(weights[idx], dtype=torch.float32),
        )

    n = len(inputs)

    if no_split or n < 20:
        # Small dataset: train on all data, use same data for validation monitoring
        all_idx = np.arange(n)
        print(f"  Small dataset ({n} points) — training on all data (no split)")
        return to_tensors(all_idx), to_tensors(all_idx), target_means, target_stds

    # Shuffle and split 80/20
    rng = np.random.RandomState(42)
    perm = rng.permutation(n)

    split = int(0.8 * n)
    train_idx = perm[:split]
    val_idx = perm[split:]

    return to_tensors(train_idx), to_tensors(val_idx), target_means, target_stds


def train(model, train_data, val_data, epochs=1000, lr=3e-3, patience=100,
          huber_delta=5.0, weight_decay=1e-4, min_lr=1e-5):
    """Train with Adam, early stopping on validation loss."""
    optimizer = torch.optim.Adam(model.parameters(), lr=lr, weight_decay=weight_decay)
    scheduler = torch.optim.lr_scheduler.ReduceLROnPlateau(
        optimizer, mode='min', factor=0.5, patience=30, min_lr=min_lr)

    train_inputs, train_targets, train_mask, train_weights = train_data
    val_inputs, val_targets, val_mask, val_weights = val_data

    best_val_loss = float('inf')
    best_state = None
    stale = 0

    for epoch in range(epochs):
        # Train
        model.train()
        optimizer.zero_grad()
        pred = model(train_inputs)
        loss = masked_huber_loss(pred, train_targets, train_mask, train_weights, huber_delta)
        loss.backward()
        optimizer.step()

        # Validate
        model.eval()
        with torch.no_grad():
            val_pred = model(val_inputs)
            val_loss = masked_huber_loss(val_pred, val_targets, val_mask, val_weights, huber_delta)

        scheduler.step(val_loss)
        current_lr = optimizer.param_groups[0]['lr']

        if val_loss < best_val_loss:
            best_val_loss = val_loss
            best_state = {k: v.clone() for k, v in model.state_dict().items()}
            stale = 0
        else:
            stale += 1

        if (epoch + 1) % 100 == 0 or epoch == 0:
            print(f"  Epoch {epoch+1:>5}: train={loss.item():.4f}  "
                  f"val={val_loss.item():.4f}  best={best_val_loss.item():.4f}  "
                  f"lr={current_lr:.1e}  stale={stale}")

        if stale >= patience:
            print(f"  Early stopping at epoch {epoch+1} (patience={patience})")
            break

    # Restore best
    model.load_state_dict(best_state)
    return best_val_loss.item()


def evaluate(model, val_data, target_means, target_stds):
    """Evaluate model on validation set, report per-target errors in original units."""
    val_inputs, val_targets_norm, val_mask, val_weights = val_data

    model.eval()
    with torch.no_grad():
        pred_norm = model(val_inputs)

    # Denormalize
    means_t = torch.tensor(target_means, dtype=torch.float32)
    stds_t = torch.tensor(target_stds, dtype=torch.float32)
    pred = pred_norm * stds_t + means_t
    actual = val_targets_norm * stds_t + means_t

    names = ([f'freq_H{h}' for h in range(2, 2 + N_FREQ)] +
             [f'decay_H{h}' for h in range(2, 2 + N_DECAY)] +
             ['ds_corr'])

    print("\n  Per-target validation error (original units):")
    print(f"  {'Target':>12} {'N_val':>5} {'MAE':>8} {'RMSE':>8} {'Unit'}")
    print(f"  {'-'*45}")

    units = ['cents'] * N_FREQ + ['ratio'] * N_DECAY + ['']
    for i, (name, unit) in enumerate(zip(names, units)):
        valid = val_mask[:, i]
        if valid.sum() == 0:
            continue
        err = (pred[:, i] - actual[:, i])[valid]
        mae = err.abs().mean().item()
        rmse = (err ** 2).mean().sqrt().item()
        print(f"  {name:>12} {valid.sum():>5} {mae:>8.2f} {rmse:>8.2f}  {unit}")


def export_weights(model, target_means, target_stds, hidden_size, output_path):
    """Export model weights as JSON for Rust weight generator.

    Saves layer weights, biases, and target normalization params.
    """
    state = model.state_dict()

    layers = []
    # model.net is Sequential: Linear, ReLU, Linear, ReLU, Linear
    layer_keys = [
        ('net.0.weight', 'net.0.bias'),  # hidden1
        ('net.2.weight', 'net.2.bias'),  # hidden2
        ('net.4.weight', 'net.4.bias'),  # output
    ]
    activations = ['relu', 'relu', 'linear']

    for (wk, bk), act in zip(layer_keys, activations):
        w = state[wk].numpy().tolist()
        b = state[bk].numpy().tolist()
        layers.append({
            'weights': w,
            'bias': b,
            'activation': act,
        })

    output_size = len(layers[-1]["bias"])
    export = {
        'architecture': 'MLP',
        'input_size': 2,
        'output_size': output_size,
        'hidden_size': hidden_size,
        'layers': layers,
        'target_means': target_means.tolist(),
        'target_stds': target_stds.tolist(),
        'input_normalization': {
            'midi_range': [21, 108],
            'velocity_range': [0, 127],
        },
    }

    with open(output_path, 'w') as f:
        json.dump(export, f, indent=2)
    print(f"\n  Exported weights to {output_path}")
    n_params = sum(p.numel() for p in model.parameters())
    print(f"  Total parameters: {n_params}")


def main():
    parser = argparse.ArgumentParser(description="Train MLP for parameter corrections")
    parser.add_argument("--data", default="ml_data/training_data.npz",
                        help="Training data path")
    parser.add_argument("--epochs", type=int, default=2000)
    parser.add_argument("--lr", type=float, default=3e-3)
    parser.add_argument("--hidden", type=int, default=8)
    parser.add_argument("--patience", type=int, default=150)
    parser.add_argument("--huber-delta", type=float, default=5.0,
                        help="Huber loss delta (switch from quadratic to linear)")
    parser.add_argument("--weight-decay", type=float, default=1e-4,
                        help="Weight decay for Adam optimizer")
    parser.add_argument("--seed", type=int, default=42,
                        help="Random seed for reproducibility")
    parser.add_argument("--export", default="ml_data/model_weights.json",
                        help="Export path for weights JSON")
    parser.add_argument("--no-split", action="store_true",
                        help="Train on all data (no train/val split)")
    parser.add_argument("--mask-decay-h3", action="store_true",
                        help="Mask decay_H3 targets (too noisy)")
    args = parser.parse_args()

    torch.manual_seed(args.seed)
    np.random.seed(args.seed)

    data_path = os.path.join(os.path.dirname(__file__), args.data)
    print(f"Loading data from {data_path}...")
    train_data, val_data, target_means, target_stds = load_data(
        data_path, no_split=args.no_split, mask_decay_h3=args.mask_decay_h3)

    n_train = train_data[0].shape[0]
    n_val = val_data[0].shape[0]
    print(f"  Train: {n_train}, Val: {n_val}")
    print(f"  Train mask coverage: {train_data[2].float().mean():.1%}")
    print(f"  Val mask coverage: {val_data[2].float().mean():.1%}")

    model = WurliMLP(n_inputs=2, n_hidden=args.hidden, n_outputs=N_OUTPUTS)
    n_params = sum(p.numel() for p in model.parameters())
    print(f"\nModel: {n_params} parameters (hidden={args.hidden})")
    print(f"Training: epochs={args.epochs}, lr={args.lr}, patience={args.patience}, "
          f"huber_delta={args.huber_delta}, wd={args.weight_decay}")
    print()

    best_val = train(model, train_data, val_data,
                     epochs=args.epochs, lr=args.lr,
                     patience=args.patience, huber_delta=args.huber_delta,
                     weight_decay=args.weight_decay)
    print(f"\n  Best validation loss: {best_val:.4f}")

    evaluate(model, val_data, target_means, target_stds)

    # Export
    export_path = os.path.join(os.path.dirname(__file__), args.export)
    os.makedirs(os.path.dirname(export_path), exist_ok=True)
    export_weights(model, target_means, target_stds, args.hidden, export_path)

    # Also save PyTorch checkpoint
    ckpt_path = os.path.join(os.path.dirname(export_path), "model_checkpoint.pt")
    torch.save({
        'model_state': model.state_dict(),
        'target_means': torch.tensor(target_means, dtype=torch.float64),
        'target_stds': torch.tensor(target_stds, dtype=torch.float64),
        'best_val_loss': best_val,
        'hidden_size': args.hidden,
        'n_train': n_train,
        'n_val': n_val,
    }, ckpt_path)
    print(f"  Saved checkpoint to {ckpt_path}")


if __name__ == "__main__":
    main()
